# API: SQLite (`rok_fluent::orm::sqlite`) — feature: `sqlite`

```toml
rok-fluent = { version = "0.4", features = ["sqlite"] }
```

## Connect

```rust,no_run
use sqlx::SqlitePool;
use rok_fluent::orm::sqlite::model::SqliteModel;

// File-backed
let pool = SqlitePool::connect("sqlite:./myapp.db").await?;

// In-memory (tests)
let pool = SqlitePool::connect("sqlite::memory:").await?;
```

## `SqliteModel` CRUD Trait

`SqliteModel` methods always take the pool explicitly — there is no task-local
pool scoping for this backend (that's PostgreSQL-only, via `OrmLayer` /
`orm::postgres::pool::with_pool`).

```rust,no_run
use rok_fluent::orm::sqlite::model::SqliteModel;

// Find
let record = Config::find_by_pk(&pool, 1_i64).await?;   // Option<Config>
let all = Config::all(&pool).await?;

// Insert — RETURNING * (requires SQLite 3.35+)
let row: Config = Config::create_returning(
    &pool,
    &[("key", "theme".into()), ("value", "dark".into())],
).await?;

// Update
Config::update_by_pk(&pool, 1_i64, &[("value", "light".into())]).await?;

// Delete
Config::delete_by_pk(&pool, 1_i64).await?;

// Count
let n = Config::count(&pool).await?;
```

## Executor

```rust,no_run
use rok_fluent::orm::sqlite::executor;

let rows = executor::fetch_all::<Config>(&pool, &sql, params).await?;
let id = executor::insert(&pool, &sql, params).await?;
let n = executor::execute(&pool, &sql, params).await?;
```

## WAL Mode (recommended)

Enable Write-Ahead Logging for better concurrent read performance:

```rust,no_run
sqlx::query("PRAGMA journal_mode=WAL;")
    .execute(&pool)
    .await?;
```

## Testing

SQLite in-memory databases are ideal for fast unit tests. Each test gets its own
isolated database:

```rust,no_run
#[tokio::test]
async fn test_config_crud() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let row: Config = Config::create_returning(
        &pool,
        &[("key", "lang".into()), ("value", "en".into())],
    )
    .await
    .unwrap();
    assert!(row.id > 0);
}
```
