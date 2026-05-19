//! rok-orm — Eloquent-inspired ORM for the rok ecosystem.
//!
//! # Quick start
//!
//! ```rust
//! use rok_orm::Model;
//!
//! #[derive(Model)]
//! pub struct User {
//!     pub id: i64,
//!     pub name: String,
//!     pub email: String,
//! }
//!
//! // Generated table name and columns
//! assert_eq!(User::table_name(), "users");
//! assert_eq!(User::columns(), &["id", "name", "email"]);
//!
//! // Build a query
//! let (sql, params) = User::query()
//!     .where_eq("active", true)
//!     .order_by_desc("created_at")
//!     .limit(10)
//!     .to_sql();
//!
//! assert!(sql.contains("FROM users"));
//! assert!(sql.contains("LIMIT 10"));
//! ```

// Re-export core types
pub use rok_orm_core::{Condition, Dialect, Join, JoinOp, Model, OrderDir, QueryBuilder, SqlValue};

// Re-export derive macros. Enable with `features = ["macros"]` (on by default).
#[cfg(feature = "macros")]
pub use rok_orm_macros::Model;
#[cfg(feature = "macros")]
pub use rok_orm_macros::Resource;
#[cfg(feature = "macros")]
pub use rok_orm_macros::Seed;
#[cfg(feature = "macros")]
pub use rok_orm_macros::query;

/// Async PostgreSQL executor. Enable with `features = ["postgres"]`.
#[cfg(feature = "postgres")]
pub mod executor;

/// Optional query logging middleware. Enable with `features = ["postgres"]`.
#[cfg(feature = "postgres")]
pub mod query_log;

/// Ergonomic async CRUD trait. Enable with `features = ["postgres"]`.
#[cfg(feature = "postgres")]
pub mod pg_model;

/// Pool-free fluent query type. Enable with `features = ["postgres"]`.
#[cfg(feature = "postgres")]
pub mod model_query;

/// Task-local pool storage. Enable with `features = ["postgres"]`.
#[cfg(feature = "postgres")]
pub mod pool;

/// PostgreSQL transaction wrapper. Enable with `features = ["postgres"]`.
#[cfg(feature = "postgres")]
pub mod transaction;

#[cfg(feature = "postgres")]
pub use model_query::ModelQuery;

#[cfg(feature = "postgres")]
pub use pg_model::PgModel;

#[cfg(feature = "postgres")]
pub use transaction::Tx;

#[cfg(feature = "postgres")]
pub use executor::RetryConfig;

#[cfg(feature = "postgres")]
pub use query_log::{set_logger, set_slow_threshold, QueryLogEntry, QueryLogger};

#[cfg(feature = "postgres")]
pub use pool::{get_named_pool, register_named_pool, unregister_named_pool, snapshot as pool_snapshot, PoolMetrics};

#[cfg(all(feature = "postgres", feature = "metrics"))]
pub use pool::emit_metrics as emit_pool_metrics;

/// In-memory model collection helpers.
pub mod collection;
pub use collection::{IntoCollection, ModelCollection};

/// Pagination types — `Page<T>`, `SimplePage<T>`, `CursorPage<T>`.
pub mod pagination;
pub use pagination::{CursorPage, Page, SimplePage};

/// Global and local scope system.
pub mod scopes;
pub use scopes::GlobalScope;

/// Model hooks and observers — lifecycle callbacks.
pub mod hooks;
pub use hooks::{observe, without_events, ModelHooks, Observer, OrmError, OrmResult};

/// Eager loading — batch-load relationships to avoid N+1 queries.
pub mod eager;
#[cfg(feature = "postgres")]
pub use eager::EagerModelQuery;
#[cfg(feature = "postgres")]
pub use eager::{eager_get, with_belongs_to, with_has_many, with_has_one, EagerLoadable, LazyLoadable};
pub use eager::{group_has_many, group_has_one, WithMany, WithOne};

/// Field casts — convert model fields to/from database representations.
pub mod casts;
pub use casts::{Cast, CastArray, CastBool, CastCommaList, CastDate, CastDatetime, CastJson, CastUuid};

/// API resources — transform ORM models into JSON-friendly response shapes.
pub mod resource;
pub use resource::{PageResource, Resource, ResourceCollection};

// ── Raw query escape hatches ──────────────────────────────────────────────────

