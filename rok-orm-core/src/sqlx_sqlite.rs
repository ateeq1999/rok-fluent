//! SQLite binding helpers for [`SqlValue`] and [`QueryBuilder`].
//!
//! Enable with `features = ["sqlx-sqlite"]`.
//!
//! ```toml
//! rok-orm-core = { version = "0.1", features = ["sqlx-sqlite"] }
//! ```

use sqlx::sqlite::SqliteArguments;
use sqlx::{query::Query, query::QueryAs, Sqlite};

use crate::SqlValue;

// ── low-level binders ─────────────────────────────────────────────────────────

/// Bind a single [`SqlValue`] to an in-progress sqlx SQLite query.
pub fn bind_value<'q>(
    q: Query<'q, Sqlite, SqliteArguments<'q>>,
    v: SqlValue,
) -> Query<'q, Sqlite, SqliteArguments<'q>> {
    match v {
        SqlValue::Text(s) => q.bind(s),
        SqlValue::Integer(n) => q.bind(n),
        SqlValue::Float(f) => q.bind(f),
        SqlValue::Bool(b) => q.bind(b),
        SqlValue::Null => q.bind(Option::<String>::None),
    }
}

/// Bind a single [`SqlValue`] to an in-progress sqlx `query_as` SQLite query.
pub fn bind_value_as<'q, T>(
    q: QueryAs<'q, Sqlite, T, SqliteArguments<'q>>,
    v: SqlValue,
) -> QueryAs<'q, Sqlite, T, SqliteArguments<'q>>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow>,
{
    match v {
        SqlValue::Text(s) => q.bind(s),
        SqlValue::Integer(n) => q.bind(n),
        SqlValue::Float(f) => q.bind(f),
        SqlValue::Bool(b) => q.bind(b),
        SqlValue::Null => q.bind(Option::<String>::None),
    }
}

// ── convenience builders ──────────────────────────────────────────────────────

/// Build a sqlx SQLite query from a SQL string + parameter list.
pub fn build_query<'q>(
    sql: &'q str,
    params: Vec<SqlValue>,
) -> Query<'q, Sqlite, SqliteArguments<'q>> {
    params
        .into_iter()
        .fold(sqlx::query(sql), |q, v| bind_value(q, v))
}

/// Build a typed sqlx `query_as` from a SQL string + parameter list.
pub fn build_query_as<'q, T>(
    sql: &'q str,
    params: Vec<SqlValue>,
) -> QueryAs<'q, Sqlite, T, SqliteArguments<'q>>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow>,
{
    params
        .into_iter()
        .fold(sqlx::query_as::<_, T>(sql), |q, v| bind_value_as(q, v))
}

// ── high-level executors ──────────────────────────────────────────────────────

/// Fetch all rows matching the given SQL and parameters.
pub async fn fetch_all_as<T>(
    pool: &sqlx::SqlitePool,
    sql: &str,
    params: Vec<SqlValue>,
) -> Result<Vec<T>, sqlx::Error>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> + Send + Unpin,
{
    build_query_as::<T>(sql, params).fetch_all(pool).await
}

/// Fetch at most one row.  Returns `None` if no rows matched.
pub async fn fetch_optional_as<T>(
    pool: &sqlx::SqlitePool,
    sql: &str,
    params: Vec<SqlValue>,
) -> Result<Option<T>, sqlx::Error>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> + Send + Unpin,
{
    build_query_as::<T>(sql, params).fetch_optional(pool).await
}

/// Execute a SQL statement and return the number of rows affected.
pub async fn execute(
    pool: &sqlx::SqlitePool,
    sql: &str,
    params: Vec<SqlValue>,
) -> Result<u64, sqlx::Error> {
    let result = build_query(sql, params).execute(pool).await?;
    Ok(result.rows_affected())
}
