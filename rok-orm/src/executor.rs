//! Async PostgreSQL executor — runs [`QueryBuilder`] output against a live pool.
//!
//! Requires the `postgres` feature:
//!
//! ```toml
//! rok-orm = { version = "0.1", features = ["postgres"] }
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use rok_orm::{Model, executor};
//!
//! #[derive(Model, sqlx::FromRow)]
//! pub struct User { pub id: i64, pub name: String }
//!
//! let pool = sqlx::PgPool::connect(&database_url).await?;
//!
//! let users: Vec<User> = executor::fetch_all(&pool, User::query().where_eq("active", true)).await?;
//! let count: i64       = executor::count(&pool, &User::query()).await?;
//! executor::update(&pool, User::query().where_eq("id", 1i64), &[("name", "Bob".into())]).await?;
//! executor::delete(&pool, User::find(42i64)).await?;
//! ```

use rok_orm_core::{sqlx_pg, Model, QueryBuilder, SqlValue};
use sqlx::postgres::PgRow;
use sqlx::PgPool;
use std::time::Instant;

use crate::hooks::{
    dispatch_created, dispatch_creating, dispatch_deleted, dispatch_deleting, dispatch_saved,
    dispatch_saving, dispatch_updated, dispatch_updating,
};

// ── Retry Configuration ────────────────────────────────────────────────────────

/// Configuration for automatic retry of serialisation-failed transactions /
/// queries.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 50,
            max_delay_ms: 2000,
        }
    }
}

/// A trait for errors that can be checked for retry eligibility.
pub trait IsRetryable {
    fn is_retryable(&self) -> bool;
}

impl IsRetryable for sqlx::Error {
    fn is_retryable(&self) -> bool {
        match self {
            sqlx::Error::Database(db_err) => {
                let code = db_err.code().map(|c| c.to_string()).unwrap_or_default();
                // 40001 = serialization_failure, 40P01 = deadlock_detected
                code == "40001" || code == "40P01"
            }
            _ => false,
        }
    }
}

/// Execute a closure with automatic retry on serialisation / deadlock errors.
///
/// Uses exponential backoff with jitter-free delay capped at `config.max_delay_ms`.
///
/// ```rust,ignore
/// use rok_orm::executor::{RetryConfig, execute_with_retry};
///
/// let config = RetryConfig::default();
/// let user = execute_with_retry(&pool, &config, |pool| async {
///     let mut tx = pool.begin().await?;
///     // ... transactional work ...
///     tx.commit().await?;
///     Ok::<_, sqlx::Error>(user)
/// }).await?;
/// ```
pub async fn execute_with_retry<F, Fut, T, E>(
    pool: &PgPool,
    config: &RetryConfig,
    f: F,
) -> Result<T, E>
where
    F: Fn(&PgPool) -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: IsRetryable,
{
    let mut attempt = 0u32;
    loop {
        attempt += 1;
        match f(pool).await {
            Ok(val) => return Ok(val),
            Err(e) if e.is_retryable() && attempt < config.max_attempts => {
                let delay = config
                    .base_delay_ms
                    .saturating_mul(2u64.pow(attempt.saturating_sub(1)))
                    .min(config.max_delay_ms);
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            }
            Err(e) => return Err(e),
        }
    }
}

/// Fetch all rows matching the query.
pub async fn fetch_all<T>(pool: &PgPool, builder: QueryBuilder<T>) -> Result<Vec<T>, sqlx::Error>
where
    T: Model + for<'r> sqlx::FromRow<'r, PgRow> + Send + Unpin,
{
    let (sql, params) = builder.to_sql();
    let start = Instant::now();
    #[cfg(feature = "tracing")]
    let _span = tracing::info_span!(
        "db.query",
        "otel.kind" = "CLIENT",
        "db.system" = "postgresql",
        "db.operation" = "SELECT",
        "db.sql.table" = T::table_name(),
        "db.statement" = %sql,
    )
    .entered();
    let result = sqlx_pg::fetch_all_as::<T>(pool, &sql, params).await;
    let duration = start.elapsed().as_millis() as u64;
    let rows = result.as_ref().map(|v| v.len() as u64).unwrap_or(0);
    crate::query_log::log_query(&sql, duration, rows, T::table_name());
    result
}

