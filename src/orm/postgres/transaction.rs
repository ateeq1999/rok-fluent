//! PostgreSQL transaction wrapper.
//!
//! [`Tx`] wraps a [`sqlx::Transaction`] and exposes the same ORM operations as
//! [`executor`], but all run inside a single database transaction.
//!
//! ```rust,no_run
//! # use rok_fluent::orm::postgres::transaction::Tx;
//! # async fn example(pool: sqlx::PgPool) -> Result<(), sqlx::Error> {
//! let mut tx = Tx::begin(&pool).await?;
//! # Ok(())
//! # }
//! ```

use std::time::Duration;

use sqlx::{postgres::PgRow, PgPool};

use super::executor::{IsRetryable, RetryConfig};
use crate::core::condition::SqlValue;
use crate::core::model::Model;
use crate::core::query::QueryBuilder;
use crate::core::sqlx::pg as sqlx_pg;

/// A running PostgreSQL transaction.
///
/// All ORM operations performed on `Tx` participate in the same database
/// transaction.  Call [`commit`](Tx::commit) to persist changes or
/// [`rollback`](Tx::rollback) to discard them.  If `Tx` is dropped without
/// either being called, the transaction is rolled back automatically by sqlx.
pub struct Tx<'c> {
    inner: sqlx::Transaction<'c, sqlx::Postgres>,
}

impl<'c> Tx<'c> {
    /// Begin a new transaction on the given pool.
    pub async fn begin(pool: &'c PgPool) -> Result<Self, sqlx::Error> {
        Ok(Self {
            inner: pool.begin().await?,
        })
    }

    /// Commit the transaction.
    pub async fn commit(self) -> Result<(), sqlx::Error> {
        self.inner.commit().await
    }

    /// Commit the transaction.
    ///
    /// For retry-on-serialisation-failure behaviour use
    /// [`Tx::run_with_retry`] instead — the sqlx `commit()` consumes the
    /// transaction handle, so retry must be at the transaction lifecycle level.
    pub async fn commit_with_retry(self, _config: &RetryConfig) -> Result<(), sqlx::Error> {
        self.inner.commit().await
    }

    /// Run a closure inside a transaction with automatic retry on serialisation /
    /// deadlock errors.
    ///
    /// If the operation (including the inner commit) fails with a retryable error
    /// (PG code 40001 or 40P01), the transaction is rolled back, a new one is
    /// started, and `f` is called again — up to `config.max_attempts` times.
    ///
    /// ```rust,no_run
    /// # use rok_fluent::orm::postgres::{executor::RetryConfig, transaction::Tx};
    /// # async fn example(pool: sqlx::PgPool) -> Result<(), sqlx::Error> {
    /// let config = RetryConfig::default();
    /// Tx::run_with_retry(&pool, &config, |_tx| async { Ok::<(), sqlx::Error>(()) }).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn run_with_retry<F, Fut, T>(
        pool: &'c PgPool,
        config: &RetryConfig,
        mut f: F,
    ) -> Result<T, sqlx::Error>
    where
        F: FnMut(&mut Tx<'c>) -> Fut,
        Fut: std::future::Future<Output = Result<T, sqlx::Error>>,
    {
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            let mut tx = Tx::begin(pool).await?;
            let result = f(&mut tx).await;
            match result {
                Ok(val) => return tx.commit().await.map(|_| val),
                Err(e) if e.is_retryable() && attempt < config.max_attempts => {
                    let delay = config
                        .base_delay_ms
                        .saturating_mul(2u64.pow(attempt.saturating_sub(1)))
                        .min(config.max_delay_ms);
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Roll back the transaction explicitly (also happens automatically on drop).
    pub async fn rollback(self) -> Result<(), sqlx::Error> {
        self.inner.rollback().await
    }

    // ── read ──────────────────────────────────────────────────────────────

    /// Fetch all rows matching the builder, within this transaction.
    pub async fn fetch_all<T>(&mut self, builder: QueryBuilder<T>) -> Result<Vec<T>, sqlx::Error>
    where
        T: Model + for<'r> sqlx::FromRow<'r, PgRow> + Send + Unpin,
    {
        let (sql, params) = builder.to_sql();
        sqlx_pg::build_query_as::<T>(&sql, params)
            .fetch_all(&mut *self.inner)
            .await
    }

    /// Fetch at most one row matching the builder, within this transaction.
    pub async fn fetch_optional<T>(
        &mut self,
        builder: QueryBuilder<T>,
    ) -> Result<Option<T>, sqlx::Error>
    where
        T: Model + for<'r> sqlx::FromRow<'r, PgRow> + Send + Unpin,
    {
        let (sql, params) = builder.to_sql();
        sqlx_pg::build_query_as::<T>(&sql, params)
            .fetch_optional(&mut *self.inner)
            .await
    }

    /// Return the row count matching the builder's WHERE clause, within this transaction.
    pub async fn count<T>(&mut self, builder: QueryBuilder<T>) -> Result<i64, sqlx::Error> {
        let (sql, params) = builder.to_count_sql();
        let row = sqlx_pg::build_query(&sql, params)
            .fetch_one(&mut *self.inner)
            .await?;
        use sqlx::Row;
        row.try_get::<i64, _>(0)
    }

    // ── write ─────────────────────────────────────────────────────────────

    /// Execute a raw SQL string and return rows affected, within this transaction.
    pub async fn execute_raw(
        &mut self,
        sql: &str,
        params: Vec<SqlValue>,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx_pg::build_query(sql, params)
            .execute(&mut *self.inner)
            .await?;
        Ok(result.rows_affected())
    }

    /// Insert a row and return rows affected, within this transaction.
    pub async fn insert<T>(
        &mut self,
        table: &str,
        data: &[(&str, SqlValue)],
    ) -> Result<u64, sqlx::Error> {
        let (sql, params) = QueryBuilder::<T>::insert_sql(table, data);
        self.execute_raw(&sql, params).await
    }

    /// Insert a row and return the full inserted row via `RETURNING *`.
    pub async fn insert_returning<T>(
        &mut self,
        table: &str,
        data: &[(&str, SqlValue)],
    ) -> Result<T, sqlx::Error>
    where
        T: Model + for<'r> sqlx::FromRow<'r, PgRow> + Send + Unpin,
    {
        let (base_sql, params) = QueryBuilder::<T>::insert_sql(table, data);
        let sql = format!("{base_sql} RETURNING *");
        sqlx_pg::build_query_as::<T>(&sql, params)
            .fetch_optional(&mut *self.inner)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    /// Insert multiple rows in a single statement, within this transaction.
    pub async fn bulk_insert<T>(
        &mut self,
        table: &str,
        rows: &[Vec<(&str, SqlValue)>],
    ) -> Result<u64, sqlx::Error> {
        if rows.is_empty() {
            return Ok(0);
        }
        let (sql, params) = QueryBuilder::<T>::bulk_insert_sql(table, rows);
        self.execute_raw(&sql, params).await
    }

    /// Update rows matching the builder's conditions, within this transaction.
    pub async fn update<T>(
        &mut self,
        builder: QueryBuilder<T>,
        data: &[(&str, SqlValue)],
    ) -> Result<u64, sqlx::Error> {
        let (sql, params) = builder.to_update_sql(data);
        self.execute_raw(&sql, params).await
    }

    /// Delete rows matching the builder's conditions, within this transaction.
    pub async fn delete<T>(&mut self, builder: QueryBuilder<T>) -> Result<u64, sqlx::Error> {
        let (sql, params) = builder.to_delete_sql();
        self.execute_raw(&sql, params).await
    }
}
