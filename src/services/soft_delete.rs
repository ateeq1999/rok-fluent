//! [`SoftDeleteService<M>`] — scoped queries and operations for soft-deletable models.

use crate::core::condition::SqlValue;
use crate::orm::postgres::model::PgModel;

/// Scoped soft-delete operations for any model with a `deleted_at` column.
///
/// Models opt in by adding `#[model(soft_delete)]` to their struct. Rows with a
/// non-NULL `deleted_at` are considered soft-deleted; rows with `NULL` are active.
///
/// # Example
///
/// ```rust,no_run
/// # use rok_fluent::services::SoftDeleteService;
/// # async fn run() -> Result<(), sqlx::Error> {
/// # let pool: sqlx::PgPool = todo!();
/// // let active  = SoftDeleteService::<Post>::all_active(&pool).await?;
/// // let deleted = SoftDeleteService::<Post>::all_deleted(&pool).await?;
/// // SoftDeleteService::<Post>::purge_deleted(&pool).await?;
/// # Ok(())
/// # }
/// ```
pub struct SoftDeleteService<M>(std::marker::PhantomData<M>);

impl<M: PgModel> SoftDeleteService<M> {
    /// Return all rows where `deleted_at IS NULL`.
    pub async fn all_active(pool: &sqlx::PgPool) -> Result<Vec<M>, sqlx::Error> {
        let builder = match M::soft_delete_column() {
            Some(col) => M::query().where_null(col),
            None => M::query(),
        };
        M::find_where(pool, builder).await
    }

    /// Return all rows where `deleted_at IS NOT NULL`.
    ///
    /// Returns an empty `Vec` when the model has no soft-delete column.
    pub async fn all_deleted(pool: &sqlx::PgPool) -> Result<Vec<M>, sqlx::Error> {
        match M::soft_delete_column() {
            Some(col) => M::find_where(pool, M::query().where_not_null(col)).await,
            None => Ok(Vec::new()),
        }
    }

    /// Return every row regardless of `deleted_at` status.
    pub async fn with_trashed(pool: &sqlx::PgPool) -> Result<Vec<M>, sqlx::Error> {
        M::all(pool).await
    }

    /// Soft-delete the row with the given primary key — sets `deleted_at = NOW()`.
    pub async fn soft_delete(
        id: impl Into<SqlValue> + Send,
        pool: &sqlx::PgPool,
    ) -> Result<u64, sqlx::Error> {
        M::soft_delete_by_pk(pool, id.into()).await
    }

    /// Restore a soft-deleted row — clears `deleted_at`.
    pub async fn restore(
        id: impl Into<SqlValue> + Send,
        pool: &sqlx::PgPool,
    ) -> Result<u64, sqlx::Error> {
        M::restore_by_pk(pool, id.into()).await
    }

    /// Hard-delete a row by primary key, bypassing the soft-delete mechanism.
    pub async fn force_delete(
        id: impl Into<SqlValue> + Send,
        pool: &sqlx::PgPool,
    ) -> Result<u64, sqlx::Error> {
        M::delete_by_pk(pool, id.into()).await
    }

    /// Hard-delete every row that has a non-NULL `deleted_at`.
    ///
    /// Returns the number of rows purged. No-op when the model has no soft-delete column.
    pub async fn purge_deleted(pool: &sqlx::PgPool) -> Result<u64, sqlx::Error> {
        match M::soft_delete_column() {
            Some(col) => M::delete_where(pool, M::query().where_not_null(col)).await,
            None => Ok(0),
        }
    }
}