/// Fetch at most one row matching the query.  Returns `None` if no rows match.
pub async fn fetch_optional<T>(
    pool: &PgPool,
    builder: QueryBuilder<T>,
) -> Result<Option<T>, sqlx::Error>
where
    T: Model + for<'r> sqlx::FromRow<'r, PgRow> + Send + Unpin,
{
    let (sql, params) = builder.to_sql();
    let start = Instant::now();
    #[cfg(feature = "tracing")]
    let _span = tracing::info_span!(
        "db.query",
        "otel.kind" = "CLIENT",
        "db.system" = "postgresql",
        "db.operation" = "SELECT",
        "db.sql.table" = T::table_name(),
        "db.statement" = %sql,
    )
    .entered();
    let result = sqlx_pg::fetch_optional_as::<T>(pool, &sql, params).await;
    let duration = start.elapsed().as_millis() as u64;
    let rows = result
        .as_ref()
        .map(|r| if r.is_some() { 1 } else { 0 })
        .unwrap_or(0);
    crate::query_log::log_query(&sql, duration, rows, T::table_name());
    result
}

/// Return the row count matching the query's WHERE clause.
pub async fn count<T: Model>(pool: &PgPool, builder: QueryBuilder<T>) -> Result<i64, sqlx::Error> {
    let (sql, params) = builder.to_count_sql();
    let start = Instant::now();
    #[cfg(feature = "tracing")]
    let _span = tracing::info_span!(
        "db.query",
        "otel.kind" = "CLIENT",
        "db.system" = "postgresql",
        "db.operation" = "SELECT COUNT",
        "db.sql.table" = T::table_name(),
        "db.statement" = %sql,
    )
    .entered();
    let row = sqlx_pg::build_query(&sql, params).fetch_one(pool).await;
    let duration = start.elapsed().as_millis() as u64;
    match row {
        Ok(row) => {
            use sqlx::Row;
            let val = row.try_get::<i64, _>(0)?;
            crate::query_log::log_query(&sql, duration, 1, T::table_name());
            Ok(val)
        }
        Err(e) => {
            crate::query_log::log_query(&sql, duration, 0, T::table_name());
            Err(e)
        }
    }
}

/// Execute a raw SQL string with positional parameters and return rows affected.
pub async fn execute_raw(
    pool: &PgPool,
    sql: &str,
    params: Vec<SqlValue>,
) -> Result<u64, sqlx::Error> {
    #[cfg(feature = "tracing")]
    let _span = tracing::info_span!(
        "db.execute",
        "otel.kind" = "CLIENT",
        "db.system" = "postgresql",
        "db.statement" = %sql,
    )
    .entered();
    sqlx_pg::execute(pool, sql, params).await
}

/// Insert a row using the column-value pairs and return rows affected.
pub async fn insert<T: 'static>(
    pool: &PgPool,
    table: &str,
    data: &[(&str, SqlValue)],
) -> Result<u64, sqlx::Error> {
    dispatch_saving::<T>(table, data).map_err(|e| sqlx::Error::Configuration(Box::new(e)))?;
    dispatch_creating::<T>(table, data).map_err(|e| sqlx::Error::Configuration(Box::new(e)))?;
    let (sql, params) = QueryBuilder::<T>::insert_sql(table, data);
    let start = Instant::now();
    let result = execute_raw(pool, &sql, params).await;
    let duration = start.elapsed().as_millis() as u64;
    let rows = result.as_ref().copied().unwrap_or(0);
    crate::query_log::log_query(&sql, duration, rows, table);
    dispatch_created::<T>(table, data);
    dispatch_saved::<T>(table, data);
    result
}

/// Update rows matching the builder's conditions and return rows affected.
pub async fn update<T: Model + 'static>(
    pool: &PgPool,
    builder: QueryBuilder<T>,
    data: &[(&str, SqlValue)],
) -> Result<u64, sqlx::Error> {
    let table = T::table_name();
    dispatch_saving::<T>(table, data).map_err(|e| sqlx::Error::Configuration(Box::new(e)))?;
    dispatch_updating::<T>(table, data).map_err(|e| sqlx::Error::Configuration(Box::new(e)))?;
    let (sql, params) = builder.to_update_sql(data);
    let start = Instant::now();
    let result = execute_raw(pool, &sql, params).await;
    let duration = start.elapsed().as_millis() as u64;
    let rows = result.as_ref().copied().unwrap_or(0);
    crate::query_log::log_query(&sql, duration, rows, table);
    dispatch_updated::<T>(table, data);
    dispatch_saved::<T>(table, data);
    result
}

