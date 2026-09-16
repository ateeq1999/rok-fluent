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
//! Concurrent callers for the same cold key don't each run their own query:
//! `SelectBuilder::fetch_all_cached`/`fetch_optional_cached` (feature
//! `postgres`) go through an internal `get_or_populate` helper that
//! single-flights the underlying query per key via a second process-wide
//! `DashMap` of `tokio::sync::OnceCell`s, so N concurrent misses for the same
//! key produce 1 query, not N. See that function's doc comment for the exact
//! mechanism.
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

// ── Single-flight coalescing ─────────────────────────────────────────────────
//
// Confined to `postgres`: the only callers of `get_or_populate` today are
// `SelectBuilder::fetch_all_cached`/`fetch_optional_cached` in
// `src/dsl/select.rs`, which are themselves postgres-only, and this code
// needs `tokio::sync::OnceCell` plus `sqlx::Error` — both only present in
// the dependency graph via `postgres`/`sqlite`/`mysql`, never via `cache`
// alone. `cache`-alone (no database backend) must keep compiling without
// `tokio`/`sqlx` present at all, so everything below is gated accordingly.

#[cfg(feature = "postgres")]
type InFlightResult = Result<Arc<dyn Any + Send + Sync>, String>;

/// Per-key single-flight state shared by every concurrent caller of
/// [`get_or_populate`] for that key: the `OnceCell` they all await, plus a
/// count of callers currently waiting on it.
///
/// `waiters` exists only to detect "every caller for this key was
/// abandoned" (every waiting future dropped/cancelled — e.g. a request
/// timeout — before the attempt resolved): see [`WaiterGuard`]. It plays no
/// role in the normal path; the resolved, cloned [`InFlightResult`] is what
/// every waiter actually consumes.
#[cfg(feature = "postgres")]
struct InFlight {
    cell: tokio::sync::OnceCell<InFlightResult>,
    waiters: std::sync::atomic::AtomicUsize,
}

/// Process-wide single-flight registry, keyed the same as [`QUERY_CACHE`].
///
/// Unlike `QUERY_CACHE`, an entry here is never long-lived: it exists only
/// while a query for that key is actually in flight, and is removed the
/// moment that attempt resolves (successfully, with an error, or because
/// every waiter abandoned it) — see [`get_or_populate`] and [`WaiterGuard`].
/// It therefore needs no TTL or invalidation wiring of its own.
#[cfg(feature = "postgres")]
static IN_FLIGHT: Lazy<DashMap<String, Arc<InFlight>>> = Lazy::new(DashMap::new);

/// RAII waiter-count decrement for a [`get_or_populate`] call.
///
/// On drop (including via cancellation — e.g. the caller's future is
/// dropped by a `tokio::time::timeout`), decrements `state`'s waiter count;
/// if that was the *last* waiter for `state` and the attempt never resolved
/// (nobody is left to drive `loader` to completion), removes the now-orphaned
/// entry from [`IN_FLIGHT`] so it doesn't linger forever and the next caller
/// for `key` starts a fresh attempt instead of waiting on one nothing will
/// ever finish.
#[cfg(feature = "postgres")]
struct WaiterGuard<'a> {
    key: &'a str,
    state: &'a Arc<InFlight>,
}

#[cfg(feature = "postgres")]
impl Drop for WaiterGuard<'_> {
    fn drop(&mut self) {
        use std::sync::atomic::Ordering;

        let was_last = self.state.waiters.fetch_sub(1, Ordering::AcqRel) == 1;
        if was_last && !self.state.cell.initialized() {
            IN_FLIGHT.remove_if(self.key, |_, v| Arc::ptr_eq(v, self.state));
        }
    }
}

