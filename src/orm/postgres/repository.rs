//! Repository / DI override — swap the concrete implementation behind
//! [`PgModel`]'s default CRUD methods without touching call sites.
//!
//! Register a [`Repository<M>`] once at startup and every `M::find_by_pk` /
//! `create` / `update_by_pk` / `delete_by_pk` / `all` call transparently
//! delegates to it instead of the built-in `executor::*` path — useful for
//! caching layers, read-replica routing, or test doubles. The lookup uses the
//! same per-type `TypeId` registry pattern as
//! [`orm::scopes`](crate::orm::scopes): registering nothing means zero
//! behavior change and the cost of one `TypeId` hashmap lookup per call.
//!
//! ```rust,no_run
//! # use rok_fluent::orm::postgres::repository::{register, Repository};
//! # use rok_fluent::orm::postgres::model::PgModel;
//! # use rok_fluent::core::model::Model;
//! # use rok_fluent::core::condition::SqlValue;
//! # use sqlx::PgPool;
//! # #[derive(Debug, Clone, sqlx::FromRow)]
//! # pub struct User { pub id: i64, pub name: String }
//! # impl Model for User {
//! #     fn table_name() -> &'static str { "users" }
//! #     fn columns() -> &'static [&'static str] { &["id", "name"] }
//! # }
//! struct FastUserRepo;
//!
//! #[async_trait::async_trait]
//! impl Repository<User> for FastUserRepo {
//!     async fn find_by_pk(
//!         &self,
//!         pool: &PgPool,
//!         id: SqlValue,
//!     ) -> Result<Option<User>, sqlx::Error> {
//!         println!("FastUserRepo::find_by_pk override fired");
//!         User::find_by_pk(pool, id).await
//!     }
//! }
//!
//! register::<User, _>(FastUserRepo);
//! ```

#![allow(dead_code, clippy::type_complexity)]

use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::{Arc, RwLock},
};

use sqlx::PgPool;

use super::model::PgModel;
use crate::core::condition::SqlValue;

// ── Repository trait ──────────────────────────────────────────────────────────

/// Overridable CRUD behavior for a [`PgModel`], dispatched dynamically through
/// `Arc<dyn Repository<M>>` once registered with [`register`].
///
/// Every method has a default body that simply calls through to the
/// corresponding static [`PgModel`] method, so implementers only need to
/// override what they want to change. Uses `#[async_trait]` rather than
/// `async fn in trait` because instances are stored and invoked through
/// `Arc<dyn Repository<M>>` for dynamic dispatch, and RPITIT trait methods
/// are not `dyn`-compatible.
#[async_trait::async_trait]
pub trait Repository<M: PgModel>: Send + Sync + 'static {
    /// Fetch a single row by primary key. Defaults to [`PgModel::find_by_pk`].
    async fn find_by_pk(&self, pool: &PgPool, id: SqlValue) -> Result<Option<M>, sqlx::Error> {
        M::find_by_pk(pool, id).await
    }

    /// Insert a row using the given column-value pairs. Defaults to [`PgModel::create`].
    async fn create(&self, pool: &PgPool, data: &[(&str, SqlValue)]) -> Result<u64, sqlx::Error> {
        M::create(pool, data).await
    }

    /// Update the row with the given primary key. Defaults to [`PgModel::update_by_pk`].
    async fn update_by_pk(
        &self,
        pool: &PgPool,
        id: SqlValue,
        data: &[(&str, SqlValue)],
    ) -> Result<u64, sqlx::Error> {
        M::update_by_pk(pool, id, data).await
    }

    /// Delete the row with the given primary key. Defaults to [`PgModel::delete_by_pk`].
    async fn delete_by_pk(&self, pool: &PgPool, id: SqlValue) -> Result<u64, sqlx::Error> {
        M::delete_by_pk(pool, id).await
    }

    /// Fetch every row from the model's table. Defaults to [`PgModel::all`].
    async fn all(&self, pool: &PgPool) -> Result<Vec<M>, sqlx::Error> {
        M::all(pool).await
    }
}

// ── Internal registry ─────────────────────────────────────────────────────────

/// Per-model repository store — not type-erased.
static TYPED_REGISTRY: std::sync::OnceLock<RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>> =
    std::sync::OnceLock::new();

fn typed_registry() -> &'static RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>> {
    TYPED_REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

struct RepositoryWrapper<M: PgModel>(Arc<dyn Repository<M>>);

// SAFETY: RepositoryWrapper wraps Arc<dyn Repository<M>> which requires Send + Sync.
// `M` never appears as data owned by this wrapper — it is only the generic parameter
// of the `Repository<M>` trait — and every `Repository<M>` implementor is Send + Sync
// by the trait's own supertrait bound, so the type-erased object is sound to send and
// share across threads regardless of whether `M` itself is Send/Sync. This mirrors
// `scopes::ScopeWrapper`, minus its extra `T: Send`/`T: Sync` bounds, which aren't
// required here and would otherwise force every `PgModel` call site to add `M: Sync`.
unsafe impl<M: PgModel> Send for RepositoryWrapper<M> {}
unsafe impl<M: PgModel> Sync for RepositoryWrapper<M> {}

// ── Public API ────────────────────────────────────────────────────────────────

/// Register a [`Repository`] implementation for model `M`.
///
/// Call once at startup (e.g. in `main`). Thread-safe. A later call for the
/// same `M` replaces the previous registration.
pub fn register<M: PgModel, R: Repository<M>>(repo: R) {
    let type_id = TypeId::of::<M>();
    let wrapper: Box<dyn Any + Send + Sync> = Box::new(RepositoryWrapper::<M>(Arc::new(repo)));
    typed_registry().write().unwrap().insert(type_id, wrapper);
}

/// Look up the registered [`Repository`] for model `M`, if any.
///
/// Called internally by [`PgModel`]'s default method bodies before falling
/// through to the existing `executor::*` call.
pub(crate) fn resolve<M: PgModel>() -> Option<Arc<dyn Repository<M>>> {
    let type_id = TypeId::of::<M>();
    let reg = typed_registry().read().unwrap();
    reg.get(&type_id)
        .and_then(|entry| entry.downcast_ref::<RepositoryWrapper<M>>())
        .map(|wrapper| wrapper.0.clone())
}
