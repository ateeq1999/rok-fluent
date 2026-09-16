//! [`SearchService`], [`FilterBuilder`], and [`SortBuilder`] — reusable,
//! composable search/filter/sort building blocks on top of Active Record.
//!
//! Requires a live PostgreSQL database:
//!
//! ```sh
//! DATABASE_URL=postgres://user:pass@localhost/rok_fluent_db \
//!   cargo run --example 08_search_filter_sort --features postgres,active
//! ```
//!
//! # Deviation from the plan
//!
//! The plan suggested `features = "sqlite,active"`. `SearchService`,
//! `FilterBuilder`, and `SortBuilder` (`src/services/{search,filter,sort}.rs`)
//! are all bound to `PgModel` and gated
//! `#[cfg(all(feature = "active", feature = "postgres"))]` in
//! `src/services/mod.rs` — the whole service layer is PostgreSQL-only today,
//! so this example requires a live PostgreSQL database.

use rok_fluent::core::model::Model;
use rok_fluent::orm::postgres::model::PgModel;
use rok_fluent::orm::postgres::pool;
use rok_fluent::services::{FilterBuilder, SearchService, SortBuilder};
use rok_fluent::ModelDerive;

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, ModelDerive)]
#[model(table = "employees")]
pub struct Employee {
    pub id: i64,
    pub name: String,
    pub department: String,
    pub active: bool,
    pub salary_cents: i64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/rok_fluent_db".into());
    let pool = sqlx::PgPool::connect(&database_url).await?;

    sqlx::query("DROP TABLE IF EXISTS employees")
        .execute(&pool)
        .await?;
    sqlx::query(
        "CREATE TABLE employees (
            id           BIGSERIAL PRIMARY KEY,
            name         TEXT NOT NULL,
            department   TEXT NOT NULL,
            active       BOOLEAN NOT NULL DEFAULT TRUE,
            salary_cents BIGINT NOT NULL
        )",
    )
    .execute(&pool)
    .await?;

    for (name, dept, active, salary) in [
        ("Alice Smith", "Engineering", true, 1_200_000_i64),
        ("Bob Jones", "Engineering", true, 950_000),
        ("Carol White", "Sales", false, 800_000),
        ("Dave Brown", "Sales", true, 880_000),
    ] {
        Employee::create(
            &pool,
            &[
                ("name", name.into()),
                ("department", dept.into()),
                ("active", active.into()),
                ("salary_cents", salary.into()),
            ],
        )
        .await?;
    }

    // ── SearchService — ILIKE across declared columns ───────────────────────
    let hits = SearchService::<Employee>::search("smith", &["name"], &pool).await?;
    println!("SearchService \"smith\": {hits:?}");

    // ── FilterBuilder — composable, reusable WHERE clause sets ─────────────
    // `ModelQuery::get()` reads the pool from a task-local scope (normally
    // set per-request by `OrmLayer`); outside Axum, `pool::with_pool` scopes
    // it manually for the duration of the given future.
    let engineering_active = FilterBuilder::<Employee>::new()
        .eq("department", "Engineering")
        .eq("active", true)
        .gte("salary_cents", 900_000_i64);
    let filtered = pool::with_pool(
        pool.clone(),
        engineering_active.apply(Employee::all_query()).get(),
    )
    .await?;
    println!(
        "\nFilterBuilder (active Engineering, salary >= 9000.00): {} row(s)",
        filtered.len()
    );
    for e in &filtered {
        println!("  {} — {}", e.name, e.department);
    }

    // ── SortBuilder — whitelist-validated user-driven sorting ──────────────
    let sort = SortBuilder::<Employee>::new()
        .allow("name")
        .allow("salary_cents")
        .apply_user_input("salary_cents", false) // descending; "not_a_column" would be silently ignored
        .apply_user_input("not_a_column", true);
    let sorted = pool::with_pool(pool.clone(), sort.apply(Employee::all_query()).get()).await?;
    println!("\nSortBuilder (salary_cents DESC):");
    for e in &sorted {
        println!("  {} — {:.2}", e.name, e.salary_cents as f64 / 100.0);
    }

    // ── Combined: paginated search ───────────────────────────────────────────
    let page = SearchService::<Employee>::search_paginated("e", &["name"], 1, 2, &pool).await?;
    println!(
        "\nsearch_paginated(\"e\"): page {} of {}, {} total",
        page.meta.current_page, page.meta.last_page, page.meta.total
    );

    Ok(())
}
