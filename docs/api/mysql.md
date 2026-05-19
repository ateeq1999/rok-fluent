# API: MySQL (`rok_fluent::orm::mysql`) — feature: `mysql`

```toml
rok-fluent = { version = "0.4", features = ["mysql"] }
```

## Connect

```rust,no_run
use sqlx::MySqlPool;
use rok_fluent::orm::mysql;

let pool = MySqlPool::connect("mysql://root:password@localhost/mydb").await?;
mysql::pool::set(pool);
```

## `MySqlModel` CRUD Trait

```rust,no_run
use rok_fluent::orm::mysql::model::MySqlModel;

// Find
let user = User::find(1_u64).await?;
let user = User::find_or_fail(1_u64).await?;
let users = User::all().await?;
let users = User::query().where_eq("active", true).all().await?;

// Insert
let id = User::insert(&[("name", "Alice".into()), ("email", "a@b.com".into())]).await?;

// Update
User::update_where(&[("active", false.into())], &[("id", 1_u64.into())]).await?;

// Delete
User::delete_where(&[("id", 1_u64.into())]).await?;

// Count
let n = User::count_where(&[("active", true.into())]).await?;
```

## Executor

```rust,no_run
use rok_fluent::orm::mysql::executor;

let rows = executor::fetch_all::<User>(&pool, &sql, params).await?;
let id = executor::insert(&pool, &sql, params).await?;
let n = executor::execute(&pool, &sql, params).await?;
```

## Dialect

MySQL uses `?` parameter placeholders. `QueryBuilder` switches placeholder style
automatically when built with `Dialect::MySql`.

```rust,no_run
use rok_fluent::core::query::Dialect;

let (sql, params) = User::query()
    .dialect(Dialect::MySql)
    .where_eq("active", true)
    .to_sql();
// sql: "SELECT * FROM users WHERE active = ?"
```
