//! Scopes V2 — global and local query scopes.
#![allow(dead_code, clippy::type_complexity)]
//!
//! **Local scopes** are plain `impl` methods on the model that return `ModelQuery<Self>`:
//!
//! ```rust,ignore
//! impl User {
//!     pub fn active() -> ModelQuery<User> {
//!         User::filter("active", true)
//!     }
//!     pub fn admins() -> ModelQuery<User> {
//!         User::filter("role", "admin")
//!     }
//! }
//!
//! User::active().admins().order_by_desc("created_at").get().await?
//! ```
//!
//! **Global scopes** are applied automatically to every query via a per-type registry.
//! Register a scope at startup; every `ModelQuery<T>` is created with that scope pre-applied.
//!
//! ```rust,ignore
//! // 1. Define the scope
//! pub struct TenantScope { pub tenant_id: i64 }
//!
//! impl GlobalScope<User> for TenantScope {
//!     fn apply(&self, builder: QueryBuilder<User>) -> QueryBuilder<User> {
//!         builder.where_eq("tenant_id", self.tenant_id)
//!     }
//! }
//!
//! // 2. Register at startup (e.g. main.rs)
//! rok_orm::scopes::register::<User>(TenantScope { tenant_id: 1 });
//!
//! // 3. Every query is now pre-filtered
//! User::all_query().get().await?          // automatically adds WHERE tenant_id = 1
//!
//! // 4. Opt out when needed
//! User::all_query().without_global_scopes().get().await?
//! ```

use std::{
    any::TypeId,
    collections::HashMap,
    sync::{Arc, RwLock},
};

use rok_orm_core::{Model, QueryBuilder};

// ── GlobalScope trait ─────────────────────────────────────────────────────────

/// A scope that is automatically applied to every `QueryBuilder<T>` for a given model.
pub trait GlobalScope<T: Model>: Send + Sync + 'static {
    /// Modify the builder in place, adding the scope's conditions.
    fn apply(&self, builder: QueryBuilder<T>) -> QueryBuilder<T>;
}

// ── Internal registry ─────────────────────────────────────────────────────────

type ScopeFn = Arc<dyn Fn() -> Box<dyn std::any::Any + Send> + Send + Sync>;

struct ScopeEntry {
    apply_fn: Arc<dyn Fn(*mut ()) + Send + Sync>,
}

// We store the scopes as type-erased closures.
// Since QueryBuilder<T> is different for each T, we use a raw-pointer trick.

/// Per-model global scope registry.
///
/// Keyed on `TypeId` of the model type `T`.
static REGISTRY: std::sync::OnceLock<
    RwLock<HashMap<TypeId, Vec<Arc<dyn Fn(*const ()) -> *const () + Send + Sync>>>>,
> = std::sync::OnceLock::new();

fn registry(
) -> &'static RwLock<HashMap<TypeId, Vec<Arc<dyn Fn(*const ()) -> *const () + Send + Sync>>>> {
    REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

// ── Public API ────────────────────────────────────────────────────────────────

/// A type-safe wrapper that stores scopes for a specific model type `T`.
struct TypedScopeEntry<T: Model> {
    scope: Arc<dyn GlobalScope<T>>,
}

/// Per-model scope store — not type-erased.
static TYPED_REGISTRY: std::sync::OnceLock<
    RwLock<HashMap<TypeId, Vec<Box<dyn std::any::Any + Send + Sync>>>>,
> = std::sync::OnceLock::new();

fn typed_registry() -> &'static RwLock<HashMap<TypeId, Vec<Box<dyn std::any::Any + Send + Sync>>>> {
    TYPED_REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

struct ScopeWrapper<T: Model>(Arc<dyn GlobalScope<T>>);

unsafe impl<T: Model + Send> Send for ScopeWrapper<T> {}
unsafe impl<T: Model + Sync> Sync for ScopeWrapper<T> {}

/// Register a global scope for model `T`.
///
/// Call once at startup (e.g. in `main`).  Thread-safe.
pub fn register<T: Model + Send + Sync + 'static>(scope: impl GlobalScope<T>) {
    let type_id = TypeId::of::<T>();
    let wrapper: Box<dyn std::any::Any + Send + Sync> =
        Box::new(ScopeWrapper::<T>(Arc::new(scope)));

    typed_registry()
        .write()
        .unwrap()
        .entry(type_id)
        .or_default()
        .push(wrapper);
}

/// Apply all registered global scopes for model `T` to the given builder.
///
/// Called internally by `PgModel::all_query`, `PgModel::filter`, and `PgModel::find_query`
/// when global scopes are active for `T`.
pub fn apply_scopes<T: Model + Send + Sync + 'static>(
    mut builder: QueryBuilder<T>,
) -> QueryBuilder<T> {
    let type_id = TypeId::of::<T>();
    let reg = typed_registry().read().unwrap();
    if let Some(entries) = reg.get(&type_id) {
        for entry in entries {
            if let Some(wrapper) = entry.downcast_ref::<ScopeWrapper<T>>() {
                builder = wrapper.0.apply(builder);
            }
        }
    }
    builder
}

/// Return `true` if any global scope is registered for model `T`.
pub fn has_scopes<T: Model + 'static>() -> bool {
    let type_id = TypeId::of::<T>();
    typed_registry()
        .read()
        .unwrap()
        .get(&type_id)
        .map(|v| !v.is_empty())
        .unwrap_or(false)
}

/// Remove all global scopes for model `T`.
pub fn clear_scopes<T: Model + 'static>() {
    let type_id = TypeId::of::<T>();
    typed_registry().write().unwrap().remove(&type_id);
}
