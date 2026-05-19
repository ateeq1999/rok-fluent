# API: SQLite (`rok_fluent::orm::sqlite`) — feature: `sqlite`

```toml
rok-fluent = { version = "0.4", features = ["sqlite"] }
```

## Connect

```rust,no_run
use sqlx::SqlitePool;
use rok_fluent::orm::sqlite;

// File-backed
let pool = SqlitePool::connect("sqlite:./myapp.db").await?;

// In-memory (tests)
let pool = SqlitePool::connect("sqlite::memory:").await?;

sqlite::pool::set(pool);
```

## `SqliteModel` CRUD Trait

```rust,no_run
use rok_fluent::orm::sqlite::model::SqliteModel;

// Find
let record = Config::find(1_i64).await?;
let record = Config::find_or_fail(1_i64).await?;
let all = Config::all().await?;

// Insert — returns last_insert_rowid
let id = Config::insert(&[("key", "theme".into()), ("value", "dark".into())]).await?;

// Update
Config::update_where(&[("value", "light".into())], &[("key", "theme".into())]).await?;

// Delete
Config::delete_where(&[("key", "theme".into())]).await?;

// Count
let n = Config::count_where(&[("active", true.into())]).await?;
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
    sqlite::pool::set(pool.clone());

    let id = Config::insert(&[("key", "lang".into()), ("value", "en".into())])
        .await
        .unwrap();
    assert!(id > 0);
}
```
