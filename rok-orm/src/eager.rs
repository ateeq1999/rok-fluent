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
//! let users = User::all_query().get().await?;
//! let result = rok_orm::with_has_many(
//!     users,
//!     "user_id",                   // FK column on Post table
//!     |post: &Post| post.user_id,  // extract FK from child
//!     |user: &User| user.id,       // extract PK from parent
//! ).await?;
//! for row in &result {
//!     println!("{} has {} posts", row.model.name, row.related.len());
//! }
//! ```
//!
//! ## String-keyed `EagerLoadable` trait
//!
//! Implement `EagerLoadable` on your model to load relations by name.
//! The model must have optional fields to hold the loaded data.
//!
//! ```rust,ignore
//! use rok_orm::{EagerLoadable, with_has_many};
//!
//! pub struct User {
//!     pub id: i64,
//!     pub name: String,
//!     // populated by eager loading
//!     pub posts: Option<Vec<Post>>,
//! }
//!
//! impl EagerLoadable for User {
//!     async fn load_eager(relation: &str, parents: Vec<Self>) -> Result<Vec<Self>, sqlx::Error> {
//!         match relation {
//!             "posts" => {
//!                 let rows = with_has_many(parents, "user_id",
//!                     |p: &Post| p.user_id,
//!                     |u: &User| u.id,
//!                 ).await?;
//!                 Ok(rows.into_iter().map(|row| {
//!                     let mut user = row.model;
//!                     user.posts = Some(row.related);
//!                     user
//!                 }).collect())
//!             }
//!             _ => Ok(parents),
//!         }
//!     }
//! }
//!
//! // Usage:
//! let users = rok_orm::eager_get(User::all_query(), &["posts"]).await?;
//! ```

#[cfg(feature = "postgres")]
use rok_orm_core::{Model, SqlValue};
use std::collections::HashMap;
#[cfg(feature = "postgres")]
use std::collections::HashSet;

// ── Result types ──────────────────────────────────────────────────────────────

/// A parent model paired with all its eagerly-loaded has-many children.
#[derive(Debug)]
pub struct WithMany<P, C> {
    pub model: P,
    pub related: Vec<C>,
}

/// A parent model paired with its eagerly-loaded has-one / belongs-to child.
#[derive(Debug)]
pub struct WithOne<P, C> {
    pub model: P,
    pub related: Option<C>,
}

// ── Grouping utilities ────────────────────────────────────────────────────────

/// Group children into parents by matching FK on children to PK on parents.
///
/// Useful after loading both collections independently.
///
/// ```rust,ignore
/// let users = User::all_query().get().await?;
/// let user_ids: Vec<i64> = users.iter().map(|u| u.id).collect();
/// let posts = Post::all_query().and_where_in("user_id", user_ids).get().await?;
/// let result = rok_orm::group_has_many(users, posts, |p| p.user_id, |u| u.id);
/// ```
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
///
/// ```rust,ignore
/// let result = rok_orm::group_has_one(users, profiles, |pr| pr.user_id, |u| u.id);
/// ```
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
///
/// Runs two queries total: one for parents (already fetched) and one `IN` query for children.
///
/// ```rust,ignore
/// let users = User::all_query().get().await?;
/// let result = rok_orm::with_has_many(
///     users,
///     "user_id",
///     |post: &Post| post.user_id,
///     |user: &User| user.id,
/// ).await?;
/// ```
#[cfg(feature = "postgres")]
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
    let children = crate::model_query::ModelQuery::<C>::new(C::query())
        .and_where_in(fk_col, pk_vals)
        .get()
        .await?;
    Ok(group_has_many(parents, children, fk_getter, pk_getter))
}

/// Batch-load has-one children for a collection of parents in a single `IN` query.
///
/// ```rust,ignore
/// let users = User::all_query().get().await?;
/// let result = rok_orm::with_has_one(
///     users,
///     "user_id",
///     |profile: &Profile| profile.user_id,
///     |user: &User| user.id,
/// ).await?;
/// ```
#[cfg(feature = "postgres")]
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
    let children = crate::model_query::ModelQuery::<C>::new(C::query())
        .and_where_in(fk_col, pk_vals)
        .get()
        .await?;
    Ok(group_has_one(parents, children, fk_getter, pk_getter))
}

