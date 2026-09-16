//! Model lifecycle hooks — inline callbacks on ORM write operations.
//!
//! # Example
//!
//! ```rust,no_run
//! # use rok_fluent::orm::hooks::{Hooks, OrmResult};
//! # struct User { pub id: i64, pub email: String }
//! impl Hooks for User {
//!     fn before_save(&mut self) -> OrmResult<()> {
//!         self.email = self.email.to_lowercase();
//!         Ok(())
//!     }
//! }
//! ```

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

/// Lets `#[derive(validator::Validate)]` models call `self.validate().map_err(OrmError::from)?`
/// directly inside a [`Hooks::before_save`]/[`Hooks::before_create`] body, so the `validator`
/// crate's own field attributes (`#[validate(email)]`, `#[validate(length(...))]`, ...) drive
/// validation without rok-fluent parsing them itself.
#[cfg(feature = "validate")]
impl From<validator::ValidationErrors> for OrmError {
    fn from(errors: validator::ValidationErrors) -> Self {
        Self(errors.to_string())
    }
}

// ── Hooks trait ────────────────────────────────────────────────────────────────

/// Inline lifecycle hooks on the model.  All methods are sync and no-op by default.
///
/// Implement explicitly per model — e.g. `impl Hooks for User {}` — rather than
/// relying on a blanket implementation, so overrides stay unambiguous.
pub trait Hooks: Sized {
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
