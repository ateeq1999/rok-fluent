//! Schema migrations — [`Schema`] DDL builder, the async [`Migration`] trait,
//! and [`MigrationRunner`].
//!
//! Requires a live PostgreSQL database:
//!
//! ```sh
//! DATABASE_URL=postgres://user:pass@localhost/rok_fluent_db \
//!   cargo run --example 05_migrations --features migrate-postgres
//! ```
//!
//! # Deviation from the plan
//!
//! The plan asked to pick "whichever [of `migrate-sqlite` / `migrate-postgres`]
//! is simpler to demo end-to-end." In the current codebase there is only one
//! choice: `MigrationRunner`, `SchemaExecutor`, and the async `Migration`
//! trait (`src/migrate/runner.rs`, `src/migrate/schema.rs`,
//! `src/migrate/migration.rs`) are all `#[cfg(feature = "postgres")]`, even
//! though `migrate-sqlite` and `migrate-mysql` feature flags exist in
//! `Cargo.toml`. Enabling `migrate-sqlite` alone gives you the synchronous
//! `Schema` DDL string builder (SQL generation only) but no executor to run
//! migrations against a live SQLite database and no `MigrationRunner` at
//! all — so a migrations example can only run end-to-end with
//! `migrate-postgres`.

use async_trait::async_trait;
use rok_fluent::migrate::{Migration, MigrationRunner, Schema, SchemaExecutor};

pub struct CreateArticlesTable;

#[async_trait]
impl Migration for CreateArticlesTable {
    fn name(&self) -> &str {
        "2026_01_01_000001_create_articles_table"
    }

    async fn up(&self, schema: &SchemaExecutor) -> anyhow::Result<()> {
        schema
            .create("articles", |t| {
                t.id();
                t.string("title").not_null();
                t.string("body").nullable();
                t.timestamps();
            })
            .await
    }

    async fn down(&self, schema: &SchemaExecutor) -> anyhow::Result<()> {
        schema.drop_table_if_exists("articles").await
    }
}

pub struct AddPublishedColumn;

#[async_trait]
impl Migration for AddPublishedColumn {
    fn name(&self) -> &str {
        "2026_01_02_000001_add_published_to_articles"
    }

    async fn up(&self, schema: &SchemaExecutor) -> anyhow::Result<()> {
        schema
            .alter_table("articles", |t| {
                t.add_column(|c| {
                    c.boolean("published").not_null().default("false");
                });
            })
            .await
    }

    async fn down(&self, schema: &SchemaExecutor) -> anyhow::Result<()> {
        schema
            .alter_table("articles", |t| {
                t.drop_column("published");
            })
            .await
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/rok_fluent_db".into());
    let pool = sqlx::PgPool::connect(&database_url).await?;

    // The `Schema` builder can also render SQL without a live connection —
    // useful for inspecting what a migration will do before running it.
    let preview = Schema::create("articles", |t| {
        t.id();
        t.string("title").not_null();
    })
    .to_sql();
    println!("Preview SQL:\n{preview}\n");

    let runner = MigrationRunner::new(pool.clone())
        .migration(CreateArticlesTable)
        .migration(AddPublishedColumn);

    runner.status().await?;
    runner.run().await?;
    println!("\nMigrations applied.");
    runner.status().await?;

    // Roll back the most recent batch (both migrations, since they ran together).
    runner.rollback().await?;
    println!("\nRolled back last batch.");
    runner.status().await?;

    Ok(())
}
