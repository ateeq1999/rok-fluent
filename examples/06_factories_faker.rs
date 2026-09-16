//! Test factories — [`Factory`] + [`FactoryBuilder`] + [`Faker`] — generating
//! fake model instances and persisting them to an in-memory SQLite database.
//!
//! Run with zero setup:
//!
//! ```sh
//! cargo run --example 06_factories_faker --features factory,sqlite,active
//! ```
//!
//! # Deviation from the plan
//!
//! `FactoryBuilder::create` / `create_many` (gated `feature = "factory-postgres"`,
//! `src/factory/mod.rs`) are unfinished stubs today: `create` never executes
//! an `INSERT` against the given pool (the `_pool` parameter is unused and
//! the column/value pairs it builds are discarded — see the `// actual impl
//! wired in Phase 5` comment in the source), and `create_many` just calls
//! `make_many()` without touching the database at all. Using them here would
//! silently mislead readers into thinking rows were persisted when they
//! weren't. Instead, this example uses the always-available `make()` /
//! `make_many()` (which only construct values, no I/O — feature `factory`
//! alone, no backend needed) and then persists the generated data itself via
//! `SqliteModel::create`, which *is* fully implemented.

use rok_fluent::core::model::Model;
use rok_fluent::factory::{Factory, Faker};
use rok_fluent::orm::sqlite::model::SqliteModel;
use rok_fluent::ModelDerive;

#[derive(Debug, Clone, sqlx::FromRow, ModelDerive)]
#[model(table = "customers")]
pub struct Customer {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub signup_score: i64,
}

impl Factory for Customer {
    fn definition() -> Self {
        Self {
            id: 0,
            name: Faker::full_name(),
            email: Faker::email(),
            signup_score: Faker::integer(0, 100),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await?;
    sqlx::query(
        "CREATE TABLE customers (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            name          TEXT NOT NULL,
            email         TEXT NOT NULL,
            signup_score  INTEGER NOT NULL
        )",
    )
    .execute(&pool)
    .await?;

    // A single fake instance with default fake fields.
    let one = Customer::factory().make();
    println!("factory().make(): {one:?}");

    // Many fake instances, with an override applied to every one.
    let vip_customers = Customer::factory()
        .count(5)
        .with(|c| c.signup_score = 100)
        .make_many();
    println!("\nfactory().count(5).with(...).make_many():");
    for c in &vip_customers {
        println!("  {} <{}> score={}", c.name, c.email, c.signup_score);
    }

    // Persist the generated data via the fully-implemented SqliteModel path.
    for c in &vip_customers {
        Customer::create(
            &pool,
            &[
                ("name", c.name.clone().into()),
                ("email", c.email.clone().into()),
                ("signup_score", c.signup_score.into()),
            ],
        )
        .await?;
    }

    let stored: Vec<Customer> = Customer::all(&pool).await?;
    println!("\nStored {} customer(s) in SQLite:", stored.len());
    for c in &stored {
        println!("  #{} {} <{}>", c.id, c.name, c.email);
    }

    Ok(())
}
