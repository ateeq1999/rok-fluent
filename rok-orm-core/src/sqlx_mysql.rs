//! MySQL binding helpers for [`SqlValue`] and [`QueryBuilder`].
//!
//! Enable with `features = ["sqlx-mysql"]`.

use sqlx::mysql::MySqlArguments;
use sqlx::{query::Query, query::QueryAs, MySql};

use crate::SqlValue;

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

/// Build a raw query with `?` positional params.
pub fn build_query<'q>(sql: &'q str, params: Vec<SqlValue>) -> Query<'q, MySql, MySqlArguments> {
    let mut q = sqlx::query(sql);
    for v in params {
        q = bind_value(q, v);
    }
    q
}

/// Execute a SELECT and collect all rows.
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

/// Execute a SELECT and return at most one row.
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

/// Execute a statement and return affected rows.
pub async fn execute(
    pool: &sqlx::MySqlPool,
    sql: &str,
    params: Vec<SqlValue>,
) -> Result<u64, sqlx::Error> {
    let result = build_query(sql, params).execute(pool).await?;
    Ok(result.rows_affected())
}
