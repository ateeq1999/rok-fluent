//! Optional query logging middleware.
//!
//! Records execution time and SQL for slow queries.
//! When the `tracing` feature is enabled, a tracing span is emitted automatically.

use crate::core::condition::SqlValue;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// A single logged query event.
#[derive(Debug, Clone)]
pub struct QueryEvent {
    /// The SQL statement that was executed.
    pub sql: String,
    /// Bound parameter values.
    pub params: Vec<SqlValue>,
    /// Execution time in milliseconds.
    pub duration_ms: u64,
    /// Number of rows affected or returned.
    pub rows_affected: u64,
    /// The table name (from the model).
    pub table: String,
}

/// A shared logging callback type.
pub type QueryLogger = Arc<dyn Fn(&QueryEvent) + Send + Sync>;

static SLOW_QUERY_THRESHOLD_MS: AtomicU64 = AtomicU64::new(200);

/// Set the slow-query threshold in milliseconds.
///
/// Queries slower than this will be passed to the registered logger.
pub fn set_slow_threshold(ms: u64) {
    SLOW_QUERY_THRESHOLD_MS.store(ms, Ordering::Relaxed);
}

/// Return the current slow-query threshold in milliseconds.
pub fn slow_threshold() -> u64 {
    SLOW_QUERY_THRESHOLD_MS.load(Ordering::Relaxed)
}

static SLOW_LOGGER: once_cell::sync::Lazy<std::sync::Mutex<Option<QueryLogger>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(None));

static ON_QUERY_LOGGER: once_cell::sync::Lazy<std::sync::Mutex<Option<QueryLogger>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(None));

/// Register a callback that is called for every slow query.
pub fn set_logger(logger: QueryLogger) {
    if let Ok(mut l) = SLOW_LOGGER.lock() {
        *l = Some(logger);
    }
}

/// Register a callback that is called for **every** query (fast and slow).
///
/// Unlike [`set_logger`], this fires unconditionally regardless of the
/// slow-query threshold.
pub fn set_on_query(logger: QueryLogger) {
    if let Ok(mut l) = ON_QUERY_LOGGER.lock() {
        *l = Some(logger);
    }
}

/// Unregister the on-query callback.
pub fn clear_on_query() {
    if let Ok(mut l) = ON_QUERY_LOGGER.lock() {
        *l = None;
    }
}

pub(crate) fn log_query(
    sql: &str,
    params: &[SqlValue],
    duration_ms: u64,
    rows_affected: u64,
    table: &str,
) {
    let event = QueryEvent {
        sql: sql.to_string(),
        params: params.to_vec(),
        duration_ms,
        rows_affected,
        table: table.to_string(),
    };

    // Emit a tracing span when the feature is enabled
    #[cfg(feature = "tracing")]
    {
        tracing::trace!(
            target: "rok_fluent::query",
            sql = %event.sql,
            table = %event.table,
            duration_ms = event.duration_ms,
            rows_affected = event.rows_affected,
            "query executed"
        );
    }

    // Fire the on-query callback for every query.
    if let Ok(l) = ON_QUERY_LOGGER.lock() {
        if let Some(ref cb) = *l {
            cb(&event);
        }
    }

    // Fire the slow-query callback only when the threshold is exceeded.
    if duration_ms >= slow_threshold() {
        if let Ok(l) = SLOW_LOGGER.lock() {
            if let Some(ref logger) = *l {
                logger(&event);
            }
        }
    }
}
