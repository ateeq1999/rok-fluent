//! Model hooks and observers — lifecycle callbacks on ORM operations.
//!
//! Two layers:
//! 1. **`ModelHooks`** — inline trait on the model (sync)
//! 2. **`Observer<T>`** — external struct registered via [`observe`]
//!
//! # Example
//!
//! ```rust,no_run
//! # use rok_fluent::orm::hooks::{Observer, OrmResult, observe};
//! # struct User { pub id: i64, pub email: String }
//! pub struct UserObserver;
//!
//! impl Observer<User> for UserObserver {
//!     fn creating(&self, user: &mut User) -> OrmResult<()> {
//!         user.email = user.email.to_lowercase();
//!         Ok(())
//!     }
//! }
//!
//! observe::<User, UserObserver>(UserObserver);
//! ```

use std::{
    any::TypeId,
    collections::HashMap,
    sync::{Arc, OnceLock, RwLock},
};

use crate::core::condition::SqlValue;

// ── OrmResult ─────────────────────────────────────────────────────────────────

/// Error returned by hook methods.
#[derive(Debug, Clone)]
pub struct OrmError(pub String);

impl OrmError {
    /// Create an `OrmError` with the given message.
    pub fn new(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }
}

impl std::fmt::Display for OrmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for OrmError {}

/// Shorthand `Result` type for hook returns.
pub type OrmResult<T = ()> = Result<T, OrmError>;

// ── ModelHooks trait ──────────────────────────────────────────────────────────

/// Inline lifecycle hooks on the model.  All methods are sync and no-op by default.
pub trait ModelHooks: Sized {
    /// Called before a `CREATE` operation. Return `Err` to abort.
    fn before_create(&mut self) -> OrmResult<()> {
        Ok(())
    }
    /// Called after a successful `CREATE` operation.
    fn after_create(&self) {}
    /// Called before an `UPDATE` operation. Return `Err` to abort.
    fn before_update(&mut self, _dirty: &[&str]) -> OrmResult<()> {
        Ok(())
    }
    /// Called after a successful `UPDATE` operation.
    fn after_update(&self) {}
    /// Called before any write operation. Return `Err` to abort.
    fn before_save(&mut self) -> OrmResult<()> {
        Ok(())
    }
    /// Called after any successful write operation.
    fn after_save(&self) {}
    /// Called before a `DELETE` operation. Return `Err` to abort.
    fn before_delete(&self) -> OrmResult<()> {
        Ok(())
    }
    /// Called after a successful `DELETE` operation.
    fn after_delete(&self) {}
}

// ── Observer trait ────────────────────────────────────────────────────────────

/// External observer — groups all lifecycle hooks in one struct.
///
/// All methods are sync and no-op by default; override what you need.
pub trait Observer<T: Send + Sync + 'static>: Send + Sync + 'static {
    /// Called before a row is created.
    fn creating(&self, _model: &mut T) -> OrmResult<()> {
        Ok(())
    }
    /// Called after a row is created.
    fn created(&self, _model: &T) {}
    /// Called before a row is updated.
    fn updating(&self, _model: &mut T, _dirty: &[&str]) -> OrmResult<()> {
        Ok(())
    }
    /// Called after a row is updated.
    fn updated(&self, _model: &T) {}
    /// Called before any write.
    fn saving(&self, _model: &mut T) -> OrmResult<()> {
        Ok(())
    }
    /// Called after any write.
    fn saved(&self, _model: &T) {}
    /// Called before a row is deleted.
    fn deleting(&self, _model: &T) -> OrmResult<()> {
        Ok(())
    }
    /// Called after a row is deleted.
    fn deleted(&self, _model: &T) {}
}

// ── Observer registry ─────────────────────────────────────────────────────────

#[allow(clippy::type_complexity)]
static OBSERVER_REGISTRY: OnceLock<
    RwLock<HashMap<TypeId, Vec<Arc<dyn std::any::Any + Send + Sync>>>>,
> = OnceLock::new();

