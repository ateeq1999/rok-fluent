//! Opt-in, per-query result cache — a process-wide, TTL-based, table-keyed
//! registry for read-heavy endpoints (e.g. `GET /posts` at ~100 req/s) that
//! repeat the same query.
//!
//! Nothing is cached implicitly. Callers opt in per query by calling a
//! `_cached` terminal (see [`SelectBuilder::fetch_all_cached`] /
//! [`SelectBuilder::fetch_optional_cached`]) and supplying a TTL — default
//! behavior for every other terminal is unchanged. A write through any DSL
//! builder ([`InsertBuilder::execute`], [`UpdateBuilder::execute`],
//! [`DeleteBuilder::execute`]) or the Active Record executor
//! (`insert`/`update`/`delete`) automatically calls [`invalidate_table`] for
//! the affected table; writes made outside those paths can call
//! [`invalidate_table`] or [`clear`] manually.
//!
//! Mirrors the `NAMED_POOLS: Lazy<DashMap<String, PgPool>>` global-registry
//! pattern in [`orm::postgres::pool`](crate::orm::postgres::pool): a single
//! process-wide [`DashMap`], sync reads/writes with no guard ever held across
//! an `.await`.
//!
//! ```rust
//! # use rok_fluent::orm::cache;
//! # use std::time::Duration;
//! # use std::sync::Arc;
//! // In practice the key comes from `make_key` inside a `_cached` terminal;
//! // shown inline here since `make_key` itself is crate-private.
//! let key = "posts:SELECT * FROM posts|".to_string();
//! cache::put(key.clone(), Arc::new(vec![1_i64, 2, 3]), Duration::from_secs(30));
//! let cached: Option<Arc<Vec<i64>>> = cache::get::<Vec<i64>>(&key);
//! assert_eq!(cached.as_deref(), Some(&vec![1_i64, 2, 3]));
//!
//! cache::invalidate_table("posts");
//! cache::clear();
//! ```
//!
//! [`SelectBuilder::fetch_all_cached`]: crate::dsl::SelectBuilder::fetch_all_cached
//! [`SelectBuilder::fetch_optional_cached`]: crate::dsl::SelectBuilder::fetch_optional_cached
//! [`InsertBuilder::execute`]: crate::dsl::InsertBuilder::execute
//! [`UpdateBuilder::execute`]: crate::dsl::UpdateBuilder::execute
//! [`DeleteBuilder::execute`]: crate::dsl::DeleteBuilder::execute

use std::any::Any;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use once_cell::sync::Lazy;

use crate::core::condition::SqlValue;

// ── Cache entry ────────────────────────────────────────────────────────────────

/// A single cached value plus the instant it expires.
///
/// Type-erased via `Arc<dyn Any + Send + Sync>` because the registry holds
/// entries for many different `T`s (whatever each caller's query returns);
/// [`get`] downcasts back to the caller's requested `T` on read.
struct CacheEntry {
    value: Arc<dyn Any + Send + Sync>,
    expires_at: Instant,
}

/// Process-wide query result cache, keyed by [`make_key`].
static QUERY_CACHE: Lazy<DashMap<String, CacheEntry>> = Lazy::new(DashMap::new);

// ── Public API ────────────────────────────────────────────────────────────────

/// Look up a cached value by `key`.
///
/// Returns `None` on a cache miss, an expired entry (which is evicted as a
/// side effect — lazy eviction on read, no background sweep task), or a
/// `T` mismatch against whatever was stored under `key`. When the `metrics`
/// feature is enabled, increments `rok_fluent_cache_hit_total` or
/// `rok_fluent_cache_miss_total` accordingly.
#[must_use]
pub fn get<T: Send + Sync + 'static>(key: &str) -> Option<Arc<T>> {
    let Some(entry) = QUERY_CACHE.get(key) else {
        record_miss();
        return None;
    };
    if entry.expires_at <= Instant::now() {
        drop(entry);
        QUERY_CACHE.remove(key);
        record_miss();
        return None;
    }
    let value = entry.value.clone();
    drop(entry);
    match value.downcast::<T>() {
        Ok(v) => {
            record_hit();
            Some(v)
        }
        Err(_) => {
            record_miss();
            None
        }
    }
}

/// Store `value` under `key`, expiring after `ttl`.
///
/// A later `put` for the same `key` replaces the previous entry (and its TTL).
pub fn put<T: Send + Sync + 'static>(key: String, value: Arc<T>, ttl: Duration) {
    QUERY_CACHE.insert(
        key,
        CacheEntry {
            value: value as Arc<dyn Any + Send + Sync>,
            expires_at: Instant::now() + ttl,
        },
    );
}

