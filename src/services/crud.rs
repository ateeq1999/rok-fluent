//! [`CrudService<M>`] — zero-boilerplate CRUD wrapper for Active Record models.

use crate::core::condition::SqlValue;
use crate::orm::eager::EagerLoadable;
use crate::orm::model_query::ModelQuery;
use crate::orm::pagination::{CursorPage, Page, SimplePage};
use crate::orm::postgres::model::PgModel;

/// A generic CRUD service for any PostgreSQL-backed Active Record model.
///
/// Wraps the most common database operations — find, list, create, update,
/// delete, paginate, search — as methods on a pool-owning struct.
///
/// # Example
///
/// ```rust,no_run
/// # use rok_fluent::services::CrudService;
/// # async fn run() -> Result<(), sqlx::Error> {
/// # let pool: sqlx::PgPool = todo!();
/// // let svc = CrudService::<User>::new(pool.clone());
/// // let all  = svc.all().await?;
/// // let page = svc.paginate(1, 25).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct CrudService<M> {
    pool: sqlx::PgPool,
    _marker: std::marker::PhantomData<M>,
}

impl<M> CrudService<M>
where
    M: PgModel + Sync + Clone + serde::Serialize,
{
    /// Create a new service wrapping the given pool.
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self {
            pool,
            _marker: std::marker::PhantomData,
        }
    }

    // ── Read ──────────────────────────────────────────────────────────────────

    /// Return all rows, unordered.
    pub async fn all(&self) -> Result<Vec<M>, sqlx::Error> {
        M::all(&self.pool).await
    }

    /// Return all rows with eagerly-loaded relations.
    pub async fn all_with(&self, relations: &[&str]) -> Result<Vec<M>, sqlx::Error>
    where
        M: EagerLoadable,
    {
        let mut results = M::all(&self.pool).await?;
        for rel in relations {
            results = M::load_eager(rel, results).await?;
        }
        Ok(results)
    }

    /// Find a row by primary key; returns `None` if not found.
    pub async fn find(&self, id: impl Into<SqlValue> + Send) -> Result<Option<M>, sqlx::Error> {
        M::find_by_pk(&self.pool, id.into()).await
    }

    /// Find a row by primary key; returns `Err(RowNotFound)` if missing.
    pub async fn find_or_fail(&self, id: impl Into<SqlValue> + Send) -> Result<M, sqlx::Error> {
        M::find_or_fail(&self.pool, id.into()).await
    }

    /// Return the total row count.
    pub async fn count(&self) -> Result<i64, sqlx::Error> {
        M::count(&self.pool).await
    }

    /// Return `true` if any row matches the given primary key.
    pub async fn exists(&self, id: impl Into<SqlValue> + Send) -> Result<bool, sqlx::Error> {
        let pool = self.pool.clone();
        crate::orm::postgres::pool::with_pool(pool, M::find_query(id.into()).exists()).await
    }

    /// Start a scoped read chain — returns a [`ModelQuery<M>`] for further filtering.
    pub fn query(&self) -> ModelQuery<M> {
        M::all_query()
    }

    /// Filter by a single column equality — convenience over `.query()`.
    pub fn filter(&self, col: &str, val: impl Into<SqlValue>) -> ModelQuery<M> {
        M::filter(col, val)
    }

    // ── Pagination ────────────────────────────────────────────────────────────

    /// Offset pagination with a `COUNT(*)` query for full metadata.
    pub async fn paginate(&self, page: u32, per_page: u32) -> Result<Page<M>, sqlx::Error> {
        let pool = self.pool.clone();
        crate::orm::postgres::pool::with_pool(pool, M::all_query().paginate(per_page, page)).await
    }

    /// Offset pagination with eagerly-loaded relations.
    pub async fn paginate_with(
        &self,
        page: u32,
        per_page: u32,
        relations: &[&str],
    ) -> Result<Page<M>, sqlx::Error>
    where
        M: EagerLoadable + Clone,
    {
        let mut page = self.paginate(page, per_page).await?;
        for rel in relations {
            page.data = M::load_eager(rel, page.data).await?;
        }
        Ok(page)
    }

    /// Simple pagination — no `COUNT(*)`; detects next page by over-fetching.
    pub async fn simple_paginate(
        &self,
        page: u32,
        per_page: u32,
    ) -> Result<SimplePage<M>, sqlx::Error> {
        let pool = self.pool.clone();
        crate::orm::postgres::pool::with_pool(pool, M::all_query().simple_paginate(per_page, page))
            .await
    }

    /// Cursor-based pagination — stable for infinite scroll.
    pub async fn cursor_paginate(
        &self,
        cursor_col: &str,
        cursor: Option<&str>,
        per_page: u32,
    ) -> Result<CursorPage<M>, sqlx::Error> {
        let pool = self.pool.clone();
        let col = cursor_col.to_owned();
        let cur = cursor.map(str::to_owned);
        crate::orm::postgres::pool::with_pool(
            pool,
            M::all_query().cursor_paginate(per_page, &col, cur.as_deref()),
        )
        .await
    }

    // ── Write ─────────────────────────────────────────────────────────────────

    /// Insert a new row and return the full inserted record.
    pub async fn create(&self, data: &[(&str, SqlValue)]) -> Result<M, sqlx::Error> {
        M::create_returning(&self.pool, data).await
    }

    /// Update columns by primary key; returns the number of rows affected.
    pub async fn update(
        &self,
        id: impl Into<SqlValue> + Send,
        data: &[(&str, SqlValue)],
    ) -> Result<u64, sqlx::Error> {
        M::update_by_pk(&self.pool, id.into(), data).await
    }

    /// Hard-delete a row by primary key; returns the number of rows affected.
    pub async fn delete(&self, id: impl Into<SqlValue> + Send) -> Result<u64, sqlx::Error> {
        M::delete_by_pk(&self.pool, id.into()).await
    }

    /// Soft-delete a row — sets `deleted_at = NOW()`.
    pub async fn soft_delete(&self, id: impl Into<SqlValue> + Send) -> Result<u64, sqlx::Error> {
        M::soft_delete_by_pk(&self.pool, id.into()).await
    }

    /// Restore a soft-deleted row — clears `deleted_at`.
    pub async fn restore(&self, id: impl Into<SqlValue> + Send) -> Result<u64, sqlx::Error> {
        M::restore_by_pk(&self.pool, id.into()).await
    }

    // ── Bulk ──────────────────────────────────────────────────────────────────

    /// Insert multiple rows at once; returns the row count.
    pub async fn bulk_create(&self, rows: &[Vec<(&str, SqlValue)>]) -> Result<u64, sqlx::Error> {
        M::bulk_create(&self.pool, rows).await
    }

    /// Delete all rows matching `col = val` pairs; returns rows affected.
    pub async fn delete_where(&self, conditions: &[(&str, SqlValue)]) -> Result<u64, sqlx::Error> {
        let mut builder = M::query();
        for (col, val) in conditions {
            builder = builder.where_eq(col, val.clone());
        }
        M::delete_where(&self.pool, builder).await
    }

    /// Upsert by a unique key column; returns the inserted or updated record.
    pub async fn upsert_by(
        &self,
        unique_col: &str,
        data: &[(&str, SqlValue)],
    ) -> Result<M, sqlx::Error> {
        M::upsert_returning(&self.pool, data, &[unique_col]).await
    }

    // ── Search ────────────────────────────────────────────────────────────────

    /// Return all rows where any of `cols` matches `term` via `ILIKE '%term%'`.
    pub async fn search(&self, term: &str, cols: &[&str]) -> Result<Vec<M>, sqlx::Error> {
        super::search::SearchService::<M>::search(term, cols, &self.pool).await
    }

    /// Offset-paginated search with total-count query.
    pub async fn search_paginated(
        &self,
        term: &str,
        cols: &[&str],
        page: u32,
        per_page: u32,
    ) -> Result<Page<M>, sqlx::Error> {
        super::search::SearchService::<M>::search_paginated(term, cols, page, per_page, &self.pool)
            .await
    }
}
