//! Task-local database pool for pool-free ORM queries.
//!
//! [`OrmLayer`](crate::orm::orm_layer::OrmLayer) sets the pool automatically for
//! every Axum request. Outside of a request context — e.g. in background tasks
//! or tests — call `with_pool` explicitly.
//!
//! ```rust,no_run
//! # use rok_fluent::orm::postgres::pool;
//! # async fn example(pool: sqlx::PgPool) {
//! pool::with_pool(pool.clone(), async {
//!     // pool-free ORM queries work here
//! }).await;
//! # }
//! ```

use dashmap::DashMap;
use once_cell::sync::Lazy;
use sqlx::PgPool;

use super::executor;
use crate::core::model::Model;
use crate::core::query::QueryBuilder;

tokio::task_local! {
    static CURRENT_POOL: PgPool;
    static REPLICA_POOL: PgPool;
}

// ── Named pool registry ───────────────────────────────────────────────────────

static NAMED_POOLS: Lazy<DashMap<String, PgPool>> = Lazy::new(DashMap::new);

/// Register a named database pool for multi-database routing.
pub fn register_named_pool(name: impl Into<String>, pool: PgPool) {
    NAMED_POOLS.insert(name.into(), pool);
}

/// Retrieve a previously registered named pool by name.
///
/// Returns `None` if no pool with that name has been registered.
pub fn get_named_pool(name: &str) -> Option<PgPool> {
    NAMED_POOLS.get(name).map(|p| p.clone())
}

/// Remove a named pool from the registry.
pub fn unregister_named_pool(name: &str) {
    NAMED_POOLS.remove(name);
}

/// Run `f` with `pool` available to all pool-free ORM query terminals in scope.
///
/// Use this in tests or background tasks where [`OrmLayer`] is not present.
///
/// [`OrmLayer`]: crate::orm::orm_layer::OrmLayer
pub async fn with_pool<F>(pool: PgPool, f: F) -> F::Output
where
    F: std::future::Future,
{
    CURRENT_POOL.scope(pool, f).await
}

/// Run `f` with both primary and replica pools in scope.
pub async fn with_pools<F>(primary: PgPool, replica: PgPool, f: F) -> F::Output
where
    F: std::future::Future,
{
    REPLICA_POOL
        .scope(replica, async { CURRENT_POOL.scope(primary, f).await })
        .await
}

/// Wrap a future to run with `pool` in scope (sync, returns a `Future`).
///
/// Used internally by [`OrmLayer`].
///
/// [`OrmLayer`]: crate::orm::orm_layer::OrmLayer
#[cfg(feature = "axum")]
pub(crate) fn scope_pool<F: std::future::Future>(
    pool: PgPool,
    f: F,
) -> impl std::future::Future<Output = F::Output> {
    CURRENT_POOL.scope(pool, f)
}

/// Retrieve the pool from the current task-local scope.
///
/// Returns `None` when called outside of [`with_pool`] / [`OrmLayer`].
///
/// [`OrmLayer`]: crate::orm::orm_layer::OrmLayer
pub fn try_current_pool() -> Option<PgPool> {
    CURRENT_POOL.try_with(|p| p.clone()).ok()
}

/// Retrieve the replica pool from the current task-local scope.
///
/// Returns `None` when no replica pool is configured or when called outside
/// of [`with_pools`].
pub fn try_replica_pool() -> Option<PgPool> {
    REPLICA_POOL.try_with(|p| p.clone()).ok()
}

/// Resolve the appropriate pool for a read query based on `builder.use_replica`.
pub fn resolve_read_pool<T>(builder: &QueryBuilder<T>) -> Result<PgPool, sqlx::Error> {
    if builder.use_replica {
        if let Some(replica) = try_replica_pool() {
            return Ok(replica);
        }
    }
    try_current_pool().ok_or_else(|| {
        sqlx::Error::Configuration(
            "no database pool in scope — add OrmLayer to your router or \
             call pool::with_pool() in tests"
                .to_string()
                .into(),
        )
    })
}

/// Fetch all rows, routing to replica when `builder.use_replica` is true.
pub async fn fetch_all<T>(builder: QueryBuilder<T>) -> Result<Vec<T>, sqlx::Error>
where
    T: Model + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
{
    let pool = resolve_read_pool(&builder)?;
    executor::fetch_all(&pool, builder).await
}

/// Fetch one optional row, routing to replica when `builder.use_replica` is true.
pub async fn fetch_optional<T>(builder: QueryBuilder<T>) -> Result<Option<T>, sqlx::Error>
where
    T: Model + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
{
    let pool = resolve_read_pool(&builder)?;
    executor::fetch_optional(&pool, builder).await
}

/// Count rows, routing to replica when `builder.use_replica` is true.
pub async fn count<T: Model>(builder: QueryBuilder<T>) -> Result<i64, sqlx::Error> {
    let pool = resolve_read_pool(&builder)?;
    executor::count(&pool, builder).await
}

/// Aggregate query, routing to replica when `builder.use_replica` is true.
pub async fn aggregate<T: Model>(
    builder: QueryBuilder<T>,
    agg_expr: &str,
) -> Result<Option<f64>, sqlx::Error> {
    let pool = resolve_read_pool(&builder)?;
    executor::aggregate(&pool, builder, agg_expr).await
}

// ── Pool health ───────────────────────────────────────────────────────────────

/// Return `true` if the pool can execute a lightweight round-trip query.
///
/// Uses `SELECT 1` — suitable for health-check endpoints and readiness probes.
///
/// ```rust,no_run
/// # async fn example(pool: sqlx::PgPool) -> Result<(), sqlx::Error> {
/// if rok_fluent::orm::postgres::pool::ping(&pool).await {
///     println!("database is reachable");
/// }
/// # Ok(())
/// # }
/// ```
pub async fn ping(pool: &PgPool) -> bool {
    sqlx::query("SELECT 1").execute(pool).await.is_ok()
}

// ── Pool metrics ──────────────────────────────────────────────────────────────

/// A point-in-time snapshot of a connection pool's resource usage.
#[derive(Debug, Clone, Copy)]
pub struct PoolMetrics {
    /// Total open connections (active + idle).
    pub size: u32,
    /// Connections currently idle (not checked out).
    pub idle: u32,
    /// Connections currently checked out by callers.
    pub active: u32,
}

/// Capture current pool metrics for `pool`.
pub fn snapshot(pool: &PgPool) -> PoolMetrics {
    let size = pool.size();
    let idle = pool.num_idle() as u32;
    PoolMetrics {
        size,
        idle,
        active: size.saturating_sub(idle),
    }
}

/// Emit pool metrics via the [`metrics`] facade (requires feature `metrics`).
///
/// Registers three gauges — `db_pool_size`, `db_pool_idle`, `db_pool_active` —
/// each labelled with `pool = label`.
#[cfg(feature = "metrics")]
pub fn emit_metrics(pool: &PgPool, label: &str) {
    let m = snapshot(pool);
    metrics::gauge!("db_pool_size",   "pool" => label.to_string()).set(m.size as f64);
    metrics::gauge!("db_pool_idle",   "pool" => label.to_string()).set(m.idle as f64);
    metrics::gauge!("db_pool_active", "pool" => label.to_string()).set(m.active as f64);
}
