# API: Migrations (`rok_fluent::migrate`) — feature: `migrate`

```toml
rok-fluent = { version = "0.4", features = ["migrate-postgres"] }
# or migrate-sqlite / migrate-mysql
```

## MigrationRunner

Orchestrates multiple migration sources. Runs each source's pending migrations in
dependency order. Uses a `_migrations` table to track applied versions.

```rust,no_run
use rok_fluent::migrate::{MigrationRunner, FileSource, EmbeddedMigrations};

MigrationRunner::new(&pool)
    .source(FileSource::new("migrations/"))
    .source(EmbeddedMigrations::from(framework_migrations()))
    .run()
    .await?;
```

### Sources

#### `FileSource`

Reads `*.sql` files from a directory. Files are sorted lexicographically; prefix with a
timestamp or sequence number (e.g., `001_create_users.sql`).

```rust,no_run
use rok_fluent::migrate::FileSource;

FileSource::new("migrations/")
FileSource::new("migrations/").with_extension("up.sql")
```

#### `EmbeddedMigrations`

SQL compiled into the binary via `include_str!`.

```rust,no_run
use rok_fluent::migrate::{EmbeddedMigrations, EmbeddedMigration};

fn framework_migrations() -> EmbeddedMigrations {
    EmbeddedMigrations::new(vec![
        EmbeddedMigration::new("001_create_jobs", include_str!("../sql/001_create_jobs.sql")),
        EmbeddedMigration::new("002_create_events", include_str!("../sql/002_create_events.sql")),
    ])
}
```

---

## `Migration` Trait

Typed Rust migrations. Preferred for application migrations — type-safe and
IDEable.

```rust,no_run
use async_trait::async_trait;
use rok_fluent::migrate::{Migration, Schema};
use sqlx::PgPool;

pub struct CreateUsersTable;

#[async_trait]
impl Migration for CreateUsersTable {
    fn name(&self) -> &str { "2024_01_01_000001_create_users_table" }

    async fn up(&self, pool: &PgPool) -> anyhow::Result<()> {
        Schema::create("users", |t| {
            t.increments("id");
            t.string("name").not_null();
            t.string("email").not_null().unique();
            t.boolean("active").not_null().default("true");
            t.timestamp("created_at").not_null().default("now()");
            t.timestamp("updated_at").not_null().default("now()");
            t.timestamp("deleted_at").nullable();
        })
        .execute(pool)
        .await
    }

    async fn down(&self, pool: &PgPool) -> anyhow::Result<()> {
        Schema::drop("users").execute(pool).await
    }
}
```

---

## Schema Builder

### `Schema::create`

```rust,no_run
use rok_fluent::migrate::Schema;

Schema::create("posts", |t| {
    t.increments("id");                      // BIGSERIAL PRIMARY KEY
    t.big_integer("user_id").not_null();
    t.string("title").not_null();            // VARCHAR(255) NOT NULL
    t.string("slug").not_null().unique();
    t.text("body").nullable();
    t.json("metadata").nullable();
    t.enum_col("status", &["draft", "published"]).not_null().default("'draft'");
    t.boolean("featured").not_null().default("false");
    t.timestamps();                          // created_at + updated_at
    t.soft_deletes();                        // deleted_at
    t.foreign("user_id").references("id").on("users").on_delete("cascade");
    t.index(&["slug"]);
    t.unique_index(&["user_id", "slug"]);
})
.execute(&pool)
.await?;
```

### `Schema::alter`

```rust,no_run
Schema::alter("posts", |t| {
    t.add_column("view_count").big_integer().not_null().default("0");
    t.rename_column("body", "content");
    t.drop_column("featured");
    t.add_index("idx_posts_user", &["user_id"]);
})
.execute(&pool)
.await?;
```

### `Schema::drop`

```rust,no_run
Schema::drop("posts").execute(&pool).await?;
Schema::drop_if_exists("posts").execute(&pool).await?;
```

---

## `TableBuilder` Column Methods

| Method | SQL type |
|---|---|
| `.increments(col)` | `BIGSERIAL PRIMARY KEY` |
| `.integer(col)` | `INTEGER` |
| `.big_integer(col)` | `BIGINT` |
| `.float(col)` | `FLOAT8` |
| `.decimal(col, p, s)` | `NUMERIC(p, s)` |
| `.boolean(col)` | `BOOLEAN` |
| `.string(col)` | `VARCHAR(255)` |
| `.char(col, n)` | `CHAR(n)` |
| `.text(col)` | `TEXT` |
| `.uuid(col)` | `UUID` |
| `.json(col)` | `JSONB` (Postgres) / `JSON` (MySQL/SQLite) |
| `.timestamp(col)` | `TIMESTAMP WITH TIME ZONE` |
| `.date(col)` | `DATE` |
| `.time(col)` | `TIME` |
| `.enum_col(col, variants)` | `VARCHAR(255)` with check or native ENUM |
| `.binary(col)` | `BYTEA` |
| `.timestamps()` | shorthand for `created_at` + `updated_at` |
| `.soft_deletes()` | shorthand for `deleted_at TIMESTAMP WITH TIME ZONE NULL` |

Column modifiers (chainable):

| Modifier | Effect |
|---|---|
| `.not_null()` | `NOT NULL` |
| `.nullable()` | `NULL` (default for most types) |
| `.default(expr)` | `DEFAULT expr` |
| `.unique()` | `UNIQUE` |
| `.primary()` | `PRIMARY KEY` |
| `.references(col).on(table)` | foreign key inline |
