//! SQLite binding helpers for [`SqlValue`].

use sqlx::sqlite::SqliteArguments;
use sqlx::{query::Query, query::QueryAs, Sqlite};

use crate::core::condition::SqlValue;

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
        // SQLite has no native JSON/UUID types; store as text.
        SqlValue::Json(j) => q.bind(j.to_string()),
        SqlValue::Uuid(u) => q.bind(u.to_string()),
    }
}

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
        SqlValue::Json(j) => q.bind(j.to_string()),
        SqlValue::Uuid(u) => q.bind(u.to_string()),
    }
}

pub fn build_query<'q>(
    sql: &'q str,
    params: Vec<SqlValue>,
) -> Query<'q, Sqlite, SqliteArguments<'q>> {
    params
        .into_iter()
        .fold(sqlx::query(sql), |q, v| bind_value(q, v))
}

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

pub async fn execute(
    pool: &sqlx::SqlitePool,
    sql: &str,
    params: Vec<SqlValue>,
) -> Result<u64, sqlx::Error> {
    let result = build_query(sql, params).execute(pool).await?;
    Ok(result.rows_affected())
}
