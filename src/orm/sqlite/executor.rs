//! Async SQLite executor — runs [`QueryBuilder`] output against a live pool.
//!
//! Requires the `sqlite` feature:
//!
//! ```toml
//! rok-fluent = { version = "0.4", features = ["sqlite"] }
//! ```
//!
//! All SQL is generated with `?` placeholders via [`Dialect::Sqlite`].

use sqlx::sqlite::SqliteRow;
use sqlx::SqlitePool;

use crate::core::condition::SqlValue;
use crate::core::model::Model;
use crate::core::query::{Dialect, QueryBuilder};
use crate::core::sqlx::sqlite as sqlx_sqlite;

/// Fetch all rows matching the query.
pub async fn fetch_all<T>(
    pool: &SqlitePool,
    builder: QueryBuilder<T>,
) -> Result<Vec<T>, sqlx::Error>
where
    T: Model + for<'r> sqlx::FromRow<'r, SqliteRow> + Send + Unpin,
{
    let (sql, params) = builder.to_sql_with_dialect(Dialect::Sqlite);
    sqlx_sqlite::fetch_all_as::<T>(pool, &sql, params).await
}

/// Fetch at most one row matching the query.  Returns `None` if no rows match.
pub async fn fetch_optional<T>(
    pool: &SqlitePool,
    builder: QueryBuilder<T>,
) -> Result<Option<T>, sqlx::Error>
where
    T: Model + for<'r> sqlx::FromRow<'r, SqliteRow> + Send + Unpin,
{
    let (sql, params) = builder.to_sql_with_dialect(Dialect::Sqlite);
    sqlx_sqlite::fetch_optional_as::<T>(pool, &sql, params).await
}

/// Return the row count matching the query's WHERE clause.
pub async fn count<T>(pool: &SqlitePool, builder: QueryBuilder<T>) -> Result<i64, sqlx::Error> {
    let (sql, params) = builder.to_count_sql_with_dialect(Dialect::Sqlite);
    let row = sqlx_sqlite::build_query(&sql, params)
        .fetch_one(pool)
        .await?;
    use sqlx::Row;
    row.try_get::<i64, _>(0)
}

/// Execute a raw SQL string and return rows affected.
pub async fn execute_raw(
    pool: &SqlitePool,
    sql: &str,
    params: Vec<SqlValue>,
) -> Result<u64, sqlx::Error> {
    sqlx_sqlite::execute(pool, sql, params).await
}

/// Insert a row and return rows affected.
pub async fn insert<T>(
    pool: &SqlitePool,
    table: &str,
    data: &[(&str, SqlValue)],
) -> Result<u64, sqlx::Error> {
    let (sql, params) = QueryBuilder::<T>::insert_sql_with_dialect(Dialect::Sqlite, table, data);
    execute_raw(pool, &sql, params).await
}

/// Update rows matching the builder's conditions and return rows affected.
pub async fn update<T>(
    pool: &SqlitePool,
    builder: QueryBuilder<T>,
    data: &[(&str, SqlValue)],
) -> Result<u64, sqlx::Error> {
    let (sql, params) = builder.to_update_sql_with_dialect(Dialect::Sqlite, data);
    execute_raw(pool, &sql, params).await
}

/// Delete rows matching the builder's conditions and return rows affected.
pub async fn delete<T>(pool: &SqlitePool, builder: QueryBuilder<T>) -> Result<u64, sqlx::Error> {
    let (sql, params) = builder.to_delete_sql_with_dialect(Dialect::Sqlite);
    execute_raw(pool, &sql, params).await
}

/// Insert a single row and return it via `RETURNING *`.
///
/// Requires SQLite 3.35+ (released 2021-03-12).
pub async fn insert_returning<T>(
    pool: &SqlitePool,
    table: &str,
    data: &[(&str, SqlValue)],
) -> Result<T, sqlx::Error>
where
    T: Model + for<'r> sqlx::FromRow<'r, SqliteRow> + Send + Unpin,
{
    let (base_sql, params) =
        QueryBuilder::<T>::insert_sql_with_dialect(Dialect::Sqlite, table, data);
    let sql = format!("{base_sql} RETURNING *");
    sqlx_sqlite::fetch_all_as::<T>(pool, &sql, params)
        .await?
        .into_iter()
        .next()
        .ok_or(sqlx::Error::RowNotFound)
}
