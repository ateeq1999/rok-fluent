//! N+1 query detector for development mode.
//!
//! Tracks per-request query counts per table and emits a warning when the
//! same table is queried many times in a single request scope, which is the
//! hallmark of an N+1 query.
//!
//! # Usage
//!
//! Enable detection at startup:
//!
//! ```rust,ignore
//! // In main.rs / bootstrap:
//! rok_orm::n1::enable(10); // warn when a table is queried > 10 times per request
//! ```
//!
//! Or via environment variable:
//!
//! ```bash
//! N1_THRESHOLD=5  # warn when threshold exceeded
//! ```
//!
//! Counters reset automatically at the start of each request when using
//! [`OrmLayer`](crate::orm_layer::OrmLayer).

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// ── Global config ─────────────────────────────────────────────────────────────

static ENABLED: AtomicBool = AtomicBool::new(false);
static THRESHOLD: AtomicUsize = AtomicUsize::new(10);

/// Enable N+1 detection, warning when a table is queried more than
/// `threshold` times within a single request.
pub fn enable(threshold: usize) {
    THRESHOLD.store(threshold.max(1), Ordering::Relaxed);
    ENABLED.store(true, Ordering::Relaxed);
}

/// Disable N+1 detection.
pub fn disable() {
    ENABLED.store(false, Ordering::Relaxed);
}

/// Return `true` if N+1 detection is currently enabled.
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

#[cfg(feature = "postgres")]
fn threshold() -> usize {
    THRESHOLD.load(Ordering::Relaxed)
}

// ── Per-request counter ───────────────────────────────────────────────────────

thread_local! {
    static COUNTERS: RefCell<HashMap<String, usize>> = RefCell::new(HashMap::new());
}

/// Reset the per-request query counters for the current task.
///
/// Called by [`OrmLayer`](crate::orm_layer::OrmLayer) at the start of each request.
pub fn reset() {
    COUNTERS.with(|c| c.borrow_mut().clear());
}

/// Record a query against `table` and emit a warning if the threshold is exceeded.
///
/// Call this from every `get()` / `first()` / `count()` terminal.
#[cfg(feature = "postgres")]
pub(crate) fn record(table: &str, result_count: usize) {
    if !is_enabled() {
        return;
    }
    let count = COUNTERS.with(|c| {
        let mut map = c.borrow_mut();
        let entry = map.entry(table.to_string()).or_insert(0);
        *entry += 1;
        *entry
    });

    if count == threshold() + 1 {
        eprintln!(
            "[rok-orm] \u{26a0}  N+1 query detected\n   \
             Table `{table}` has been queried {count} times in this request scope.\n   \
             If loading a relation, add .with(\"{table}\") for batch loading.\n   \
             Last result: {result_count} row(s)."
        );
    }
}

// ── auto-detect from APP_ENV ──────────────────────────────────────────────────

/// Enable detection from the `N1_THRESHOLD` env var, or default to 10 when
/// `APP_ENV` is `development`/`dev`/`local`.
///
/// Called by [`OrmLayer`](crate::orm_layer::OrmLayer) at construction time.
pub fn auto_configure() {
    if let Ok(t) = std::env::var("N1_THRESHOLD") {
        if let Ok(n) = t.parse::<usize>() {
            enable(n);
            return;
        }
    }
    let is_dev = matches!(
        std::env::var("APP_ENV").as_deref(),
        Ok("development" | "dev" | "local")
    );
    if is_dev {
        enable(10);
    }
}
