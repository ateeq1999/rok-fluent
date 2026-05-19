# rok-orm-migrate

> Database migration DSL with schema builder, transaction retry, and enum DDL for the Rok ORM.

Part of the [Rok Framework](https://rok.rs) — a full-stack Rust web framework built on Axum 0.8 and SQLx 0.8.

[![crates.io](https://img.shields.io/crates/v/rok-orm-migrate)](https://crates.io/crates/rok-orm-migrate) [![docs.rs](https://img.shields.io/docsrs/rok-orm-migrate)](https://docs.rs/rok-orm-migrate) [![MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

## Features

- Versioned SQL migration files (`001_name.up.sql` / `001_name.down.sql`) applied in order
- Embedded migrations compiled into the binary via `include_str!` or `MigrationSource` trait
- Fluent schema builder: `create_table!`, `alter_table!`, `drop_table!`, `add_column!`, `add_index!`
- Foreign key helpers: `add_foreign_key!` with `on_delete` and `on_update` actions
- PostgreSQL enum DDL: `Schema::create_enum` and `Schema::alter_enum_add`
- Schema cache population at migration time for runtime column introspection
- Transaction retry for concurrent migration runs (avoids duplicate-apply races)
- CLI integration via `rok db:migrate`, `rok db:rollback`, `rok db:status`, `rok db:fresh`

## Installation

```toml
[dependencies]
rok-orm-migrate = { version = "0.2", features = ["postgres"] }
```

## Quick Start

```rust
use rok_orm_migrate::{MigrationRunner, FileSource};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL")?).await?;

    MigrationRunner::new(pool.clone())
        .source(rok_auth::migrations())          // embedded crate migrations
        .source(rok_acl::migrations())
        .source(FileSource::new("./migrations")) // project SQL files
        .run()
        .await?;

    Ok(())
}
```

## Core API

### Schema builder

```rust
use rok_orm_migrate::Schema;

// Create table
let sql = Schema::create("users", |t| {
    t.id();                                  // BIGSERIAL PRIMARY KEY
    t.string("email").unique().not_null();
    t.string("name").not_null();
    t.boolean("active").default(true);
    t.string("avatar_url").nullable();
    t.timestamps();                          // created_at, updated_at
});

// Add column
let sql = Schema::alter("users", |t| {
    t.add_column("phone", |c| c.string().nullable());
    t.add_index("idx_users_phone", &["phone"]);
});

// Drop table
let sql = Schema::drop("users");
```

### Foreign keys and indexes

```rust
let sql = Schema::alter("posts", |t| {
    t.add_foreign_key("user_id", "users", "id")
        .on_delete("CASCADE")
        .on_update("RESTRICT");

    t.add_index("idx_posts_user_published", &["user_id", "published"]);
    t.add_unique_index("idx_posts_slug", &["slug"]);
});
```

### Enum DDL (PostgreSQL)

```rust
use rok_orm_migrate::Schema;

// Create a new PostgreSQL enum type
let sql = Schema::create_enum("content_status", &["draft", "published", "archived"]);

// Add a variant to an existing enum (non-destructive)
let sql = Schema::alter_enum_add("content_status", "scheduled");

// Use an enum column in a table
let sql = Schema::create("articles", |t| {
    t.id();
    t.string("title").not_null();
    t.enum_col("status", "content_status").default("draft");
    t.timestamps();
});
```

### Transaction retry

```rust
use rok_orm_migrate::RetryConfig;

// Applied automatically during MigrationRunner::run()
// Configure if needed:
let runner = MigrationRunner::new(pool)
    .retry(RetryConfig {
        max_attempts: 5,
        base_delay_ms: 100,
    })
    .source(FileSource::new("./migrations"))
    .run()
    .await?;
```

## Feature Flags

| Flag | Description | Default |
|------|-------------|---------|
| `postgres` | PostgreSQL migration tracking (`_rok_migrations` table) | No |
| `sqlite` | SQLite migration tracking | No |
| `mysql` | MySQL migration tracking | No |

## Integration

`rok-orm-migrate` is typically called once at application startup before the Axum router is bound. Each Rok crate that requires DB tables (e.g. `rok-auth`, `rok-acl`, `rok-queue`) exposes a `migrations()` function returning an embedded `MigrationSource`. Register them all in one `MigrationRunner` so migrations are applied in dependency order.

```rust
// In main.rs
MigrationRunner::new(pool.clone())
    .source(rok_auth::migrations())
    .source(rok_acl::migrations())
    .source(rok_queue::migrations())
    .source(rok_notification::migrations())
    .source(FileSource::new("./migrations"))  // app-specific migrations last
    .run()
    .await
    .expect("migrations failed");
```

The CLI command `rok db:status` reads the same `_rok_migrations` tracking table and displays which migrations have been applied and which are pending.

## License

MIT
