# Guide: Migrations

## Setup

```toml
[dependencies]
rok-fluent = { version = "0.4", features = ["migrate-postgres"] }
```

## Directory Layout

Keep migration files in a `migrations/` directory at the project root. Name files so
they sort lexicographically in the order they should run:

```
migrations/
  2024_01_01_000001_create_users.sql
  2024_01_01_000002_create_posts.sql
  2024_01_15_000001_add_active_to_users.sql
```

Or use a sequence prefix:

```
migrations/
  001_create_users.sql
  002_create_posts.sql
  003_add_active_to_users.sql
```

## Running Migrations at Startup

```rust,no_run
use rok_fluent::migrate::{FileSource, MigrationRunner};
use sqlx::PgPool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pool = PgPool::connect(&std::env::var("DATABASE_URL")?).await?;

    MigrationRunner::new(pool.clone())
        .source(FileSource::new("migrations/"))
        .run()
        .await?;

    // … start your application
    Ok(())
}
```

`MigrationRunner` creates a `_migrations` table on first run and tracks every applied
migration by name. Re-running is idempotent — already-applied migrations are skipped.

## Writing SQL Migrations

```sql
-- migrations/2024_01_01_000001_create_users.sql
CREATE TABLE users (
    id          BIGSERIAL PRIMARY KEY,
    name        VARCHAR(255) NOT NULL,
    email       VARCHAR(255) NOT NULL UNIQUE,
    active      BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    updated_at  TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    deleted_at  TIMESTAMP WITH TIME ZONE
);

CREATE INDEX idx_users_email ON users (email);
```

## Writing Typed Rust Migrations

Preferred for migrations that need logic (conditional DDL, data backfills):

```rust,no_run
use async_trait::async_trait;
use rok_fluent::migrate::{Migration, Schema};
use sqlx::PgPool;

pub struct AddViewCountToPosts;

#[async_trait]
impl Migration for AddViewCountToPosts {
    fn name(&self) -> &str { "2024_02_01_000001_add_view_count_to_posts" }

    async fn up(&self, pool: &PgPool) -> anyhow::Result<()> {
        Schema::alter("posts", |t| {
            t.add_column("view_count").big_integer().not_null().default("0");
        })
        .execute(pool)
        .await?;

        // Data backfill
        sqlx::query("UPDATE posts SET view_count = 0 WHERE view_count IS NULL")
            .execute(pool)
            .await?;

        Ok(())
    }

    async fn down(&self, pool: &PgPool) -> anyhow::Result<()> {
        Schema::alter("posts", |t| {
            t.drop_column("view_count");
        })
        .execute(pool)
        .await
    }
}
```

Register alongside SQL migrations:

```rust,no_run
MigrationRunner::new(pool.clone())
    .source(FileSource::new("migrations/"))
    .typed(vec![Box::new(AddViewCountToPosts)])
    .run()
    .await?;
```

## Embedded Migrations (for libraries)

Libraries that ship their own schema should embed migrations so users don't need to
copy SQL files:

```rust,no_run
use rok_fluent::migrate::{EmbeddedMigration, EmbeddedMigrations};

pub fn schema_migrations() -> EmbeddedMigrations {
    EmbeddedMigrations::new(vec![
        EmbeddedMigration::new(
            "rok_jobs_001_create_jobs",
            include_str!("../sql/001_create_jobs.sql"),
        ),
        EmbeddedMigration::new(
            "rok_jobs_002_add_priority",
            include_str!("../sql/002_add_priority.sql"),
        ),
    ])
}
```

Users compose sources:

```rust,no_run
MigrationRunner::new(pool.clone())
    .source(FileSource::new("migrations/"))        // app migrations
    .source(rok_jobs::schema_migrations())          // library migrations
    .run()
    .await?;
```

## Rollbacks

Rollback is intentionally manual — it requires calling `down()` on specific migrations:

```rust,no_run
// Roll back to a named migration
MigrationRunner::new(pool.clone())
    .rollback_to("2024_01_01_000001_create_users")
    .await?;

// Roll back one step
MigrationRunner::new(pool.clone())
    .rollback_one()
    .await?;
```

## Testing Migrations

Use an in-memory or throwaway database in CI:

```rust,no_run
#[tokio::test]
async fn test_schema_is_current() {
    let pool = sqlx::PgPool::connect("postgres://localhost/test_db")
        .await
        .unwrap();

    MigrationRunner::new(pool.clone())
        .source(FileSource::new("migrations/"))
        .run()
        .await
        .unwrap();

    // Verify schema
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM _migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(count.0 > 0);
}
```

## `rok db` CLI

The `cli` feature ships a `rok` binary for running migrations without embedding the
runner in your application binary.

```sh
# Set the database URL
export DATABASE_URL=postgres://user:pass@localhost/mydb

# Run pending migrations
rok db migrate

# Check status
rok db status

# Roll back the last batch
rok db rollback

# Create a new migration file
rok db make create_tags_table
# → creates: migrations/20260521120000_create_tags_table.sql

# Inspect the live schema
rok db schema dump
rok db schema diff

# Custom migrations directory
rok db migrate --dir db/migrations
rok db make add_index_to_users --dir db/migrations
```

The generated migration template uses `-- up` / `-- down` delimiters:

```sql
-- up

-- create_tags_table


-- down

-- DROP TABLE IF EXISTS ...;
```

---

## Best Practices

- **Never edit a committed migration.** Add a new migration to alter it.
- **Make migrations transactional.** DDL runs in a transaction by default; if the
  migration fails, nothing is applied.
- **Test both `up` and `down`.** Rollback paths should be verified in CI.
- **Use descriptive names.** `add_email_to_users` beats `migration_003`.
- **Keep migrations small.** One logical change per migration. Large multi-table
  migrations are harder to roll back.
- **Backfill data in the same migration as the schema change** when the column is
  immediately `NOT NULL` with no default.