/// Delete rows matching the builder's conditions and return rows affected.
pub async fn delete<T: Model + 'static>(
    pool: &PgPool,
    builder: QueryBuilder<T>,
) -> Result<u64, sqlx::Error> {
    let table = T::table_name();
    dispatch_deleting::<T>(table, &[]).map_err(|e| sqlx::Error::Configuration(Box::new(e)))?;
    let (sql, params) = builder.to_delete_sql();
    let start = Instant::now();
    let result = execute_raw(pool, &sql, params).await;
    let duration = start.elapsed().as_millis() as u64;
    let rows = result.as_ref().copied().unwrap_or(0);
    crate::query_log::log_query(&sql, duration, rows, table);
    dispatch_deleted::<T>(table, &[]);
    result
}

/// Insert multiple rows in a single `INSERT INTO … VALUES …, …` statement.
///
/// All rows must have the same columns in the same order as the first row.
/// Returns the total number of rows inserted.
///
/// ```rust,ignore
/// use rok_orm::executor;
///
/// executor::bulk_insert::<User>(
///     &pool,
///     "users",
///     &[
///         vec![("name", "Alice".into()), ("email", "a@a.com".into())],
///         vec![("name", "Bob".into()),   ("email", "b@b.com".into())],
///     ],
/// ).await?;
/// ```
pub async fn bulk_insert<T: 'static>(
    pool: &PgPool,
    table: &str,
    rows: &[Vec<(&str, SqlValue)>],
) -> Result<u64, sqlx::Error> {
    if rows.is_empty() {
        return Ok(0);
    }
    let first_row = &rows[0];
    dispatch_saving::<T>(table, first_row).map_err(|e| sqlx::Error::Configuration(Box::new(e)))?;
    dispatch_creating::<T>(table, first_row)
        .map_err(|e| sqlx::Error::Configuration(Box::new(e)))?;
    let (sql, params) = QueryBuilder::<T>::bulk_insert_sql(table, rows);
    let start = Instant::now();
    let result = execute_raw(pool, &sql, params).await;
    let duration = start.elapsed().as_millis() as u64;
    let rows_affected = result.as_ref().copied().unwrap_or(0);
    crate::query_log::log_query(&sql, duration, rows_affected, table);
    dispatch_created::<T>(table, first_row);
    dispatch_saved::<T>(table, first_row);
    result
}

/// Soft-delete rows matching the builder's conditions by setting `delete_col = NOW()`.
///
/// The SET clause uses the SQL `NOW()` function (not a bound parameter), so it
/// is always accurate to the current transaction time.
///
/// ```rust,ignore
/// // Soft-delete all sessions for a user.
/// executor::soft_delete(&pool, Session::query().where_eq("user_id", uid), "deleted_at").await?;
/// ```
pub async fn soft_delete<T: Model + 'static>(
    pool: &PgPool,
    builder: QueryBuilder<T>,
    delete_col: &str,
) -> Result<u64, sqlx::Error> {
    let table = T::table_name();
    dispatch_deleting::<T>(table, &[]).map_err(|e| sqlx::Error::Configuration(Box::new(e)))?;
    let (where_clause, params) = builder.to_where_clause();
    let sql = format!(
        "UPDATE {} SET {} = NOW(){}",
        table, delete_col, where_clause,
    );
    let start = Instant::now();
    let result = execute_raw(pool, &sql, params).await;
    let duration = start.elapsed().as_millis() as u64;
    let rows = result.as_ref().copied().unwrap_or(0);
    crate::query_log::log_query(&sql, duration, rows, table);
    dispatch_deleted::<T>(table, &[]);
    result
}

/// Restore soft-deleted rows matching the builder by setting `delete_col = NULL`.
pub async fn restore<T: Model>(
    pool: &PgPool,
    builder: QueryBuilder<T>,
    delete_col: &str,
) -> Result<u64, sqlx::Error> {
    let (where_clause, params) = builder.to_where_clause();
    let sql = format!(
        "UPDATE {} SET {} = NULL{}",
        T::table_name(),
        delete_col,
        where_clause,
    );
    let start = Instant::now();
    let result = execute_raw(pool, &sql, params).await;
    let duration = start.elapsed().as_millis() as u64;
    let rows = result.as_ref().copied().unwrap_or(0);
    crate::query_log::log_query(&sql, duration, rows, T::table_name());
    result
}

