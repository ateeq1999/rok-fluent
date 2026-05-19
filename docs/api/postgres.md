# API: PostgreSQL (`rok_fluent::orm::postgres`) — feature: `postgres`

```toml
rok-fluent = { version = "0.4", features = ["postgres"] }
```

## Pool

```rust,no_run
use rok_fluent::orm::postgres::pool;
use sqlx::PgPool;

// Store the pool in a task-local at startup
pool::set(PgPool::connect("postgres://localhost/mydb").await?);

// Retrieve it inside async tasks
let pool: &PgPool = pool::get()?;

// Read-replica pool (feature: replica)
pool::set_replica(PgPool::connect("postgres://replica/mydb").await?);
let replica: &PgPool = pool::get_replica().unwrap_or_else(|_| pool::get().unwrap());
```

---

## `PgModel` CRUD Trait

```rust,no_run
use rok_fluent::orm::postgres::model::PgModel;

// All methods are async and return Result<_, sqlx::Error>

// Find
let user = User::find(1_i64).await?;                        // Option<User>
let user = User::find_or_fail(1_i64).await?;                // User  (404 error if missing)
let users = User::all().await?;                             // Vec<User>
let users = User::query().where_eq("active", true).all().await?;

// Insert
let id = User::insert(&[("name", "Alice".into()), ("email", "a@b.com".into())]).await?;

// Insert with RETURNING
let user: User = User::insert_returning(&[("name", "Alice".into())]).await?;

// Bulk insert
let count = User::bulk_insert(&[row1, row2, row3]).await?;

// Update
let rows = User::update_where(
    &[("active", false.into())],                // SET
    &[("id", 1_i64.into())],                    // WHERE
).await?;

// Upsert (INSERT … ON CONFLICT DO UPDATE)
User::upsert(&[("email", "a@b.com".into()), ("name", "Alice 2".into())], "email").await?;

// Delete
let rows = User::delete_where(&[("id", 1_i64.into())]).await?;

// Soft delete (sets deleted_at; model must have #[rok_orm(soft_delete)])
let rows = User::soft_delete_where(&[("id", 1_i64.into())]).await?;
let rows = User::restore_where(&[("id", 1_i64.into())]).await?;   // clears deleted_at

// Count
let n = User::count_where(&[("active", true.into())]).await?;

// Exists
let exists = User::exists_where(&[("email", "a@b.com".into())]).await?;

// Raw SQL
let rows: Vec<User> = User::raw("SELECT * FROM users WHERE score > $1", vec![100_i64.into()]).await?;
let affected = User::execute_sql("DELETE FROM users WHERE created_at < $1", vec![cutoff.into()]).await?;
```

---

## Executor (`rok_fluent::orm::postgres::executor`)

Lower-level query runner used internally. Exposed for advanced use cases.

```rust,no_run
use rok_fluent::orm::postgres::executor;

let rows = executor::fetch_all::<User>(&pool, &sql, params).await?;
let row = executor::fetch_one::<User>(&pool, &sql, params).await?;
let id = executor::insert(&pool, &sql, params).await?;
let n = executor::execute(&pool, &sql, params).await?;
let rows = executor::bulk_insert::<User>(&pool, "users", &batch).await?;
```

---

## Transactions (`rok_fluent::orm::postgres::transaction`)

```rust,no_run
use rok_fluent::orm::postgres::transaction::Tx;

// Run a closure in a transaction; rolls back on any error
let result: MyOutput = Tx::run(|tx| async move {
    let id = User::insert_in_tx(&tx, &[("name", "Alice".into())]).await?;
    Account::insert_in_tx(&tx, &[("user_id", id.into())]).await?;
    Ok(id)
})
.await?;

// Nested savepoints
Tx::run(|tx| async move {
    Tx::savepoint(&tx, "sp1", |sp| async move {
        // … if this block errors, only sp1 is rolled back
        Ok(())
    })
    .await?;
    Ok(())
})
.await?;
```

---

## Pivot Queries (`rok_fluent::orm::postgres::pivot_query`)

Many-to-many through a junction table.

```rust,no_run
use rok_fluent::orm::postgres::pivot_query::PivotQuery;

// Load all Tags for a Post via post_tags junction table
let tags = PivotQuery::<Post, Tag>::new("post_tags", "post_id", "tag_id")
    .for_id(post_id)
    .all()
    .await?;

// Attach / detach
PivotQuery::<Post, Tag>::new("post_tags", "post_id", "tag_id")
    .attach(post_id, &[tag_id_1, tag_id_2])
    .await?;

PivotQuery::<Post, Tag>::new("post_tags", "post_id", "tag_id")
    .detach(post_id, &[tag_id_1])
    .await?;

PivotQuery::<Post, Tag>::new("post_tags", "post_id", "tag_id")
    .sync(post_id, &[tag_id_1, tag_id_3])   // replace all associations
    .await?;
```

---

## Query Log (`rok_fluent::orm::postgres::query_log`) — feature: `tracing`

Enabled automatically when `tracing` feature is active. Every query emits a `DEBUG`
span with fields:

| Span field | Value |
|---|---|
| `db.statement` | SQL string |
| `db.operation` | `SELECT` / `INSERT` / `UPDATE` / `DELETE` |
| `db.table` | model's `table_name()` |
| `db.rows_affected` | result row count |
| `db.duration_ms` | wall-clock time |
