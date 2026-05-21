//! [`TransactionService`] — composable transactions with savepoints and CRUD.

use crate::core::condition::SqlValue;
use crate::core::query::QueryBuilder;
use crate::core::sqlx::pg as sqlx_pg;
use crate::orm::postgres::model::PgModel;

/// A running database transaction with savepoint and CRUD support.
///
/// Obtained from [`TransactionService::begin`]. Dropping without committing
/// will automatically roll back the transaction (and any active savepoints).
pub struct TxCtx<'c> {
    inner: sqlx::Transaction<'c, sqlx::Postgres>,
}

type ModelQuery<M> = crate::orm::model_query::ModelQuery<M>;

impl<'c> TxCtx<'c> {
    fn new(inner: sqlx::Transaction<'c, sqlx::Postgres>) -> Self {
        Self { inner }
    }

    /// Commit the transaction.
    pub async fn commit(self) -> Result<(), sqlx::Error> {
        self.inner.commit().await
    }

    /// Roll back the transaction (also happens automatically on drop).
    pub async fn rollback(self) -> Result<(), sqlx::Error> {
        self.inner.rollback().await
    }

    // ── Savepoints ─────────────────────────────────────────────────────────

    /// Create a named savepoint within the current transaction.
    ///
    /// Use [`rollback_to`](TxCtx::rollback_to) to revert to this point without
    /// aborting the entire transaction, or [`release`](TxCtx::release) to
    /// discard the savepoint.
    pub async fn savepoint(&mut self, name: &str) -> Result<(), sqlx::Error> {
        let sql = format!("SAVEPOINT \"{name}\"");
        sqlx::query(&sql).execute(&mut *self.inner).await?;
        Ok(())
    }

    /// Roll back to a named savepoint, discarding all changes since it was set.
    pub async fn rollback_to(&mut self, name: &str) -> Result<(), sqlx::Error> {
        let sql = format!("ROLLBACK TO SAVEPOINT \"{name}\"");
        sqlx::query(&sql).execute(&mut *self.inner).await?;
        Ok(())
    }

    /// Release (forget) a named savepoint.
    pub async fn release(&mut self, name: &str) -> Result<(), sqlx::Error> {
        let sql = format!("RELEASE SAVEPOINT \"{name}\"");
        sqlx::query(&sql).execute(&mut *self.inner).await?;
        Ok(())
    }

    // ── Read ───────────────────────────────────────────────────────────────

    /// Fetch rows via a model query, within this transaction.
    pub async fn fetch_all<T>(&mut self, query: ModelQuery<T>) -> Result<Vec<T>, sqlx::Error>
    where
        T: PgModel + Sync,
    {
        let builder = query.into_final_builder();
        let (sql, params) = builder.to_sql();
        sqlx_pg::build_query_as::<T>(&sql, params)
            .fetch_all(&mut *self.inner)
            .await
    }

    /// Fetch at most one row via a model query, within this transaction.
    pub async fn fetch_optional<T>(
        &mut self,
        query: ModelQuery<T>,
    ) -> Result<Option<T>, sqlx::Error>
    where
        T: PgModel + Sync,
    {
        let builder = query.into_final_builder();
        let (sql, params) = builder.to_sql();
        sqlx_pg::build_query_as::<T>(&sql, params)
            .fetch_optional(&mut *self.inner)
            .await
    }

    // ── Write ──────────────────────────────────────────────────────────────

    /// Insert a row and return the number of rows affected.
    pub async fn create<T>(&mut self, data: &[(&str, SqlValue)]) -> Result<u64, sqlx::Error>
    where
        T: PgModel + Sync,
    {
        let (sql, params) = QueryBuilder::<T>::insert_sql(T::table_name(), data);
        let result = sqlx_pg::build_query(&sql, params)
            .execute(&mut *self.inner)
            .await?;
        Ok(result.rows_affected())
    }

    /// Insert a row and return the full inserted row via `RETURNING *`.
    pub async fn create_returning<T>(&mut self, data: &[(&str, SqlValue)]) -> Result<T, sqlx::Error>
    where
        T: PgModel + Sync,
    {
        let (base_sql, params) = QueryBuilder::<T>::insert_sql(T::table_name(), data);
        let sql = format!("{base_sql} RETURNING *");
        sqlx_pg::build_query_as::<T>(&sql, params)
            .fetch_optional(&mut *self.inner)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    /// Update rows matching a model query, within this transaction.
    pub async fn update<T>(
        &mut self,
        query: ModelQuery<T>,
        data: &[(&str, SqlValue)],
    ) -> Result<u64, sqlx::Error>
    where
        T: PgModel + Sync,
    {
        let builder = query.into_final_builder();
        let (sql, params) = builder.to_update_sql(data);
        let result = sqlx_pg::build_query(&sql, params)
            .execute(&mut *self.inner)
            .await?;
        Ok(result.rows_affected())
    }

    /// Update by primary key.
    pub async fn update_by_pk<T>(
        &mut self,
        id: impl Into<SqlValue> + Send,
        data: &[(&str, SqlValue)],
    ) -> Result<u64, sqlx::Error>
    where
        T: PgModel + Sync,
    {
        let query = T::find_query(id.into());
        self.update::<T>(query, data).await
    }

    /// Delete rows matching a model query, within this transaction.
    pub async fn delete<T>(&mut self, query: ModelQuery<T>) -> Result<u64, sqlx::Error>
    where
        T: PgModel + Sync,
    {
        let builder = query.into_final_builder();
        let (sql, params) = builder.to_delete_sql();
        let result = sqlx_pg::build_query(&sql, params)
            .execute(&mut *self.inner)
            .await?;
        Ok(result.rows_affected())
    }

    /// Delete by primary key.
    pub async fn delete_by_pk<T>(
        &mut self,
        id: impl Into<SqlValue> + Send,
    ) -> Result<u64, sqlx::Error>
    where
        T: PgModel + Sync,
    {
        let query = T::find_query(id.into());
        self.delete::<T>(query).await
    }
}

/// Composable transaction service with savepoint support.
///
/// # Example
///
/// ```rust,no_run
/// # use rok_fluent::services::TransactionService;
/// # async fn example(pool: sqlx::PgPool) -> Result<(), sqlx::Error> {
/// let mut tx = TransactionService::begin(&pool).await?;
/// tx.savepoint("after_setup").await?;
/// // … operations …
/// tx.rollback_to("after_setup").await?;
/// tx.release("after_setup").await?;
/// tx.commit().await?;
/// # Ok(())
/// # }
/// ```
pub struct TransactionService;

impl TransactionService {
    /// Begin a new database transaction, returning a [`TxCtx`] for CRUD and
    /// savepoint operations.
    ///
    /// Call [`TxCtx::commit`] to persist or let it drop to roll back.
    pub async fn begin(pool: &sqlx::PgPool) -> Result<TxCtx<'_>, sqlx::Error> {
        let inner = pool.begin().await?;
        Ok(TxCtx::new(inner))
    }
}
