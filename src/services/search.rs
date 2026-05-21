//! [`SearchService<M>`] — full-text and ILIKE search across declared columns.

use crate::core::model::Model;
use crate::core::query::QueryBuilder;
use crate::orm::pagination::{Page, SimplePage};
use crate::orm::postgres::model::PgModel;

/// Column-scoped text search for any PostgreSQL-backed model.
///
/// Pass the list of columns to search explicitly. When `#[table(searchable)]` is
/// available (Phase 34), the list will be derivable from the struct definition.
///
/// **Standard path** uses `ILIKE '%term%'` with columns OR-ed together.
/// **Full-text path** (`fts`) uses PostgreSQL `to_tsvector` / `plainto_tsquery`.
///
/// # Example
///
/// ```rust,no_run
/// # use rok_fluent::services::SearchService;
/// # async fn run() -> Result<(), sqlx::Error> {
/// # let pool: sqlx::PgPool = todo!();
/// // let users = SearchService::<User>::search("alice", &["name", "email"], &pool).await?;
/// // let page  = SearchService::<User>::search_paginated("alice", &["name", "email"], 1, 25, &pool).await?;
/// # Ok(())
/// # }
/// ```
pub struct SearchService<M>(std::marker::PhantomData<M>);

impl<M: PgModel + Sync + Clone + serde::Serialize> SearchService<M> {
    /// Return all rows where any of `cols` matches `term` via `ILIKE '%term%'`.
    ///
    /// Columns are OR-ed: `(col1 ILIKE $1 OR col2 ILIKE $1 OR …)`.
    /// When `cols` is empty, returns all rows.
    pub async fn search(
        term: &str,
        cols: &[&str],
        pool: &sqlx::PgPool,
    ) -> Result<Vec<M>, sqlx::Error> {
        let builder = ilike_builder::<M>(term, cols);
        M::find_where(pool, builder).await
    }

    /// Offset-paginated ILIKE search with a total-count query.
    pub async fn search_paginated(
        term: &str,
        cols: &[&str],
        page: u32,
        per_page: u32,
        pool: &sqlx::PgPool,
    ) -> Result<Page<M>, sqlx::Error> {
        let cols_owned: Vec<String> = cols.iter().map(|&c| c.to_owned()).collect();
        let term_owned = term.to_owned();
        let query = M::all_query()
            .and_where_group(move |b| ilike_into_builder::<M>(b, &term_owned, &cols_owned));
        crate::orm::postgres::pool::with_pool(pool.clone(), query.paginate(per_page, page)).await
    }

    /// Simple (no `COUNT(*)`) paginated ILIKE search.
    pub async fn search_simple_paginated(
        term: &str,
        cols: &[&str],
        page: u32,
        per_page: u32,
        pool: &sqlx::PgPool,
    ) -> Result<SimplePage<M>, sqlx::Error> {
        let cols_owned: Vec<String> = cols.iter().map(|&c| c.to_owned()).collect();
        let term_owned = term.to_owned();
        let query = M::all_query()
            .and_where_group(move |b| ilike_into_builder::<M>(b, &term_owned, &cols_owned));
        crate::orm::postgres::pool::with_pool(pool.clone(), query.simple_paginate(per_page, page))
            .await
    }

    /// PostgreSQL full-text search using `to_tsvector` + `plainto_tsquery`.
    ///
    /// Requires a GIN index on the concatenated column expression for best performance.
    /// Renders: `WHERE to_tsvector('english', col1 || ' ' || col2) @@ plainto_tsquery('english', $1)`
    ///
    /// Falls back to ILIKE when `cols` is empty.
    pub async fn fts(
        term: &str,
        cols: &[&str],
        pool: &sqlx::PgPool,
    ) -> Result<Vec<M>, sqlx::Error> {
        if cols.is_empty() {
            return Self::search(term, &[], pool).await;
        }
        let builder = fts_builder::<M>(term, cols);
        M::find_where(pool, builder).await
    }
}

// ── internal helpers ──────────────────────────────────────────────────────────

fn ilike_builder<M: Model>(term: &str, cols: &[&str]) -> QueryBuilder<M> {
    ilike_into_builder(
        M::query(),
        term,
        &cols.iter().map(|&s| s.to_owned()).collect::<Vec<_>>(),
    )
}

fn ilike_into_builder<M: Model>(
    mut builder: QueryBuilder<M>,
    term: &str,
    cols: &[String],
) -> QueryBuilder<M> {
    if cols.is_empty() {
        return builder;
    }
    let pattern = format!("%{term}%");
    for (i, col) in cols.iter().enumerate() {
        if i == 0 {
            builder = builder.where_ilike(col, &pattern);
        } else {
            builder = builder.or_where_ilike(col, &pattern);
        }
    }
    builder
}

fn fts_builder<M: Model>(term: &str, cols: &[&str]) -> QueryBuilder<M> {
    let concat = cols.join(" || ' ' || ");
    // Single-quote escaping: replace ' with '' for the literal query string.
    let safe_term = term.replace('\'', "''");
    let raw =
        format!("to_tsvector('english', {concat}) @@ plainto_tsquery('english', '{safe_term}')");
    M::query().where_raw(&raw)
}