/// Update `updated_at = NOW()` for rows matching the builder (touch).
///
/// Does nothing (returns 0) if the model has no `timestamp_columns()`.
pub async fn touch<T: Model>(pool: &PgPool, builder: QueryBuilder<T>) -> Result<u64, sqlx::Error> {
    if let Some((_, updated_at)) = T::timestamp_columns() {
        let (where_clause, params) = builder.to_where_clause();
        let sql = format!(
            "UPDATE {} SET {updated_at} = NOW(){}",
            T::table_name(),
            where_clause,
        );
        let start = Instant::now();
        let result = execute_raw(pool, &sql, params).await;
        let duration = start.elapsed().as_millis() as u64;
        let rows = result.as_ref().copied().unwrap_or(0);
        crate::query_log::log_query(&sql, duration, rows, T::table_name());
        result
    } else {
        Ok(0)
    }
}

/// Run an aggregate expression and return the raw scalar value.
///
/// `agg_expr` is something like `"MAX(total)"`, `"SUM(price)"`, `"AVG(score)"`.
/// Returns `None` when no rows match (empty table / no matching rows).
pub async fn aggregate<T: Model>(
    pool: &PgPool,
    builder: QueryBuilder<T>,
    agg_expr: &str,
) -> Result<Option<f64>, sqlx::Error> {
    let (sql, params) = builder.to_aggregate_sql(agg_expr);
    let start = Instant::now();
    let result = sqlx_pg::build_query(&sql, params)
        .fetch_optional(pool)
        .await;
    let duration = start.elapsed().as_millis() as u64;
    match result {
        Ok(row) => {
            crate::query_log::log_query(&sql, duration, 1, T::table_name());
            use sqlx::Row;
            Ok(row.and_then(|r| r.try_get::<Option<f64>, _>(0).ok().flatten()))
        }
        Err(e) => {
            crate::query_log::log_query(&sql, duration, 0, T::table_name());
            Err(e)
        }
    }
}

/// Insert a row using `INSERT … ON CONFLICT … DO UPDATE SET …` (PostgreSQL upsert).
pub async fn upsert<T: Model>(
    pool: &PgPool,
    data: &[(&str, SqlValue)],
    conflict_cols: &[&str],
) -> Result<u64, sqlx::Error> {
    let (sql, params) = QueryBuilder::<T>::upsert_sql(T::table_name(), data, conflict_cols);
    let start = Instant::now();
    let result = execute_raw(pool, &sql, params).await;
    let duration = start.elapsed().as_millis() as u64;
    let rows = result.as_ref().copied().unwrap_or(0);
    crate::query_log::log_query(&sql, duration, rows, T::table_name());
    result
}

/// Atomically increment `col` by `amount` for rows matching the builder.
pub async fn increment<T: Model>(
    pool: &PgPool,
    builder: QueryBuilder<T>,
    col: &str,
    amount: i64,
) -> Result<u64, sqlx::Error> {
    let (where_clause, params) = builder.to_where_clause();
    let sql = format!(
        "UPDATE {} SET {col} = {col} + {amount}{}",
        T::table_name(),
        where_clause
    );
    let start = Instant::now();
    let result = execute_raw(pool, &sql, params).await;
    let duration = start.elapsed().as_millis() as u64;
    let rows = result.as_ref().copied().unwrap_or(0);
    crate::query_log::log_query(&sql, duration, rows, T::table_name());
    result
}

/// Atomically decrement `col` by `amount` for rows matching the builder.
pub async fn decrement<T: Model>(
    pool: &PgPool,
    builder: QueryBuilder<T>,
    col: &str,
    amount: i64,
) -> Result<u64, sqlx::Error> {
    increment(pool, builder, col, -amount).await
}

