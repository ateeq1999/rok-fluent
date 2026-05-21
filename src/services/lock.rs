//! [`LockService`] — advisory and row-level locking utilities.

use std::time::Duration;

/// PostgreSQL advisory-lock lifecycle and row-level locking helpers.
///
/// Advisory locks are application-level mutexes stored in the PostgreSQL
/// server's memory. They are not tied to any specific table row.
///
/// # Example (advisory lock)
///
/// ```rust,ignore
/// use rok_fluent::services::LockService;
///
/// // Acquire an exclusive session-level advisory lock
/// LockService::acquire(42, &pool).await?;
/// // … critical section …
/// LockService::release(42, &pool).await?;
/// ```
///
/// # Example (row-level lock)
///
/// ```rust,ignore
/// use rok_fluent::dsl::{Lock, LockConflict};
///
/// db::select()
///     .from(User::table())
///     .where_(User::ID.eq(42_i64))
///     .lock(Lock::ForUpdate)
///     .lock_conflict(LockConflict::NoWait)
///     .fetch_optional::<User>(&pool).await?;
/// ```
pub struct LockService;

impl LockService {
    /// Acquire an exclusive session-level advisory lock.
    ///
    /// Blocks until the lock becomes available.
    ///
    /// ```sql
    /// SELECT pg_advisory_lock(42)
    /// ```
    pub async fn acquire(key: i64, pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
        sqlx::query("SELECT pg_advisory_lock($1)")
            .bind(key)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Try to acquire an advisory lock without blocking.
    ///
    /// Returns `true` on success, `false` if another session holds the lock.
    ///
    /// ```sql
    /// SELECT pg_try_advisory_lock(42)
    /// ```
    pub async fn try_acquire(key: i64, pool: &sqlx::PgPool) -> Result<bool, sqlx::Error> {
        let row: (bool,) = sqlx::query_as("SELECT pg_try_advisory_lock($1)")
            .bind(key)
            .fetch_one(pool)
            .await?;
        Ok(row.0)
    }

    /// Release a previously acquired session-level advisory lock.
    ///
    /// ```sql
    /// SELECT pg_advisory_unlock(42)
    /// ```
    pub async fn release(key: i64, pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
        sqlx::query("SELECT pg_advisory_unlock($1)")
            .bind(key)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Acquire a session-level advisory lock with a timeout.
    ///
    /// Polls [`try_acquire`](LockService::try_acquire) every 100 ms until the
    /// lock is acquired or the timeout expires.
    pub async fn acquire_timeout(
        key: i64,
        timeout: Duration,
        pool: &sqlx::PgPool,
    ) -> Result<bool, sqlx::Error> {
        let start = std::time::Instant::now();
        loop {
            if LockService::try_acquire(key, pool).await? {
                return Ok(true);
            }
            if start.elapsed() >= timeout {
                return Ok(false);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    // ── Transaction-level advisory locks ────────────────────────────────────

    /// Acquire a **transaction-level** advisory lock.
    ///
    /// Automatically released when the transaction commits or rolls back.
    /// Consider using [`TransactionService`](crate::services::TransactionService)
    /// together with `pg_advisory_xact_lock`.
    pub async fn acquire_xact(key: i64, pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(key)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Try to acquire a transaction-level advisory lock.
    pub async fn try_acquire_xact(key: i64, pool: &sqlx::PgPool) -> Result<bool, sqlx::Error> {
        let row: (bool,) = sqlx::query_as("SELECT pg_try_advisory_xact_lock($1)")
            .bind(key)
            .fetch_one(pool)
            .await?;
        Ok(row.0)
    }
}
