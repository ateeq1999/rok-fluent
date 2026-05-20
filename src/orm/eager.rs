//! Eager loading — batch-load relationships to avoid N+1 queries.
//!
//! # Two complementary APIs
//!
//! ## Low-level helpers (recommended for most cases)
//!
//! Fetch parents, then use `with_has_many` / `with_has_one` / `with_belongs_to`
//! to batch-load related rows in a single `IN` query.
//!
//! ```rust,ignore
//! # use rok_fluent::orm::eager::{with_has_many, group_has_many};
//! # use rok_fluent::core::model::Model;
//! # #[derive(Debug, sqlx::FromRow)]
//! # pub struct User { pub id: i64, pub name: String }
//! # impl Model for User {
//! #     fn table_name() -> &'static str { "users" }
//! #     fn columns() -> &'static [&'static str] { &["id", "name"] }
//! # }
//! # #[derive(Debug, sqlx::FromRow)]
//! # pub struct Post { pub id: i64, pub user_id: i64 }
//! # impl Model for Post {
//! #     fn table_name() -> &'static str { "posts" }
//! #     fn columns() -> &'static [&'static str] { &["id", "user_id"] }
//! # }
//! # async fn example() -> Result<(), sqlx::Error> {
//! # let users: Vec<User> = vec![];
//! let result = with_has_many(
//!     users,
//!     "user_id",
//!     |post: &Post| post.user_id,
//!     |user: &User| user.id,
//! ).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## String-keyed `EagerLoadable` trait
//!
//! Implement `EagerLoadable` on your model to load relations by name.

use std::collections::HashMap;

#[cfg(all(feature = "active", feature = "postgres"))]
use std::collections::HashSet;

#[cfg(all(feature = "active", feature = "postgres"))]
use crate::core::condition::SqlValue;
#[cfg(all(feature = "active", feature = "postgres"))]
use crate::core::model::Model;

// ── Result types ──────────────────────────────────────────────────────────────

/// A parent model paired with all its eagerly-loaded has-many children.
#[derive(Debug)]
pub struct WithMany<P, C> {
    /// The parent model.
    pub model: P,
    /// All related child rows.
    pub related: Vec<C>,
}

/// A parent model paired with its eagerly-loaded has-one / belongs-to child.
#[derive(Debug)]
pub struct WithOne<P, C> {
    /// The parent model.
    pub model: P,
    /// The optional related row.
    pub related: Option<C>,
}

// ── Grouping utilities ────────────────────────────────────────────────────────

/// Group children into parents by matching FK on children to PK on parents.
pub fn group_has_many<P, C, K>(
    parents: Vec<P>,
    children: Vec<C>,
    fk_getter: impl Fn(&C) -> K,
    pk_getter: impl Fn(&P) -> K,
) -> Vec<WithMany<P, C>>
where
    K: Eq + std::hash::Hash,
{
    let mut result: Vec<WithMany<P, C>> = parents
        .into_iter()
        .map(|m| WithMany {
            model: m,
            related: Vec::new(),
        })
        .collect();

    let pk_to_idx: HashMap<K, usize> = result
        .iter()
        .enumerate()
        .map(|(i, r)| (pk_getter(&r.model), i))
        .collect();

    for child in children {
        let fk = fk_getter(&child);
        if let Some(&idx) = pk_to_idx.get(&fk) {
            result[idx].related.push(child);
        }
    }
    result
}

/// Group at most one child per parent (has-one / belongs-to from parent side).
pub fn group_has_one<P, C, K>(
    parents: Vec<P>,
    children: Vec<C>,
    fk_getter: impl Fn(&C) -> K,
    pk_getter: impl Fn(&P) -> K,
) -> Vec<WithOne<P, C>>
where
    K: Eq + std::hash::Hash,
{
    let mut result: Vec<WithOne<P, C>> = parents
        .into_iter()
        .map(|m| WithOne {
            model: m,
            related: None,
        })
        .collect();

    let pk_to_idx: HashMap<K, usize> = result
        .iter()
        .enumerate()
        .map(|(i, r)| (pk_getter(&r.model), i))
        .collect();

    for child in children {
        let fk = fk_getter(&child);
        if let Some(&idx) = pk_to_idx.get(&fk) {
            if result[idx].related.is_none() {
                result[idx].related = Some(child);
            }
        }
    }
    result
}

// ── Batch loaders ─────────────────────────────────────────────────────────────

/// Batch-load has-many children for a collection of parents in a single `IN` query.
#[cfg(all(feature = "active", feature = "postgres"))]
pub async fn with_has_many<P, C, K>(
    parents: Vec<P>,
    fk_col: &str,
    fk_getter: impl Fn(&C) -> K,
    pk_getter: impl Fn(&P) -> K,
) -> Result<Vec<WithMany<P, C>>, sqlx::Error>
where
    P: Model + Send + Sync + 'static,
    C: Model + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Sync + Unpin + 'static,
    K: Eq + std::hash::Hash + Into<SqlValue> + Clone,
{
    if parents.is_empty() {
        return Ok(Vec::new());
    }
    let pk_vals: Vec<K> = parents.iter().map(&pk_getter).collect();
    let children = crate::orm::model_query::ModelQuery::<C>::new(C::query())
        .and_where_in(fk_col, pk_vals)
        .get()
        .await?;
    Ok(group_has_many(parents, children, fk_getter, pk_getter))
}