/// Insert a single row and return it using `RETURNING *`.
///
/// Useful when you need the generated primary key or server-side defaults.
///
/// ```rust,ignore
/// use rok_orm::executor;
///
/// let user: User = executor::insert_returning::<User>(
///     &pool,
///     "users",
///     &[("name", "Alice".into()), ("email", "a@a.com".into())],
/// ).await?;
/// println!("new id = {}", user.id);
/// ```
pub async fn insert_returning<T>(
    pool: &PgPool,
    table: &str,
    data: &[(&str, SqlValue)],
) -> Result<T, sqlx::Error>
where
    T: Model + for<'r> sqlx::FromRow<'r, PgRow> + Send + Unpin,
{
    let (base_sql, params) = QueryBuilder::<T>::insert_sql(table, data);
    let sql = format!("{base_sql} RETURNING *");
    let start = Instant::now();
    let result = sqlx_pg::fetch_all_as::<T>(pool, &sql, params).await;
    let duration = start.elapsed().as_millis() as u64;
    match result {
        Ok(rows) => {
            let count = rows.len() as u64;
            let row = rows.into_iter().next().ok_or(sqlx::Error::RowNotFound)?;
            crate::query_log::log_query(&sql, duration, count, table);
            Ok(row)
        }
        Err(e) => {
            crate::query_log::log_query(&sql, duration, 0, table);
            Err(e)
        }
    }
}

/// Insert multiple rows and return all inserted rows via `RETURNING *`.
///
/// All rows must have the same columns in the same order as the first row.
///
/// ```rust,ignore
/// let users: Vec<User> = executor::bulk_insert_returning::<User>(
///     &pool,
///     "users",
///     &[
///         vec![("name", "Alice".into()), ("email", "a@a.com".into())],
///         vec![("name", "Bob".into()),   ("email", "b@b.com".into())],
///     ],
/// ).await?;
/// ```
pub async fn bulk_insert_returning<T>(
    pool: &PgPool,
    table: &str,
    rows: &[Vec<(&str, SqlValue)>],
) -> Result<Vec<T>, sqlx::Error>
where
    T: Model + for<'r> sqlx::FromRow<'r, PgRow> + Send + Unpin,
{
    if rows.is_empty() {
        return Ok(vec![]);
    }
    let (base_sql, params) = QueryBuilder::<T>::bulk_insert_sql(table, rows);
    let sql = format!("{base_sql} RETURNING *");
    let start = Instant::now();
    let result = sqlx_pg::fetch_all_as::<T>(pool, &sql, params).await;
    let duration = start.elapsed().as_millis() as u64;
    let rows_affected = result.as_ref().map(|v| v.len() as u64).unwrap_or(0);
    crate::query_log::log_query(&sql, duration, rows_affected, table);
    result
}

/// Update rows matching the builder's conditions and return the affected rows
/// via `RETURNING *`.
///
/// ```rust,ignore
/// let users: Vec<User> = executor::update_returning(&pool, User::query().where_eq("active", true), &[("status", "archived".into())]).await?;
/// ```
pub async fn update_returning<T>(
    pool: &PgPool,
    builder: QueryBuilder<T>,
    data: &[(&str, SqlValue)],
) -> Result<Vec<T>, sqlx::Error>
where
    T: Model + for<'r> sqlx::FromRow<'r, PgRow> + Send + Unpin + 'static,
{
    let table = T::table_name();
    dispatch_saving::<T>(table, data).map_err(|e| sqlx::Error::Configuration(Box::new(e)))?;
    dispatch_updating::<T>(table, data).map_err(|e| sqlx::Error::Configuration(Box::new(e)))?;
    let (base_sql, base_params) = builder.to_update_sql(data);
    let sql = format!("{base_sql} RETURNING *");
    let start = Instant::now();
    let result = sqlx_pg::fetch_all_as::<T>(pool, &sql, base_params).await;
    let duration = start.elapsed().as_millis() as u64;
    let rows_affected = result.as_ref().map(|v| v.len() as u64).unwrap_or(0);
    crate::query_log::log_query(&sql, duration, rows_affected, table);
    dispatch_updated::<T>(table, data);
    dispatch_saved::<T>(table, data);
    result
}

/// Delete rows matching the builder's conditions and return the deleted rows
/// via `RETURNING *`.
///
/// ```rust,ignore
/// let deleted: Vec<User> = executor::delete_returning(&pool, User::query().where_eq("status", "inactive")).await?;
/// ```
pub async fn delete_returning<T>(
    pool: &PgPool,
    builder: QueryBuilder<T>,
) -> Result<Vec<T>, sqlx::Error>
where
    T: Model + for<'r> sqlx::FromRow<'r, PgRow> + Send + Unpin + 'static,
{
    let table = T::table_name();
    dispatch_deleting::<T>(table, &[]).map_err(|e| sqlx::Error::Configuration(Box::new(e)))?;
    let (base_sql, base_params) = builder.to_delete_sql();
    let sql = format!("{base_sql} RETURNING *");
    let start = Instant::now();
    let result = sqlx_pg::fetch_all_as::<T>(pool, &sql, base_params).await;
    let duration = start.elapsed().as_millis() as u64;
    let rows_affected = result.as_ref().map(|v| v.len() as u64).unwrap_or(0);
    crate::query_log::log_query(&sql, duration, rows_affected, table);
    dispatch_deleted::<T>(table, &[]);
    result
}

