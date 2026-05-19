//! Optional query logging middleware.
//!
//! Records execution time and SQL for slow queries.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct QueryLogEntry {
    pub sql: String,
    pub duration_ms: u64,
    pub rows_affected: u64,
    pub table: String,
}

pub type QueryLogger = Arc<dyn Fn(&QueryLogEntry) + Send + Sync>;

static SLOW_QUERY_THRESHOLD_MS: AtomicU64 = AtomicU64::new(200);

pub fn set_slow_threshold(ms: u64) {
    SLOW_QUERY_THRESHOLD_MS.store(ms, Ordering::Relaxed);
}

pub fn slow_threshold() -> u64 {
    SLOW_QUERY_THRESHOLD_MS.load(Ordering::Relaxed)
}

static LOGGER: once_cell::sync::Lazy<std::sync::Mutex<Option<QueryLogger>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(None));

pub fn set_logger(logger: QueryLogger) {
    if let Ok(mut l) = LOGGER.lock() {
        *l = Some(logger);
    }
}

pub(crate) fn log_query(sql: &str, duration_ms: u64, rows_affected: u64, table: &str) {
    if duration_ms >= slow_threshold() {
        if let Ok(l) = LOGGER.lock() {
            if let Some(ref logger) = *l {
                logger(&QueryLogEntry {
                    sql: sql.to_string(),
                    duration_ms,
                    rows_affected,
                    table: table.to_string(),
                });
            }
        }
    }
}