/// Evict every cached entry keyed under `table` (i.e. every key starting
/// with `"{table}:"`, per [`make_key`]'s format).
///
/// Called automatically after a successful write through the DSL builders
/// and the Active Record executor; also usable directly as a manual escape
/// hatch for writes made outside those paths (e.g. raw SQL).
pub fn invalidate_table(table: &str) {
    let prefix = format!("{table}:");
    QUERY_CACHE.retain(|k, _| !k.starts_with(&prefix));
}

/// Evict every cached entry, regardless of table.
pub fn clear() {
    QUERY_CACHE.clear();
}

/// Build a cache key from the table name, the rendered SQL, and the bound
/// parameters — `SqlValue` and `SelectBuilder` have no `Hash`/`Eq` impl, so
/// the key is derived from the *rendered* query output rather than the
/// builder itself.
///
/// Format: `"{table}:{sql}|{params joined by SqlValue::to_sql_literal()}"`.
/// The `"{table}:"` prefix is what [`invalidate_table`] matches against.
// The only caller today is `SelectBuilder::fetch_all_cached`/`fetch_optional_cached`
// in src/dsl/select.rs, gated on `postgres` + `query` in addition to `cache` — silence
// dead_code for the (currently caller-less but otherwise valid) `cache`-alone build.
#[cfg_attr(not(all(feature = "postgres", feature = "query")), allow(dead_code))]
pub(crate) fn make_key(table: &str, sql: &str, params: &[SqlValue]) -> String {
    let params_str = params
        .iter()
        .map(SqlValue::to_sql_literal)
        .collect::<Vec<_>>()
        .join(",");
    format!("{table}:{sql}|{params_str}")
}

#[cfg(feature = "metrics")]
fn record_hit() {
    metrics::counter!("rok_fluent_cache_hit_total").increment(1);
}

#[cfg(not(feature = "metrics"))]
fn record_hit() {}

#[cfg(feature = "metrics")]
fn record_miss() {
    metrics::counter!("rok_fluent_cache_miss_total").increment(1);
}

#[cfg(not(feature = "metrics"))]
fn record_miss() {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // `QUERY_CACHE` is a single process-wide static, and `clear()`/`invalidate_table`
    // are global side effects — serialize these tests so they can't observe each
    // other's writes when `cargo test` runs them concurrently on the same binary.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn put_then_get_hits() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let key = make_key("t1", "SELECT * FROM t1", &[]);
        put(
            key.clone(),
            Arc::new(vec![1_i64, 2, 3]),
            Duration::from_secs(60),
        );
        let hit = get::<Vec<i64>>(&key);
        assert_eq!(hit.as_deref(), Some(&vec![1_i64, 2, 3]));
        invalidate_table("t1");
    }

    #[test]
    fn expired_entry_is_a_miss_and_is_evicted() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let key = make_key("t2", "SELECT * FROM t2", &[]);
        put(key.clone(), Arc::new(42_i64), Duration::from_millis(0));
        std::thread::sleep(Duration::from_millis(5));
        assert!(get::<i64>(&key).is_none());
        assert!(!QUERY_CACHE.contains_key(&key));
    }

    #[test]
    fn invalidate_table_only_evicts_matching_prefix() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let k1 = make_key("orders", "SELECT * FROM orders", &[]);
        let k2 = make_key("orders_archive", "SELECT * FROM orders_archive", &[]);
        put(k1.clone(), Arc::new(1_i64), Duration::from_secs(60));
        put(k2.clone(), Arc::new(2_i64), Duration::from_secs(60));
        invalidate_table("orders");
        assert!(get::<i64>(&k1).is_none());
        assert!(get::<i64>(&k2).is_some());
        invalidate_table("orders_archive");
    }

    #[test]
    fn make_key_differs_by_params() {
        let k1 = make_key(
            "users",
            "SELECT * FROM users WHERE id = $1",
            &[SqlValue::Integer(1)],
        );
        let k2 = make_key(
            "users",
            "SELECT * FROM users WHERE id = $1",
            &[SqlValue::Integer(2)],
        );
        assert_ne!(k1, k2);
    }

    #[test]
    fn clear_evicts_everything() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let key = make_key("zzz", "SELECT * FROM zzz", &[]);
        put(key.clone(), Arc::new(1_i64), Duration::from_secs(60));
        clear();
        assert!(get::<i64>(&key).is_none());
    }
}
