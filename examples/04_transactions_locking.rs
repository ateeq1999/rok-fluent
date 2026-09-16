//! Transactions with savepoints ([`TransactionService`]) and advisory locking
//! ([`LockService`]) — both PostgreSQL-specific.
//!
//! Requires a live PostgreSQL database:
//!
//! ```sh
//! DATABASE_URL=postgres://user:pass@localhost/rok_fluent_db \
//!   cargo run --example 04_transactions_locking --features postgres,active
//! ```
//!
//! # Deviations from the plan / README
//!
//! - `LockService::acquire` takes an `i64` key, not a `&str` as shown in
//!   `README.md`'s snippet (`LockService::acquire("deploy_lock", &pool)`).
//!   `pg_advisory_lock` takes a 64-bit integer key, so this example hashes a
//!   human-readable name down to an `i64`.
//! - `TransactionService::begin` returns a `TxCtx`, whose `.create::<T>(...)`
//!   needs a turbofish (or inference from a `let` binding's type) to pick the
//!   model — plain `tx.create(&[...])` as shown in the README does not
//!   compile on its own.

use rok_fluent::core::model::Model;
use rok_fluent::orm::postgres::model::PgModel;
use rok_fluent::services::{LockService, TransactionService};
use rok_fluent::ModelDerive;

#[derive(Debug, Clone, sqlx::FromRow, ModelDerive)]
#[model(table = "accounts")]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub balance_cents: i64,
}

/// Turn a human-readable lock name into a stable `i64` key for
/// `pg_advisory_lock`, which only accepts integer keys.
fn lock_key(name: &str) -> i64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hasher);
    hasher.finish() as i64
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/rok_fluent_db".into());
    let pool = sqlx::PgPool::connect(&database_url).await?;

    sqlx::query("DROP TABLE IF EXISTS accounts")
        .execute(&pool)
        .await?;
    sqlx::query(
        "CREATE TABLE accounts (
            id            BIGSERIAL PRIMARY KEY,
            name          TEXT NOT NULL,
            balance_cents BIGINT NOT NULL
        )",
    )
    .execute(&pool)
    .await?;

    // ── Transaction with savepoints ─────────────────────────────────────────
    let mut tx = TransactionService::begin(&pool).await?;

    let alice: Account = tx
        .create_returning(&[
            ("name", "Alice".into()),
            ("balance_cents", 10_000_i64.into()),
        ])
        .await?;
    tx.savepoint("after_alice").await?;

    let bob_created = tx
        .create::<Account>(&[("name", "Bob".into()), ("balance_cents", (-500_i64).into())])
        .await;
    if bob_created.is_err() {
        // A negative balance in a real schema might violate a CHECK
        // constraint; demonstrate reverting to the savepoint instead of
        // aborting the whole transaction.
        tx.rollback_to("after_alice").await?;
    }
    tx.release("after_alice").await?;

    tx.update_by_pk::<Account>(alice.id, &[("balance_cents", 12_000_i64.into())])
        .await?;
    tx.commit().await?;
    println!("Committed transaction — Alice's account created and updated.");

    // ── Advisory lock ────────────────────────────────────────────────────────
    let key = lock_key("nightly_reconciliation_job");
    LockService::acquire(key, &pool).await?;
    println!("Acquired advisory lock {key} — simulating a critical section.");
    let all: Vec<Account> = Account::all(&pool).await?;
    println!("Read {} account(s) while holding the lock.", all.len());
    LockService::release(key, &pool).await?;
    println!("Released advisory lock {key}.");

    // ── Non-blocking try-acquire ─────────────────────────────────────────────
    let got_lock = LockService::try_acquire(key, &pool).await?;
    println!("try_acquire while free: {got_lock}");
    if got_lock {
        LockService::release(key, &pool).await?;
    }

    Ok(())
}
