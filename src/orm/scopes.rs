//! Scopes — global and local query scopes.
//!
//! **Local scopes** are plain `impl` methods on the model that return `ModelQuery<Self>`.
//!
//! **Global scopes** are applied automatically to every query via a per-type registry.
//! Register a scope at startup; every `ModelQuery<T>` is created with that scope pre-applied.
//!
//! ```rust,no_run
//! # use rok_fluent::orm::scopes::{GlobalScope, register};
//! # use rok_fluent::core::query::QueryBuilder;
//! # struct User;
//! # impl rok_fluent::core::model::Model for User {
//! #     fn table_name() -> &'static str { "users" }
//! #     fn columns() -> &'static [&'static str] { &["id"] }
//! # }
//! pub struct TenantScope { pub tenant_id: i64 }
//!
//! impl GlobalScope<User> for TenantScope {
//!     fn apply(&self, builder: QueryBuilder<User>) -> QueryBuilder<User> {
//!         builder.where_eq("tenant_id", self.tenant_id)
//!     }
//! }
//!
//! register::<User>(TenantScope { tenant_id: 1 });
//! ```

#![allow(dead_code, clippy::type_complexity)]

use std::{
    any::TypeId,
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::core::model::Model;
use crate::core::query::QueryBuilder;

// ── GlobalScope trait ─────────────────────────────────────────────────────────

/// A scope that is automatically applied to every `QueryBuilder<T>` for a given model.
pub trait GlobalScope<T: Model>: Send + Sync + 'static {
    /// Modify the builder, adding the scope's conditions.
    fn apply(&self, builder: QueryBuilder<T>) -> QueryBuilder<T>;
}

// ── Internal registry ─────────────────────────────────────────────────────────

/// Per-model scope store — not type-erased.
static TYPED_REGISTRY: std::sync::OnceLock<
    RwLock<HashMap<TypeId, Vec<Box<dyn std::any::Any + Send + Sync>>>>,
> = std::sync::OnceLock::new();

fn typed_registry() -> &'static RwLock<HashMap<TypeId, Vec<Box<dyn std::any::Any + Send + Sync>>>> {
    TYPED_REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

struct ScopeWrapper<T: Model>(Arc<dyn GlobalScope<T>>);

// SAFETY: ScopeWrapper wraps Arc<dyn GlobalScope<T>> which requires Send + Sync.
unsafe impl<T: Model + Send> Send for ScopeWrapper<T> {}
unsafe impl<T: Model + Sync> Sync for ScopeWrapper<T> {}

// ── Public API ────────────────────────────────────────────────────────────────

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
/// Called internally by `ModelQuery` when global scopes are active for `T`.
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