/// Runs `loader` to populate `key`, coalescing concurrent callers for the
/// same cold key into a single execution of `loader` — the fix for the
/// "thundering herd" a cold or just-expired cache key otherwise produces
/// under concurrent load (see `docs/guides/caching.md`).
///
/// Behavior:
/// - A warm key never touches the in-flight machinery at all: checked via
///   plain [`get`] up front, same as before this existed.
/// - For a cold key, every concurrent caller joins the *same*
///   `tokio::sync::OnceCell` (found or created via [`IN_FLIGHT`]) and awaits
///   it; only the one caller `tokio` elects to actually run the future wins
///   the race to call `loader`. Every other caller waits on that single
///   attempt's result instead of starting its own.
/// - `Ok`: the winner stores the value in [`QUERY_CACHE`] via [`put`] and
///   every waiter gets an `Arc` clone of it.
/// - `Err`: every waiter gets an error too — `sqlx::Error` isn't `Clone`, so
///   each waiter gets its own `sqlx::Error::Protocol` carrying the original
///   error's `Display` text rather than a bit-for-bit clone of the original
///   value. The key is *not* poisoned: the in-flight entry is removed right
///   after the attempt resolves (`Ok` or `Err`), so the next caller retries
///   `loader` from scratch rather than replaying the failure.
/// - Cancellation: if every waiter for a key is dropped/cancelled before the
///   attempt resolves (including the one actually running `loader`, in which
///   case `tokio::sync::OnceCell` itself lets another waiter take over — see
///   its docs), [`WaiterGuard`] removes the orphaned entry so it doesn't leak.
#[cfg(feature = "postgres")]
pub(crate) async fn get_or_populate<T, F, Fut>(
    key: &str,
    ttl: Duration,
    loader: F,
) -> Result<Arc<T>, sqlx::Error>
where
    T: Send + Sync + 'static,
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, sqlx::Error>> + Send,
{
    if let Some(hit) = get::<T>(key) {
        return Ok(hit);
    }

    // `entry`'s shard guard lives only for this statement, well before the
    // `.await` below — never held across an await point, per this module's
    // own convention (see the module doc).
    let state = IN_FLIGHT
        .entry(key.to_string())
        .or_insert_with(|| {
            Arc::new(InFlight {
                cell: tokio::sync::OnceCell::new(),
                waiters: std::sync::atomic::AtomicUsize::new(0),
            })
        })
        .clone();
    state
        .waiters
        .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    let _waiter_guard = WaiterGuard { key, state: &state };

    let owned_key = key.to_string();
    let result = state
        .cell
        .get_or_init(move || async move {
            match loader().await {
                Ok(value) => {
                    let arc = Arc::new(value);
                    put(owned_key, arc.clone(), ttl);
                    Ok(arc as Arc<dyn Any + Send + Sync>)
                }
                Err(e) => Err(e.to_string()),
            }
        })
        .await
        .clone();

    // The attempt has resolved (no further `.await` since, so this can't
    // race with a cancellation of *this* call) — evict it so the next cold
    // miss for `key` (TTL expiry, or a retry after this `Err`) starts fresh
    // rather than replaying this resolved result forever. Guarded by pointer
    // identity so a since-replaced entry for the same key is left alone.
    IN_FLIGHT.remove_if(key, |_, v| Arc::ptr_eq(v, &state));

    match result {
        Ok(value) => value.downcast::<T>().map_err(|_| {
            sqlx::Error::Protocol("rok_fluent: cache in-flight type mismatch".to_string())
        }),
        Err(message) => Err(sqlx::Error::Protocol(message)),
    }
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

    // `get_or_populate` only exists under `postgres` (see its doc comment for
    // why). These are `#[tokio::test]`s rather than plain `#[test]`s, so
    // unlike the sync tests above they deliberately do *not* serialize via
    // `TEST_LOCK` — holding a blocking `std::sync::Mutex` guard across an
    // `.await` would violate this module's own no-guard-across-await
    // convention. Each test instead uses a cache key unique to itself, so
    // concurrent test-binary execution can't cross-contaminate `QUERY_CACHE`/
    // `IN_FLIGHT` (both process-wide statics shared by every test in this
    // binary) — none of these tests call `clear()`/`invalidate_table`, which
    // would affect unrelated concurrently-running tests too.
    #[cfg(feature = "postgres")]
    mod single_flight {
        use super::*;
        use std::sync::atomic::{AtomicU64, Ordering};

        #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
        async fn concurrent_cold_miss_coalesces_to_one_query() {
            let key = "single_flight:cold_miss";
            let calls = Arc::new(AtomicU64::new(0));

            let mut handles = Vec::with_capacity(20);
            for _ in 0..20 {
                let calls = calls.clone();
                handles.push(tokio::spawn(async move {
                    get_or_populate::<i64, _, _>(key, Duration::from_secs(30), move || async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        Ok(7_i64)
                    })
                    .await
                }));
            }

            for h in handles {
                let value = h
                    .await
                    .expect("task panicked")
                    .expect("loader must not fail");
                assert_eq!(*value, 7);
            }

            assert_eq!(
                calls.load(Ordering::SeqCst),
                1,
                "20 concurrent cold-miss callers for the same key must coalesce into 1 loader call"
            );
            assert!(
                get::<i64>(key).is_some(),
                "the winning attempt must populate the cache"
            );
            assert!(
                IN_FLIGHT.get(key).is_none(),
                "the in-flight marker must be cleaned up once the attempt resolves"
            );
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
        async fn concurrent_error_is_shared_and_key_is_not_poisoned() {
            let key = "single_flight:error_then_retry";
            let attempts = Arc::new(AtomicU64::new(0));

            let mut handles = Vec::with_capacity(20);
            for _ in 0..20 {
                let attempts = attempts.clone();
                handles.push(tokio::spawn(async move {
                    get_or_populate::<i64, _, _>(key, Duration::from_secs(30), move || async move {
                        attempts.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        Err(sqlx::Error::RowNotFound)
                    })
                    .await
                }));
            }

            for h in handles {
                let err = h
                    .await
                    .expect("task panicked")
                    .expect_err("loader must fail for every waiter");
                match err {
                    sqlx::Error::Protocol(_) => {}
                    other => panic!("unexpected error variant: {other:?}"),
                }
            }

            assert_eq!(
                attempts.load(Ordering::SeqCst),
                1,
                "20 concurrent callers racing a failing loader must still only invoke it once"
            );
            assert!(
                get::<i64>(key).is_none(),
                "a failed attempt must not leave a cached value behind"
            );

            // Not poisoned: a later call for the same key retries `loader`
            // rather than staying permanently broken.
            let value = get_or_populate::<i64, _, _>(key, Duration::from_secs(30), {
                let attempts = attempts.clone();
                move || async move {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    Ok(9_i64)
                }
            })
            .await
            .expect("retry after a failed attempt must succeed");
            assert_eq!(*value, 9);
            assert_eq!(
                attempts.load(Ordering::SeqCst),
                2,
                "the retry must reach the loader again, not reuse the stale failure"
            );
        }

        #[tokio::test]
        async fn warm_key_never_touches_the_in_flight_path() {
            let key = "single_flight:already_warm";
            put(key.to_string(), Arc::new(41_i64), Duration::from_secs(30));

            let value = get_or_populate::<i64, _, _>(key, Duration::from_secs(30), || async {
                panic!("loader must not run for an already-warm key")
            })
            .await
            .expect("a warm key must short-circuit to the cached value");
            assert_eq!(*value, 41);
        }
    }
}
