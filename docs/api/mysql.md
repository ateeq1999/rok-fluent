# API: MySQL (`rok_fluent::orm::mysql`) — feature: `mysql`

```toml
rok-fluent = { version = "0.4", features = ["mysql"] }
```

## Connect

```rust,no_run
use sqlx::MySqlPool;
use rok_fluent::orm::mysql::model::MysqlModel;

let pool = MySqlPool::connect("mysql://root:password@localhost/mydb").await?;
```

## `MysqlModel` CRUD Trait

`MysqlModel` methods always take the pool explicitly — there is no task-local
pool scoping for this backend (that's PostgreSQL-only, via `OrmLayer` /
`orm::postgres::pool::with_pool`).

```rust,no_run
use rok_fluent::orm::mysql::model::MysqlModel;

// Find
let user = User::find_by_pk(&pool, 1_u64).await?;    // Option<User>
let user = User::find_or_fail(&pool, 1_u64).await?;  // User (errors if missing)
let users = User::all(&pool).await?;
let active = User::find_where(&pool, User::query().where_eq("active", true)).await?;

// Insert — returns LAST_INSERT_ID()
let id = User::create(&pool, &[("name", "Alice".into()), ("email", "a@b.com".into())]).await?;

// Update
User::update_by_pk(&pool, 1_u64, &[("active", false.into())]).await?;

// Delete
User::delete_by_pk(&pool, 1_u64).await?;

// Count
let n = User::count(&pool).await?;
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
