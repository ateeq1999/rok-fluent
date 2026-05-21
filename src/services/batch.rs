//! [`BatchService<M>`] — efficient multi-row operations for Active Record models.

use crate::core::condition::SqlValue;
use crate::orm::postgres::model::PgModel;

/// Efficient bulk database operations for any PostgreSQL-backed model.
///
/// ```rust,no_run
/// # use rok_fluent::services::BatchService;
/// # async fn run() -> Result<(), sqlx::Error> {
/// # let pool: sqlx::PgPool = todo!();
/// // let rows = vec![
/// //     vec![("name", SqlValue::from("Alice")), ("email", SqlValue::from("a@b.com"))],
/// //     vec![("name", SqlValue::from("Bob")),   ("email", SqlValue::from("b@b.com"))],
/// // ];
/// // BatchService::<User>::bulk_insert(&rows, &pool).await?;
/// # Ok(())
/// # }
/// ```
pub struct BatchService<M>(std::marker::PhantomData<M>);

impl<M: PgModel> BatchService<M> {
    /// Insert multiple rows at once.
    pub async fn bulk_insert(
        rows: &[Vec<(&str, SqlValue)>],
        pool: &sqlx::PgPool,
    ) -> Result<u64, sqlx::Error> {
        M::bulk_create(pool, rows).await
    }

    /// Insert multiple rows in chunks of `chunk_size` to avoid parameter limits.
    pub async fn bulk_insert_chunked(
        rows: &[Vec<(&str, SqlValue)>],
        chunk_size: usize,
        pool: &sqlx::PgPool,
    ) -> Result<u64, sqlx::Error> {
        M::bulk_insert_chunked(pool, rows, chunk_size).await
    }

    /// Upsert multiple rows by the given unique key column.
    pub async fn bulk_upsert_by(
        unique_col: &str,
        rows: &[Vec<(&str, SqlValue)>],
        pool: &sqlx::PgPool,
    ) -> Result<u64, sqlx::Error> {
        M::bulk_upsert(pool, rows, &[unique_col]).await
    }

    /// Delete all rows matching the given column = value pairs.
    pub async fn delete_where(
        conditions: &[(&str, SqlValue)],
        pool: &sqlx::PgPool,
    ) -> Result<u64, sqlx::Error> {
        let mut builder = M::query();
        for (col, val) in conditions {
            builder = builder.where_eq(col, val.clone());
        }
        M::delete_where(pool, builder).await
    }
}
