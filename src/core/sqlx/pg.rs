//! PostgreSQL binding helpers for [`SqlValue`].

use sqlx::postgres::PgArguments;
use sqlx::{query::Query, query::QueryAs, Postgres};

use crate::core::condition::SqlValue;

pub fn bind_value<'q>(
    q: Query<'q, Postgres, PgArguments>,
    v: SqlValue,
) -> Query<'q, Postgres, PgArguments> {
    match v {
        SqlValue::Text(s) => q.bind(s),
        SqlValue::Integer(n) => q.bind(n),
        SqlValue::Float(f) => q.bind(f),
        SqlValue::Bool(b) => q.bind(b),
        SqlValue::Null => q.bind(Option::<String>::None),
    }
}

pub fn bind_value_as<'q, T>(
    q: QueryAs<'q, Postgres, T, PgArguments>,
    v: SqlValue,
) -> QueryAs<'q, Postgres, T, PgArguments>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
    match v {
        SqlValue::Text(s) => q.bind(s),
        SqlValue::Integer(n) => q.bind(n),
        SqlValue::Float(f) => q.bind(f),
        SqlValue::Bool(b) => q.bind(b),
        SqlValue::Null => q.bind(Option::<String>::None),
    }
}

pub fn build_query(sql: &str, params: Vec<SqlValue>) -> Query<'_, Postgres, PgArguments> {
    params
        .into_iter()
        .fold(sqlx::query(sql), |q, v| bind_value(q, v))
}

pub fn build_query_as<T>(sql: &str, params: Vec<SqlValue>) -> QueryAs<'_, Postgres, T, PgArguments>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
    params
        .into_iter()
        .fold(sqlx::query_as::<_, T>(sql), |q, v| bind_value_as(q, v))
}

pub async fn fetch_all_as<T>(
    pool: &sqlx::PgPool,
    sql: &str,
    params: Vec<SqlValue>,
) -> Result<Vec<T>, sqlx::Error>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
{
    build_query_as::<T>(sql, params).fetch_all(pool).await
}

pub async fn fetch_optional_as<T>(
    pool: &sqlx::PgPool,
    sql: &str,
    params: Vec<SqlValue>,
) -> Result<Option<T>, sqlx::Error>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
{
    build_query_as::<T>(sql, params).fetch_optional(pool).await
}

pub async fn execute(
    pool: &sqlx::PgPool,
    sql: &str,
    params: Vec<SqlValue>,
) -> Result<u64, sqlx::Error> {
    let result = build_query(sql, params).execute(pool).await?;
    Ok(result.rows_affected())
}
