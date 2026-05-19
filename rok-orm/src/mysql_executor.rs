//! Async MySQL executor — runs queries against a live pool.
//!
//! Enable with `features = ["mysql"]`.
//!
//! SQL uses `?` positional placeholders (same as SQLite dialect).

use rok_orm_core::{sqlx_mysql, Dialect, Model, QueryBuilder, SqlValue};
use sqlx::mysql::MySqlRow;
use sqlx::MySqlPool;

pub async fn fetch_all<T>(pool: &MySqlPool, builder: QueryBuilder<T>) -> Result<Vec<T>, sqlx::Error>
where
    T: Model + for<'r> sqlx::FromRow<'r, MySqlRow> + Send + Unpin,
{
    let (sql, params) = builder.to_sql_with_dialect(Dialect::Sqlite);
    sqlx_mysql::fetch_all_as::<T>(pool, &sql, params).await
}

pub async fn fetch_optional<T>(
    pool: &MySqlPool,
    builder: QueryBuilder<T>,
) -> Result<Option<T>, sqlx::Error>
where
    T: Model + for<'r> sqlx::FromRow<'r, MySqlRow> + Send + Unpin,
{
    let (sql, params) = builder.to_sql_with_dialect(Dialect::Sqlite);
    sqlx_mysql::fetch_optional_as::<T>(pool, &sql, params).await
}

pub async fn count<T>(pool: &MySqlPool, builder: QueryBuilder<T>) -> Result<i64, sqlx::Error> {
    let (sql, params) = builder.to_count_sql_with_dialect(Dialect::Sqlite);
    let row = sqlx_mysql::build_query(&sql, params)
        .fetch_one(pool)
        .await?;
    use sqlx::Row;
    // MySQL returns COUNT(*) as i64
    row.try_get::<i64, _>(0)
}

pub async fn execute_raw(
    pool: &MySqlPool,
    sql: &str,
    params: Vec<SqlValue>,
) -> Result<u64, sqlx::Error> {
    sqlx_mysql::execute(pool, sql, params).await
}

/// INSERT INTO `table` (cols) VALUES (?) returning the LAST_INSERT_ID.
pub async fn insert(
    pool: &MySqlPool,
    table: &str,
    data: &[(&str, SqlValue)],
) -> Result<i64, sqlx::Error> {
    let cols: Vec<&str> = data.iter().map(|(c, _)| *c).collect();
    let placeholders = (0..data.len()).map(|_| "?").collect::<Vec<_>>().join(", ");
    let sql = format!(
        "INSERT INTO `{table}` ({}) VALUES ({placeholders})",
        cols.join(", ")
    );

    let mut q = sqlx::query(&sql);
    for (_, val) in data {
        q = sqlx_mysql::bind_value(q, val.clone());
    }
    let result = q.execute(pool).await?;
    Ok(result.last_insert_id() as i64)
}

/// UPDATE `table` SET … WHERE `pk_col` = ? returning affected rows.
pub async fn update(
    pool: &MySqlPool,
    table: &str,
    pk_col: &str,
    pk_val: SqlValue,
    data: &[(&str, SqlValue)],
) -> Result<u64, sqlx::Error> {
    let set: Vec<String> = data.iter().map(|(c, _)| format!("`{c}` = ?")).collect();
    let sql = format!(
        "UPDATE `{table}` SET {} WHERE `{pk_col}` = ?",
        set.join(", ")
    );

    let mut params: Vec<SqlValue> = data.iter().map(|(_, v)| v.clone()).collect();
    params.push(pk_val);
    sqlx_mysql::execute(pool, &sql, params).await
}

/// DELETE FROM `table` WHERE `pk_col` = ?
pub async fn delete(
    pool: &MySqlPool,
    table: &str,
    pk_col: &str,
    pk_val: SqlValue,
) -> Result<u64, sqlx::Error> {
    let sql = format!("DELETE FROM `{table}` WHERE `{pk_col}` = ?");
    sqlx_mysql::execute(pool, &sql, vec![pk_val]).await
}

/// SELECT COUNT(*) for a given builder — same as `count` but public alias.
pub async fn aggregate(
    pool: &MySqlPool,
    table: &str,
    expr: &str,
    where_sql: &str,
    params: Vec<SqlValue>,
) -> Result<Option<f64>, sqlx::Error> {
    let sql = if where_sql.is_empty() {
        format!("SELECT {expr} FROM `{table}`")
    } else {
        format!("SELECT {expr} FROM `{table}` WHERE {where_sql}")
    };
    let row = sqlx_mysql::build_query(&sql, params)
        .fetch_optional(pool)
        .await?;
    match row {
        None => Ok(None),
        Some(r) => {
            use sqlx::Row;
            let v: Option<f64> = r.try_get(0).ok();
            Ok(v)
        }
    }
}