/// Insert multiple rows with `ON CONFLICT … DO UPDATE` in a single statement.
///
/// All rows must have the same column keys in the same order.
///
/// ```rust,ignore
/// User::bulk_upsert(
///     &pool,
///     &[
///         vec![("email", "a@a.com".into()), ("name", "Alice".into())],
///         vec![("email", "b@b.com".into()), ("name", "Bob".into())],
///     ],
///     &["email"],
/// ).await?;
/// ```
pub async fn bulk_upsert<T: Model + 'static>(
    pool: &PgPool,
    rows: &[Vec<(&str, SqlValue)>],
    conflict_cols: &[&str],
) -> Result<u64, sqlx::Error> {
    if rows.is_empty() {
        return Ok(0);
    }
    let table = T::table_name();
    let first_row = &rows[0];
    let cols: Vec<&str> = first_row.iter().map(|(c, _)| *c).collect();
    let conflict_target = conflict_cols.join(", ");
    let update_set: Vec<String> = cols
        .iter()
        .filter(|c| !conflict_cols.contains(c))
        .map(|c| format!("{c} = EXCLUDED.{c}"))
        .collect();

    let cols_count = cols.len();
    let mut row_placeholders: Vec<String> = Vec::with_capacity(rows.len());
    let mut params: Vec<SqlValue> = Vec::with_capacity(rows.len() * cols_count);

    for (row_idx, row) in rows.iter().enumerate() {
        let phs: Vec<String> = (0..cols_count)
            .map(|col_idx| format!("${}", row_idx * cols_count + col_idx + 1))
            .collect();
        row_placeholders.push(format!("({})", phs.join(", ")));
        for (_, val) in row.iter() {
            params.push(val.clone());
        }
    }

    let conflict_clause = if update_set.is_empty() {
        format!("ON CONFLICT ({conflict_target}) DO NOTHING")
    } else {
        format!(
            "ON CONFLICT ({conflict_target}) DO UPDATE SET {}",
            update_set.join(", ")
        )
    };

    let sql = format!(
        "INSERT INTO {} ({}) VALUES {} {}",
        table,
        cols.join(", "),
        row_placeholders.join(", "),
        conflict_clause,
    );

    let start = Instant::now();
    let result = execute_raw(pool, &sql, params).await;
    let duration = start.elapsed().as_millis() as u64;
    let rows_affected = result.as_ref().copied().unwrap_or(0);
    crate::query_log::log_query(&sql, duration, rows_affected, table);
    result
}

/// Upsert a row (`INSERT … ON CONFLICT … DO UPDATE SET …`) and return the
/// affected row via `RETURNING *`.
///
/// ```rust,ignore
/// let user: User = executor::upsert_returning::<User>(
///     &pool,
///     &[("email", "a@a.com".into()), ("name", "Alice".into())],
///     &["email"],
/// ).await?;
/// ```
pub async fn upsert_returning<T>(
    pool: &PgPool,
    data: &[(&str, SqlValue)],
    conflict_cols: &[&str],
) -> Result<T, sqlx::Error>
where
    T: Model + for<'r> sqlx::FromRow<'r, PgRow> + Send + Unpin,
{
    let (base_sql, params) = QueryBuilder::<T>::upsert_sql(T::table_name(), data, conflict_cols);
    let sql = format!("{base_sql} RETURNING *");
    let start = Instant::now();
    let result = sqlx_pg::fetch_all_as::<T>(pool, &sql, params).await;
    let duration = start.elapsed().as_millis() as u64;
    match result {
        Ok(rows) => {
            let count = rows.len() as u64;
            let row = rows.into_iter().next().ok_or(sqlx::Error::RowNotFound)?;
            crate::query_log::log_query(&sql, duration, count, T::table_name());
            Ok(row)
        }
        Err(e) => {
            crate::query_log::log_query(&sql, duration, 0, T::table_name());
            Err(e)
        }
    }
}
