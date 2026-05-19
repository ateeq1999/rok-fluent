//! Model hooks and observers — lifecycle callbacks on ORM operations.
//!
//! Two layers:
//! 1. **`ModelHooks`** — inline trait on the model (sync)
//! 2. **`Observer<T>`** — external struct registered via `rok_orm::observe::<T, Obs>()`
//!
//! # Example
//!
//! ```rust,ignore
//! pub struct UserObserver;
//!
//! impl Observer<User> for UserObserver {
//!     fn creating(&self, user: &mut User) -> OrmResult<()> {
//!         user.email = user.email.to_lowercase();
//!         Ok(())
//!     }
//!     fn created(&self, user: &User) {
//!         println!("User {} created", user.id);
//!     }
//! }
//!
//! rok_orm::observe::<User, UserObserver>(UserObserver);
//! ```

use std::{
    any::TypeId,
    collections::HashMap,
    sync::{Arc, OnceLock, RwLock},
};

use crate::SqlValue;

// ── OrmResult ─────────────────────────────────────────────────────────────────

/// Error returned by hook methods.
#[derive(Debug, Clone)]
pub struct OrmError(pub String);

impl OrmError {
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

pub type OrmResult<T = ()> = Result<T, OrmError>;

// ── ModelHooks trait ──────────────────────────────────────────────────────────

/// Inline lifecycle hooks on the model.  All methods are sync and no-op by default.
pub trait ModelHooks: Sized {
    fn before_create(&mut self) -> OrmResult<()> {
        Ok(())
    }
    fn after_create(&self) {}
    fn before_update(&mut self, _dirty: &[&str]) -> OrmResult<()> {
        Ok(())
    }
    fn after_update(&self) {}
    fn before_save(&mut self) -> OrmResult<()> {
        Ok(())
    }
    fn after_save(&self) {}
    fn before_delete(&self) -> OrmResult<()> {
        Ok(())
    }
    fn after_delete(&self) {}
}

// ── Observer trait ────────────────────────────────────────────────────────────

/// External observer — groups all lifecycle hooks in one struct.
/// All methods are sync and no-op by default; override what you need.
pub trait Observer<T: Send + Sync + 'static>: Send + Sync + 'static {
    fn creating(&self, _model: &mut T) -> OrmResult<()> {
        Ok(())
    }
    fn created(&self, _model: &T) {}
    fn updating(&self, _model: &mut T, _dirty: &[&str]) -> OrmResult<()> {
        Ok(())
    }
    fn updated(&self, _model: &T) {}
    fn saving(&self, _model: &mut T) -> OrmResult<()> {
        Ok(())
    }
    fn saved(&self, _model: &T) {}
    fn deleting(&self, _model: &T) -> OrmResult<()> {
        Ok(())
    }
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

#[allow(unused_macros)]
macro_rules! dispatch {
    ($fn_name:ident, $hook:ident ($($arg:ident: $ty:ty),*) -> $ret:ty, $body:expr) => {
        pub fn $fn_name<T: Send + Sync + 'static>($($arg: $ty),*) -> $ret {
            let type_id = TypeId::of::<T>();
            let reg = observer_registry().read().unwrap();
            if observers_muted() {
                return $body;
            }
            if let Some(entries) = reg.get(&type_id) {
                for entry in entries {
                    if let Some(wrapper) = entry.downcast_ref::<ObserverWrapper<T>>() {
                        $body;
                        let _ = wrapper.0.$hook($($arg),*);
                    }
                }
            }
            $body
        }
    };
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
/// rok_orm::without_events(|| async {
///     User::all_query().update_all(&[("login_count", 0.into())]).await?;
///     Ok(())
/// }).await?;
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