/// Execute a raw `SELECT` and return raw `sqlx::postgres::PgRow` rows.
///
/// This is the low-level escape hatch. For JSON output use a typed query instead.
///
/// ```rust,ignore
/// let rows = rok_orm::raw_rows("SELECT id FROM users WHERE active = $1", vec![true.into()]).await?;
/// ```
#[cfg(feature = "postgres")]
pub async fn raw_rows(
    sql: &str,
    params: Vec<SqlValue>,
) -> Result<Vec<sqlx::postgres::PgRow>, sqlx::Error> {
    use rok_orm_core::sqlx_pg;
    let pool = pool::try_current_pool()
        .ok_or_else(|| sqlx::Error::Configuration("no pool in scope".to_string().into()))?;
    sqlx_pg::build_query(sql, params).fetch_all(&pool).await
}

/// Execute a raw SQL statement (INSERT / UPDATE / DELETE) and return rows affected.
///
/// ```rust,ignore
/// rok_orm::execute("UPDATE users SET synced_at = NOW() WHERE active = $1", vec![true.into()]).await?;
/// ```
#[cfg(feature = "postgres")]
pub async fn execute_sql(sql: &str, params: Vec<SqlValue>) -> Result<u64, sqlx::Error> {
    let pool = pool::try_current_pool()
        .ok_or_else(|| sqlx::Error::Configuration("no pool in scope".to_string().into()))?;
    executor::execute_raw(&pool, sql, params).await
}

/// Many-to-many pivot table query. Enable with `features = ["postgres"]`.
#[cfg(feature = "postgres")]
pub mod pivot_query;

#[cfg(feature = "postgres")]
pub use pivot_query::PivotQuery;

/// Indirect "has many through" / "has one through" relationship query.
#[cfg(feature = "postgres")]
pub mod through;

#[cfg(feature = "postgres")]
pub use through::ThroughQuery;

/// Polymorphic relationship types: MorphTo, MorphMany, MorphOne, MorphToMany.
#[cfg(feature = "postgres")]
pub mod morph;

#[cfg(feature = "postgres")]
pub use morph::{MorphMany, MorphOne, MorphTo, MorphToMany};

/// A relationship query — type alias for [`ModelQuery`] returned by `has_many!`, `has_one!`,
/// and `belongs_to!` macros.
#[cfg(feature = "postgres")]
pub type RelationQuery<T> = model_query::ModelQuery<T>;

/// N+1 query detector — emits warnings in development when a table is queried too often.
pub mod n1;

/// Tower middleware that scopes a pool to each request. Enable with `features = ["axum"]`.
#[cfg(feature = "axum")]
pub mod orm_layer;

#[cfg(feature = "axum")]
pub use orm_layer::OrmLayer;

/// Async SQLite executor. Enable with `features = ["sqlite"]`.
#[cfg(feature = "sqlite")]
pub mod sqlite_executor;

/// Ergonomic async CRUD trait for SQLite. Enable with `features = ["sqlite"]`.
#[cfg(feature = "sqlite")]
pub mod sqlite_model;

#[cfg(feature = "sqlite")]
pub use sqlite_model::SqliteModel;

/// Async MySQL executor. Enable with `features = ["mysql"]`.
#[cfg(feature = "mysql")]
pub mod mysql_executor;

/// Ergonomic async CRUD trait for MySQL. Enable with `features = ["mysql"]`.
#[cfg(feature = "mysql")]
pub mod mysql_model;

#[cfg(feature = "mysql")]
pub use mysql_model::MysqlModel;

// ── Relationship macros ───────────────────────────────────────────────────────

/// Create a `has_many` relationship query — returns a [`RelationQuery`] of
/// all rows in `$Model` where `$fk = self.pk_value()`.
///
/// # Example
///
/// ```rust,ignore
/// impl User {
///     pub fn posts(&self) -> RelationQuery<Post> {
///         has_many!(self, Post, "user_id")
///     }
/// }
/// ```
#[macro_export]
macro_rules! has_many {
    ($self:expr, $model:ty, $fk:expr) => {
        <$model as $crate::PgModel>::filter($fk, $crate::Model::pk_value($self))
    };
}

/// Create a `has_one` relationship query — returns a [`RelationQuery`] of the
/// first row in `$Model` where `$fk = self.pk_value()`.
///
/// Callers typically chain `.first()` to unwrap the single result.
///
/// # Example
///
/// ```rust,ignore
/// impl User {
///     pub fn profile(&self) -> RelationQuery<Profile> {
///         has_one!(self, Profile, "user_id")
///     }
/// }
/// ```
#[macro_export]
macro_rules! has_one {
    ($self:expr, $model:ty, $fk:expr) => {
        <$model as $crate::PgModel>::filter($fk, $crate::Model::pk_value($self))
    };
}

