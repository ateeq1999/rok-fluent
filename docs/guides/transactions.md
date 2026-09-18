# Transactions

rok-fluent provides two transaction APIs:

- **`Tx`** — low-level wrapper around `sqlx::Transaction` (`src/orm/postgres/transaction.rs`)
- **`TransactionService`** — higher-level API with savepoints and CRUD helpers (`src/services/transaction.rs`)

Both are gated behind `features = ["active", "postgres"]`.

See [`examples/04_transactions_locking.rs`](../../examples/04_transactions_locking.rs)
for a runnable version covering `TransactionService` savepoints alongside advisory
locking.

## `TransactionService`

### Begin a transaction

```rust,ignore
use rok_fluent::services::TransactionService;

let mut tx = TransactionService::begin(&pool).await?;
```

### Savepoints

Named savepoints inside the transaction:

```rust,ignore
tx.savepoint("sp1").await?;

// Do work that might fail…

tx.rollback_to("sp1").await?;   // undo to savepoint
tx.release("sp1").await?;        // discard savepoint
```

### CRUD inside a transaction

All operations use the transaction handle — no pool parameter required:

```rust,ignore
// Create
let user = tx.create::<User>(&[("name", "Alice".into()), ("email", "alice@example.com".into())]).await?;

// Create and return
let user = tx.create_returning::<User>(&[("name", "Bob".into())]).await?;

// Update by primary key
let user = tx.update_by_pk::<User>(42_i64, &[("name", "Charlie".into())]).await?;

// Update with a condition — `TxCtx::update`/`::delete` take a `ModelQuery<T>`,
// so build the filter with `User::filter(...)`, not `User::query()`.
let n = tx.update::<User>(
    User::filter("active", true),
    &[("status", "inactive".into())],
).await?;

// Delete
let n = tx.delete_by_pk::<User>(42_i64).await?;
let n = tx.delete::<User>(User::filter("status", "guest")).await?;
```

### Raw queries

```rust,ignore
let rows: Vec<User> = tx.fetch_all("SELECT * FROM users WHERE active = $1", &[true.into()]).await?;
let row: Option<User> = tx.fetch_optional("SELECT * FROM users WHERE id = $1", &[42_i64.into()]).await?;
```

### Commit / Rollback

```rust,ignore
tx.commit().await?;
// or
tx.rollback().await?;
```

## `Tx` (low-level)

The `Tx` struct wraps `sqlx::Transaction` directly. Use it when you need
fine-grained control or retry logic.

`Tx` has no primary-key convenience methods (`find_by_pk`/`update_by_pk`/`delete_by_pk` are
`TransactionService`-only) — build a `QueryBuilder<T>` filter via `Model::query()` or
`Model::find(id)` instead:

```rust,ignore
use rok_fluent::orm::postgres::transaction::Tx;
use rok_fluent::core::model::Model;

let mut tx = Tx::begin(&pool).await?;

// Pool-free CRUD — table name + data pairs, and a `QueryBuilder<T>` for the WHERE clause
let user: User = tx
    .insert_returning("users", &[("name", "Dave".into())])
    .await?;
let found: Option<User> = tx.fetch_optional(User::find(42_i64)).await?;
tx.update(User::query().where_eq("id", 42_i64), &[("name", "Eve".into())]).await?;
tx.delete(User::query().where_eq("id", 42_i64)).await?;

tx.commit().await?;
```

### Retry on serialisation failure

```rust,ignore
use rok_fluent::orm::postgres::executor::RetryConfig;

let config = RetryConfig::default();
Tx::run_with_retry(&pool, &config, |tx| async move {
    // Transaction body — retries up to `config.max_attempts` times on serialisation errors
    let user: User = tx.insert_returning("users", &[("name", "RetryUser".into())]).await?;
    Ok(user)
}).await?;
```

## Choosing between `Tx` and `TransactionService`

| Use case | API |
|---|---|
| Wrapping existing code in a transaction | `Tx::begin` / `TransactionService::begin` |
| Savepoints | `TransactionService` |
| CRUD inside transaction | Both — `TransactionService` has `fetch_all`/`fetch_optional` as well |
| Retry logic | `Tx::run_with_retry` |
| Raw SQL queries | `TransactionService` |

## Feature flag

```
rok-fluent = { version = "0.4", features = ["active", "postgres"] }
```