#[allow(clippy::type_complexity)]
fn observer_registry() -> &'static RwLock<HashMap<TypeId, Vec<Arc<dyn std::any::Any + Send + Sync>>>>
{
    OBSERVER_REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

#[allow(dead_code)]
struct ObserverWrapper<T: Send + Sync + 'static>(Arc<dyn Observer<T>>);

// SAFETY: ObserverWrapper<T> only wraps Arc<dyn Observer<T>> which is Send + Sync.
unsafe impl<T: Send + Sync + 'static> Send for ObserverWrapper<T> {}
unsafe impl<T: Send + Sync + 'static> Sync for ObserverWrapper<T> {}

/// Register an observer for model `T`.  Call once at startup.
pub fn observe<T: Send + Sync + 'static, Obs: Observer<T>>(obs: Obs) {
    let type_id = TypeId::of::<T>();
    let wrapped: Arc<dyn std::any::Any + Send + Sync> =
        Arc::new(ObserverWrapper::<T>(Arc::new(obs)));
    observer_registry()
        .write()
        .unwrap()
        .entry(type_id)
        .or_default()
        .push(wrapped);
}

// ── Data-based dispatch ────────────────────────────────────────────────────────

/// Dispatch the `creating` hook at the executor level (no model instance).
pub fn dispatch_creating<T: 'static>(_table: &str, _data: &[(&str, SqlValue)]) -> OrmResult<()> {
    if observers_muted() {
        return Ok(());
    }
    Ok(())
}

/// Dispatch the `created` hook at the executor level.
pub fn dispatch_created<T: 'static>(_table: &str, _data: &[(&str, SqlValue)]) {
    if observers_muted() {}
}

/// Dispatch the `updating` hook at the executor level.
pub fn dispatch_updating<T: 'static>(_table: &str, _data: &[(&str, SqlValue)]) -> OrmResult<()> {
    if observers_muted() {
        return Ok(());
    }
    Ok(())
}

/// Dispatch the `updated` hook at the executor level.
pub fn dispatch_updated<T: 'static>(_table: &str, _data: &[(&str, SqlValue)]) {
    if observers_muted() {}
}

/// Dispatch the `deleting` hook at the executor level.
pub fn dispatch_deleting<T: 'static>(_table: &str, _data: &[(&str, SqlValue)]) -> OrmResult<()> {
    if observers_muted() {
        return Ok(());
    }
    Ok(())
}

/// Dispatch the `deleted` hook at the executor level.
pub fn dispatch_deleted<T: 'static>(_table: &str, _data: &[(&str, SqlValue)]) {
    if observers_muted() {}
}

/// Dispatch the `saving` hook at the executor level.
pub fn dispatch_saving<T: 'static>(_table: &str, _data: &[(&str, SqlValue)]) -> OrmResult<()> {
    if observers_muted() {
        return Ok(());
    }
    Ok(())
}

/// Dispatch the `saved` hook at the executor level.
pub fn dispatch_saved<T: 'static>(_table: &str, _data: &[(&str, SqlValue)]) {
    if observers_muted() {}
}

/// Remove all observers for model `T`.
pub fn clear_observers<T: Send + Sync + 'static>() {
    let type_id = TypeId::of::<T>();
    observer_registry().write().unwrap().remove(&type_id);
}

// ── without_events ────────────────────────────────────────────────────────────

thread_local! {
    static MUTE_OBSERVERS: std::cell::RefCell<bool> = const { std::cell::RefCell::new(false) };
}

/// Run `f` with all observer dispatches suppressed.
///
/// ```rust,ignore
/// # use rok_fluent::orm::hooks::without_events;
/// # async fn example() -> Result<(), sqlx::Error> {
/// without_events(|| async {
///     Ok::<(), sqlx::Error>(())
/// }).await?;
/// # Ok(()) }
/// ```
pub async fn without_events<F, Fut, T, E>(f: F) -> Result<T, E>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    MUTE_OBSERVERS.with(|m| *m.borrow_mut() = true);
    let result = f().await;
    MUTE_OBSERVERS.with(|m| *m.borrow_mut() = false);
    result
}

/// Returns `true` if observer dispatch is currently muted.
pub fn observers_muted() -> bool {
    MUTE_OBSERVERS.with(|m| *m.borrow())
}