/// Batch-load has-one children for a collection of parents in a single `IN` query.
#[cfg(all(feature = "active", feature = "postgres"))]
pub async fn with_has_one<P, C, K>(
    parents: Vec<P>,
    fk_col: &str,
    fk_getter: impl Fn(&C) -> K,
    pk_getter: impl Fn(&P) -> K,
) -> Result<Vec<WithOne<P, C>>, sqlx::Error>
where
    P: Model + Send + Sync + 'static,
    C: Model + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Sync + Unpin + 'static,
    K: Eq + std::hash::Hash + Into<SqlValue> + Clone,
{
    if parents.is_empty() {
        return Ok(Vec::new());
    }
    let pk_vals: Vec<K> = parents.iter().map(&pk_getter).collect();
    let children = crate::orm::model_query::ModelQuery::<C>::new(C::query())
        .and_where_in(fk_col, pk_vals)
        .get()
        .await?;
    Ok(group_has_one(parents, children, fk_getter, pk_getter))
}

/// Batch-load belongs-to parents for a collection of child models in a single `IN` query.
///
/// Returns `(child, Option<parent>)` pairs.
#[cfg(all(feature = "active", feature = "postgres"))]
pub async fn with_belongs_to<C, P, K>(
    children: Vec<C>,
    parent_pk_col: &str,
    fk_getter: impl Fn(&C) -> K,
    pk_getter: impl Fn(&P) -> K,
) -> Result<Vec<(C, Option<P>)>, sqlx::Error>
where
    C: Send + 'static,
    P: Model
        + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>
        + Clone
        + Send
        + Sync
        + Unpin
        + 'static,
    K: Eq + std::hash::Hash + Into<SqlValue> + Clone,
{
    if children.is_empty() {
        return Ok(Vec::new());
    }

    let mut seen: HashSet<K> = HashSet::new();
    let mut fk_vals: Vec<K> = Vec::new();
    for c in &children {
        let k = fk_getter(c);
        if seen.insert(k.clone()) {
            fk_vals.push(k);
        }
    }

    let parents = crate::orm::model_query::ModelQuery::<P>::new(P::query())
        .and_where_in(parent_pk_col, fk_vals)
        .get()
        .await?;

    let parent_map: HashMap<K, P> = parents.into_iter().map(|p| (pk_getter(&p), p)).collect();

    Ok(children
        .into_iter()
        .map(|c| {
            let fk = fk_getter(&c);
            let parent = parent_map.get(&fk).cloned();
            (c, parent)
        })
        .collect())
}

// ── EagerLoadable trait ───────────────────────────────────────────────────────

/// Trait for string-keyed eager loading.
///
/// Implement this on a model that has optional fields to hold loaded relations.
/// Then use [`eager_get`] to load relations by name after the main query.
#[cfg(all(feature = "active", feature = "postgres"))]
pub trait EagerLoadable: Sized + Send + Sync + 'static {
    /// Load the named relation into the given parent models.
    fn load_eager(
        relation: &str,
        parents: Vec<Self>,
    ) -> impl std::future::Future<Output = Result<Vec<Self>, sqlx::Error>> + Send;
}

/// Fetch rows and eagerly load the given relations via [`EagerLoadable`].
#[cfg(all(feature = "active", feature = "postgres"))]
pub async fn eager_get<M>(
    query: crate::orm::model_query::ModelQuery<M>,
    relations: &[&str],
) -> Result<Vec<M>, sqlx::Error>
where
    M: EagerLoadable
        + crate::core::model::Model
        + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>
        + Send
        + Sync
        + Unpin
        + 'static,
{
    let mut results = query.get().await?;
    for rel in relations {
        results = M::load_eager(rel, results).await?;
    }
    Ok(results)
}

// ── Lazy eager loading ────────────────────────────────────────────────────────

/// Extension trait that adds `.load("relation")` to `Vec<M>` after an initial query.
#[cfg(all(feature = "active", feature = "postgres"))]
pub trait LazyLoadable: Sized {
    /// Load the named relation into this collection.
    fn load(
        self,
        relation: &str,
    ) -> impl std::future::Future<Output = Result<Self, sqlx::Error>> + Send;
}

#[cfg(all(feature = "active", feature = "postgres"))]
impl<M> LazyLoadable for Vec<M>
where
    M: EagerLoadable
        + crate::core::model::Model
        + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>
        + Send
        + Sync
        + Unpin
        + 'static,
{
    fn load(
        self,
        relation: &str,
    ) -> impl std::future::Future<Output = Result<Self, sqlx::Error>> + Send {
        M::load_eager(relation, self)
    }
}

// ── EagerModelQuery ───────────────────────────────────────────────────────────

/// An intermediate query type returned by [`ModelQuery::with`](crate::orm::model_query::ModelQuery::with).
///
/// Chain `.with()` calls to add more relations, then call `.get()` to execute.
/// The model `M` must implement [`EagerLoadable`].
#[cfg(all(feature = "active", feature = "postgres"))]
pub struct EagerModelQuery<M> {
    pub(crate) query: crate::orm::model_query::ModelQuery<M>,
    pub(crate) relations: Vec<String>,
}

#[cfg(all(feature = "active", feature = "postgres"))]
impl<M> EagerModelQuery<M>
where
    M: EagerLoadable
        + crate::core::model::Model
        + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>
        + Send
        + Sync
        + Unpin
        + 'static,
{
    /// Add another relation to eagerly load.
    #[must_use]
    pub fn with(mut self, relation: impl Into<String>) -> Self {
        self.relations.push(relation.into());
        self
    }

    /// Execute the query and eagerly load all registered relations.
    pub async fn get(self) -> Result<Vec<M>, sqlx::Error> {
        let refs: Vec<&str> = self.relations.iter().map(String::as_str).collect();
        eager_get(self.query, &refs).await
    }
}