/// Batch-load belongs-to parents for a collection of child models in a single `IN` query.
///
/// Returns `(child, Option<parent>)` pairs.
///
/// ```rust,ignore
/// let posts = Post::all_query().get().await?;
/// let result = rok_orm::with_belongs_to::<Post, User, i64>(
///     posts,
///     "id",                        // PK column on parent (User) table
///     |post: &Post| post.user_id,  // FK on child pointing to parent PK
///     |user: &User| user.id,       // PK of parent
/// ).await?;
/// for (post, user_opt) in &result {
///     if let Some(user) = user_opt {
///         println!("'{}' by {}", post.title, user.name);
///     }
/// }
/// ```
#[cfg(feature = "postgres")]
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

    // Collect unique FK values for the IN query
    let mut seen: HashSet<K> = HashSet::new();
    let mut fk_vals: Vec<K> = Vec::new();
    for c in &children {
        let k = fk_getter(c);
        if seen.insert(k.clone()) {
            fk_vals.push(k);
        }
    }

    let parents = crate::model_query::ModelQuery::<P>::new(P::query())
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
#[cfg(feature = "postgres")]
pub trait EagerLoadable: Sized + Send + Sync + 'static {
    /// Load the named relation into the given parent models.
    ///
    /// Called once per relation name passed to [`eager_get`].
    /// Return `Ok(parents)` unchanged for unknown relation names.
    fn load_eager(
        relation: &str,
        parents: Vec<Self>,
    ) -> impl std::future::Future<Output = Result<Vec<Self>, sqlx::Error>> + Send;
}

/// Fetch rows and eagerly load the given relations via [`EagerLoadable`].
///
/// ```rust,ignore
/// let users = rok_orm::eager_get(User::all_query(), &["posts", "profile"]).await?;
/// ```
#[cfg(feature = "postgres")]
pub async fn eager_get<M>(
    query: crate::model_query::ModelQuery<M>,
    relations: &[&str],
) -> Result<Vec<M>, sqlx::Error>
where
    M: EagerLoadable
        + rok_orm_core::Model
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
///
/// Load relations in batches after fetching the parent models — no N+1 queries.
///
/// # Example
///
/// ```rust,ignore
/// use rok_orm::LazyLoadable;
///
/// let posts = Post::all().await?;
/// let posts = posts.load("comments").await?;
/// let posts = posts.load("tags").await?;
/// ```
#[cfg(feature = "postgres")]
pub trait LazyLoadable: Sized {
    /// Load the named relation into this collection.
    ///
    /// Internally calls [`EagerLoadable::load_eager`] on the full slice — one
    /// batch query per relation, regardless of collection size.
    fn load(
        self,
        relation: &str,
    ) -> impl std::future::Future<Output = Result<Self, sqlx::Error>> + Send;
}

#[cfg(feature = "postgres")]
impl<M> LazyLoadable for Vec<M>
where
    M: EagerLoadable
        + rok_orm_core::Model
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

/// An intermediate query type returned by [`ModelQuery::with`].
///
/// Chain `.with()` calls to add more relations, then call `.get()` to execute.
/// The model `M` must implement [`EagerLoadable`].
///
/// # Example
///
/// ```rust,ignore
/// use rok_orm::EagerLoadable;
///
/// let users = User::all_query()
///     .with("posts")
///     .with("profile")
///     .get()
///     .await?;
/// ```
#[cfg(feature = "postgres")]
pub struct EagerModelQuery<M> {
    pub(crate) query: crate::model_query::ModelQuery<M>,
    pub(crate) relations: Vec<String>,
}

#[cfg(feature = "postgres")]
impl<M> EagerModelQuery<M>
where
    M: EagerLoadable
        + rok_orm_core::Model
        + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>
        + Send
        + Sync
        + Unpin
        + 'static,
{
    /// Add another relation to eagerly load.
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
