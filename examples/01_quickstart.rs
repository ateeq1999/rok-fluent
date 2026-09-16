//! Quickstart — connect to an in-memory SQLite database, derive a [`Model`],
//! and run basic CRUD reads.
//!
//! Run with zero setup:
//!
//! ```sh
//! cargo run --example 01_quickstart --features sqlite,active
//! ```
//!
//! # Deviation from the typed DSL
//!
//! The plan for this example originally called for the Drizzle-style typed
//! DSL (`db::select()...fetch_all()`, feature `query`). In the current
//! codebase the DSL's async terminals (`SelectBuilder::fetch_all`,
//! `fetch_optional`, `fetch_one`, `count`, `exists`, `paginate`) are only
//! implemented for PostgreSQL — see the `#[cfg(feature = "postgres")]` block
//! in `src/dsl/select.rs`. There is no SQLite or MySQL executor for the `dsl`
//! module yet, so a zero-setup SQLite quickstart cannot use `db::select()`.
//!
//! This example instead uses the Active Record style (`active` feature),
//! whose [`SqliteModel`] trait *does* have a full SQLite executor
//! (`src/orm/sqlite/executor.rs`), so it runs against SQLite with no external
//! database required. See `examples/02_crud.rs` for the typed DSL builders
//! (which require PostgreSQL).

use rok_fluent::core::model::Model;
use rok_fluent::orm::sqlite::model::SqliteModel;
use rok_fluent::ModelDerive as Model_;

#[derive(Debug, Clone, sqlx::FromRow, Model_)]
#[model(table = "notes")]
pub struct Note {
    pub id: i64,
    pub title: String,
    pub done: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Zero-setup: an in-memory SQLite database created fresh for this run.
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await?;

    sqlx::query(
        "CREATE TABLE notes (
            id    INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT    NOT NULL,
            done  BOOLEAN NOT NULL DEFAULT 0
        )",
    )
    .execute(&pool)
    .await?;

    Note::create(
        &pool,
        &[
            ("title", "Write the rok-fluent quickstart".into()),
            ("done", false.into()),
        ],
    )
    .await?;
    Note::create(
        &pool,
        &[("title", "Ship examples/".into()), ("done", true.into())],
    )
    .await?;

    let notes: Vec<Note> = Note::all(&pool).await?;
    println!("All notes:");
    for note in &notes {
        println!(
            "  #{} [{}] {}",
            note.id,
            if note.done { "x" } else { " " },
            note.title
        );
    }

    let first: Option<Note> = Note::find_by_pk(&pool, 1_i64).await?;
    println!("\nFirst note by primary key: {first:?}");

    let total = Note::count(&pool).await?;
    println!("Total notes: {total}");

    Ok(())
}