/// Create a `belongs_to` relationship query — returns a [`RelationQuery`] for
/// the parent row whose primary key equals the foreign key value on `self`.
///
/// Pass the actual foreign key *value* (not the column name) as the third arg.
///
/// # Example
///
/// ```rust,ignore
/// impl Post {
///     pub fn user(&self) -> RelationQuery<User> {
///         belongs_to!(self, User, self.user_id)
///     }
/// }
/// ```
#[macro_export]
macro_rules! belongs_to {
    ($self:expr, $model:ty, $fk_value:expr) => {
        <$model as $crate::PgModel>::find_query($fk_value)
    };
}

/// Create a `belongs_to_many` pivot relationship query — returns a [`PivotQuery`].
///
/// # Arguments
///
/// - `$self`    — owning model instance
/// - `$model`   — related model type
/// - `$through` — pivot table name (string literal)
/// - `$fk`      — FK on the pivot table pointing to the owner (string literal)
/// - `$rfk`     — FK on the pivot table pointing to the related model (string literal)
///
/// # Example
///
/// ```rust,ignore
/// impl Post {
///     pub fn tags(&self) -> PivotQuery<Tag> {
///         belongs_to_many!(self, Tag, "post_tags", "post_id", "tag_id")
///     }
/// }
/// ```
#[macro_export]
macro_rules! belongs_to_many {
    ($self:expr, $model:ty, $through:expr, $fk:expr, $rfk:expr) => {
        $crate::PivotQuery::<$model>::new($crate::Model::pk_value($self), $through, $fk, $rfk)
    };
}

/// Create a `has_many_through` relationship — indirect relationship via a bridge table.
///
/// # Arguments
///
/// - `$self`       — owning model instance
/// - `$target`     — target model type
/// - `$through`    — bridge table name (string literal, e.g. `"users"`)
/// - `$first_key`  — FK on the bridge table pointing to the owner (e.g. `"country_id"`)
/// - `$second_key` — FK on the target table pointing to the bridge table PK (e.g. `"user_id"`)
///
/// # Example
///
/// ```rust,ignore
/// impl Country {
///     pub fn posts(&self) -> ThroughQuery<Post> {
///         has_many_through!(self, Post, "users", "country_id", "user_id")
///     }
/// }
///
/// let posts = country.posts().where_eq("published", true).get().await?;
/// ```
#[macro_export]
macro_rules! has_many_through {
    ($self:expr, $target:ty, $through:expr, $first_key:expr, $second_key:expr) => {
        $crate::ThroughQuery::<$target>::new(
            $crate::Model::pk_value($self),
            $through,
            $first_key,
            $second_key,
        )
    };
}

/// Create a `has_one_through` relationship.
///
/// Same as [`has_many_through!`] — call `.first()` on the result to get `Option<T>`.
///
/// # Example
///
/// ```rust,ignore
/// impl Mechanic {
///     pub fn car_owner(&self) -> ThroughQuery<User> {
///         has_one_through!(self, User, "cars", "mechanic_id", "owner_id")
///     }
/// }
///
/// let owner: Option<User> = mechanic.car_owner().first().await?;
/// ```
#[macro_export]
macro_rules! has_one_through {
    ($self:expr, $target:ty, $through:expr, $first_key:expr, $second_key:expr) => {
        $crate::ThroughQuery::<$target>::new(
            $crate::Model::pk_value($self),
            $through,
            $first_key,
            $second_key,
        )
    };
}

/// Create a `morph_many` relationship — polymorphic "has many".
///
/// # Arguments
///
/// - `$self`       — owning model instance
/// - `$target`     — child model type
/// - `$owner_type` — morph type string for this owner (e.g. `"post"`)
/// - `$key`        — morph key prefix on the child table (e.g. `"commentable"`)
///
/// # Example
///
/// ```rust,ignore
/// impl Post {
///     pub fn comments(&self) -> MorphMany<Comment> {
///         morph_many!(self, Comment, "post", "commentable")
///     }
/// }
/// let all = post.comments().get().await?;
/// ```
#[macro_export]
macro_rules! morph_many {
    ($self:expr, $target:ty, $owner_type:expr, $key:expr) => {
        $crate::MorphMany::<$target>::new($owner_type, $crate::Model::pk_value($self), $key)
    };
}

/// Create a `morph_one` relationship — polymorphic "has one".
///
/// # Example
///
/// ```rust,ignore
/// impl Post {
///     pub fn cover_image(&self) -> MorphOne<Image> {
///         morph_one!(self, Image, "post", "imageable")
///     }
/// }
/// let img: Option<Image> = post.cover_image().get().await?;
/// ```
#[macro_export]
macro_rules! morph_one {
    ($self:expr, $target:ty, $owner_type:expr, $key:expr) => {
        $crate::MorphOne::<$target>::new($owner_type, $crate::Model::pk_value($self), $key)
    };
}

