//! MySQL binding helpers for [`SqlValue`].

use sqlx::mysql::MySqlArguments;
use sqlx::{query::Query, query::QueryAs, MySql};

use crate::core::condition::SqlValue;

pub fn bind_value<'q>(
    q: Query<'q, MySql, MySqlArguments>,
    v: SqlValue,
) -> Query<'q, MySql, MySqlArguments> {
    match v {
        SqlValue::Text(s) => q.bind(s),
        SqlValue::Integer(n) => q.bind(n),
        SqlValue::Float(f) => q.bind(f),
        SqlValue::Bool(b) => q.bind(b),
        SqlValue::Null => q.bind(Option::<String>::None),
    }
}

pub fn bind_value_as<'q, T>(
    q: QueryAs<'q, MySql, T, MySqlArguments>,
    v: SqlValue,
) -> QueryAs<'q, MySql, T, MySqlArguments>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow>,
{
    match v {
        SqlValue::Text(s) => q.bind(s),
        SqlValue::Integer(n) => q.bind(n),
        SqlValue::Float(f) => q.bind(f),
        SqlValue::Bool(b) => q.bind(b),
        SqlValue::Null => q.bind(Option::<String>::None),
    }
}

pub fn build_query<'q>(sql: &'q str, params: Vec<SqlValue>) -> Query<'q, MySql, MySqlArguments> {
    params
        .into_iter()
        .fold(sqlx::query(sql), |q, v| bind_value(q, v))
}

pub async fn fetch_all_as<T>(
    pool: &sqlx::MySqlPool,
    sql: &str,
    params: Vec<SqlValue>,
) -> Result<Vec<T>, sqlx::Error>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
{
    let mut q = sqlx::query_as::<MySql, T>(sql);
    for v in params {
        q = bind_value_as(q, v);
    }
    q.fetch_all(pool).await
}

pub async fn fetch_optional_as<T>(
    pool: &sqlx::MySqlPool,
    sql: &str,
    params: Vec<SqlValue>,
) -> Result<Option<T>, sqlx::Error>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
{
    let mut q = sqlx::query_as::<MySql, T>(sql);
    for v in params {
        q = bind_value_as(q, v);
    }
    q.fetch_optional(pool).await
}

pub async fn execute(
    pool: &sqlx::MySqlPool,
    sql: &str,
    params: Vec<SqlValue>,
) -> Result<u64, sqlx::Error> {
    let result = build_query(sql, params).execute(pool).await?;
    Ok(result.rows_affected())
}
