//! Test factories — [`Factory`] + [`FactoryBuilder`] + [`Faker`] — generating
//! fake model instances and persisting them to an in-memory SQLite database.
//!
//! Run with zero setup:
//!
//! ```sh
//! cargo run --example 06_factories_faker --features factory,sqlite,active
//! ```

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

    // A single fake instance with default fake fields — `make()` only builds
    // the value in memory, no I/O.
    let one = Customer::factory().make();
    println!("factory().make(): {one:?}");

    // Build-and-persist one row in a single call — `create()` inserts via
    // `INSERT … RETURNING *` and hands back the row as stored (id included).
    let inserted = Customer::factory()
        .with(|c| c.name = "Solo Customer".into())
        .create(&pool)
        .await?;
    println!(
        "\nfactory().create(&pool): #{} {}",
        inserted.id, inserted.name
    );

    // Many fake instances, with an override applied to every one, built and
    // persisted together via `create_many()`.
    let vip_customers = Customer::factory()
        .count(5)
        .with(|c| c.signup_score = 100)
        .create_many(&pool)
        .await?;
    println!("\nfactory().count(5).with(...).create_many(&pool):");
    for c in &vip_customers {
        println!(
            "  #{} {} <{}> score={}",
            c.id, c.name, c.email, c.signup_score
        );
    }

    let stored: Vec<Customer> = Customer::all(&pool).await?;
    println!("\nStored {} customer(s) in SQLite:", stored.len());
    for c in &stored {
        println!("  #{} {} <{}>", c.id, c.name, c.email);
    }

    Ok(())
}