/// Create a `morph_many_to_many` pivot relationship (taggable pattern).
///
/// # Arguments
///
/// - `$self`       — owning model instance
/// - `$target`     — related model type
/// - `$owner_type` — morph type string for this owner (e.g. `"post"`)
/// - `$pivot`      — pivot table name (e.g. `"taggables"`)
/// - `$key`        — morph key prefix on the pivot (e.g. `"taggable"`)
/// - `$related_fk` — FK on pivot pointing to the related model (e.g. `"tag_id"`)
///
/// # Example
///
/// ```rust,ignore
/// impl Post {
///     pub fn tags(&self) -> MorphToMany<Tag> {
///         morph_many_to_many!(self, Tag, "post", "taggables", "taggable", "tag_id")
///     }
/// }
/// let tags = post.tags().get().await?;
/// post.tags().attach(tag_id).await?;
/// post.tags().sync(&[1i64, 2, 3]).await?;
/// ```
#[macro_export]
macro_rules! morph_many_to_many {
    ($self:expr, $target:ty, $owner_type:expr, $pivot:expr, $key:expr, $related_fk:expr) => {
        $crate::MorphToMany::<$target>::new(
            $owner_type,
            $crate::Model::pk_value($self),
            $pivot,
            $key,
            $related_fk,
        )
    };
}

// ── Accessor / mutator macros ─────────────────────────────────────────────────

/// Define a computed getter (accessor) on a model.
///
/// Expands to `pub fn $fn_name(&self) -> ReturnType { body }`.
///
/// By convention name the function `get_<field>_attribute`.
///
/// # Example
///
/// ```rust,ignore
/// impl User {
///     accessor!(get_full_name_attribute, &self -> String,
///         format!("{} {}", self.first_name, self.last_name));
/// }
/// let name = user.get_full_name_attribute();
/// ```
#[macro_export]
macro_rules! accessor {
    ($fn_name:ident, &$self:ident -> $ret:ty, $body:expr) => {
        pub fn $fn_name(&$self) -> $ret {
            $body
        }
    };
}

/// Run `EXPLAIN ANALYZE` on a `ModelQuery` and return the plan text + estimated cost.
///
/// The macro accepts a `ModelQuery` expression (not yet awaited).  It extracts the
/// generated SQL, prepends `EXPLAIN (ANALYZE, BUFFERS, FORMAT TEXT)`, and executes
/// it against the current pool.
///
/// # Example
///
/// ```rust,ignore
/// let (plan, cost) = explain!(User::query().where_eq("active", true)).await?;
/// println!("Estimated cost: {cost:.2}");
/// println!("{plan}");
/// ```
///
/// Returns `(plan_text: String, estimated_cost: f64)`.
#[cfg(feature = "postgres")]
#[macro_export]
macro_rules! explain {
    ($query:expr) => {{
        async {
            use $crate::pool;
            use $crate::SqlValue;
            let qb = $query.builder();
            let (sql, params) = qb.to_sql();
            let explain_sql = format!("EXPLAIN (ANALYZE, BUFFERS, FORMAT TEXT) {sql}");
            let rows = $crate::raw_rows(&explain_sql, params).await?;
            use sqlx::Row as _;
            let lines: Vec<String> = rows
                .iter()
                .map(|r| r.try_get::<String, _>(0).unwrap_or_default())
                .collect();
            let plan_text = lines.join("\n");
            let cost: f64 = lines
                .first()
                .and_then(|l| {
                    l.split("cost=")
                        .nth(1)
                        .and_then(|s| s.split("..").nth(1))
                        .and_then(|s| s.split_whitespace().next())
                        .and_then(|s| s.parse().ok())
                })
                .unwrap_or(0.0);
            Ok::<(String, f64), sqlx::Error>((plan_text, cost))
        }
    }};
}

/// Define a setter (mutator) on a model.
///
/// Expands to `pub fn $fn_name(&mut self, $arg: Type) { body }`.
///
/// By convention name the function `set_<field>_attribute`.
///
/// # Example
///
/// ```rust,ignore
/// impl User {
///     mutator!(set_email_attribute, &mut self, val: String, {
///         self.email = val.to_lowercase();
///     });
/// }
/// user.set_email_attribute("ALICE@example.com".to_string());
/// ```
#[macro_export]
macro_rules! mutator {
    ($fn_name:ident, &mut $self:ident, $arg:ident: $arg_ty:ty, $body:block) => {
        pub fn $fn_name(&mut $self, $arg: $arg_ty) {
            $body
        }
    };
}
