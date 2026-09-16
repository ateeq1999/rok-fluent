//! Window functions (`RANK()`, `ROW_NUMBER()`) and CTEs (`WITH … AS (…)`) via
//! the typed DSL.
//!
//! Requires a live PostgreSQL database:
//!
//! ```sh
//! DATABASE_URL=postgres://user:pass@localhost/rok_fluent_db \
//!   cargo run --example 09_window_functions_cte --features postgres,query
//! ```
//!
//! # Why PostgreSQL-only
//!
//! As in `examples/02_crud.rs`, the DSL's async terminals
//! (`SelectBuilder::fetch_all`, etc.) only exist for PostgreSQL
//! (`#[cfg(feature = "postgres")]` in `src/dsl/select.rs`), so this example
//! cannot run against SQLite regardless of window/CTE support.

use rok_fluent::dsl::{db, rank, Window};

#[derive(Debug, sqlx::FromRow, rok_fluent::TableDerive)]
#[table(name = "employees")]
pub struct Employee {
    pub id: i64,
    pub name: String,
    pub department: String,
    pub salary_cents: i64,
}

/// Row shape returned by the ranked query: the employee's name plus a
/// window-computed rank column, aliased `"dept_rank"` in the projection.
#[derive(Debug, sqlx::FromRow)]
pub struct RankedEmployee {
    pub name: String,
    pub department: String,
    pub dept_rank: i64,
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
            salary_cents BIGINT NOT NULL
        )",
    )
    .execute(&pool)
    .await?;

    for (name, dept, salary) in [
        ("Alice", "Engineering", 1_200_000_i64),
        ("Bob", "Engineering", 950_000),
        ("Carol", "Engineering", 1_100_000),
        ("Dave", "Sales", 880_000),
        ("Eve", "Sales", 990_000),
    ] {
        db::insert_into(Employee::table())
            .values([("name", name), ("department", dept)])
            .execute(&pool)
            .await?;
        db::update(Employee::table())
            .set("salary_cents", salary)
            .where_(Employee::NAME.eq(name))
            .execute(&pool)
            .await?;
    }

    // ── Window function: RANK() OVER (PARTITION BY department ORDER BY salary DESC) ──
    let ranked: Vec<RankedEmployee> = db::select()
        .from(Employee::table())
        .columns(["name", "department"])
        .win_col(
            rank()
                .over(
                    Window::new()
                        .partition_by(Employee::DEPARTMENT)
                        .order_by(Employee::SALARY_CENTS.desc()),
                )
                .alias("dept_rank"),
        )
        .order_by(Employee::DEPARTMENT.asc())
        .fetch_all(&pool)
        .await?;

    println!("Ranked by department (highest salary first):");
    for row in &ranked {
        println!("  [{}] #{} {}", row.department, row.dept_rank, row.name);
    }

    // ── CTE: WITH top_earners AS (…) SELECT … FROM top_earners ─────────────
    let top_earners_cte = db::select()
        .from(Employee::table())
        .where_(Employee::SALARY_CENTS.gt(900_000_i64))
        .order_by(Employee::SALARY_CENTS.desc());

    let via_cte: Vec<Employee> = db::select()
        .with_cte("top_earners", top_earners_cte)
        .from_cte("top_earners")
        .fetch_all(&pool)
        .await?;

    println!("\nVia CTE \"top_earners\" (salary > 9000.00):");
    for e in &via_cte {
        println!("  {} — {:.2}", e.name, e.salary_cents as f64 / 100.0);
    }

    Ok(())
}
