# Locking

rok-fluent supports two types of locking:

- **Advisory locks** — application-level named locks via `LockService`
- **Row-level locking** — PostgreSQL `FOR UPDATE` / `FOR SHARE` via the typed DSL

Both are PostgreSQL-specific and require `features = ["postgres"]`.

## `LockService` (Advisory Locks)

PostgreSQL advisory locks are named locks stored in memory (not on rows). They are
useful for coordinating work across connections.

### Basic acquire / release

```rust,ignore
use rok_fluent::services::LockService;

// Blocking acquire — waits until the lock is available
LockService::acquire("deploy_lock", &pool).await?;

// Do work…
LockService::release("deploy_lock", &pool).await?;
```

### Non-blocking try-acquire

```rust,ignore
if LockService::try_acquire("cache_build_lock", &pool).await? {
    // Lock acquired — do work, then release
    LockService::release("cache_build_lock", &pool).await?;
} else {
    // Lock held by another session — skip
}
```

### Acquire with timeout

```rust,ignore
use std::time::Duration;

// Fail if the lock isn't acquired within 5 seconds
LockService::acquire_timeout("rate_limiter", Duration::from_secs(5), &pool).await?;
```

### Transaction-scoped advisory locks

Auto-released when the transaction commits or rolls back — no explicit release needed.

```rust,ignore
LockService::acquire_xact("order_processor", &pool).await?;
LockService::try_acquire_xact("order_processor", &pool).await?;
```

### Advisory lock naming

Lock names are converted to `i64` hash values for the underlying
`pg_advisory_lock` / `pg_try_advisory_lock` functions. Use descriptive,
unique string keys to avoid collisions.

## Row-Level Locking (DSL)

Attach a locking clause to a `SELECT` query via the typed DSL:

```rust,ignore
use rok_fluent::dsl::{db, Lock, LockConflict};
use rok_fluent::dsl::Lock;

let user: User = db::select()
    .from(User::table())
    .where_(User::ID.eq(42_i64))
    .lock(Lock::ForUpdate)
    .lock_conflict(LockConflict::NoWait)
    .fetch_one(&pool)
    .await?;
```

### Lock strengths

| Variant | SQL | Behaviour |
|---|---|---|
| `Lock::ForUpdate` | `FOR UPDATE` | Block other write locks |
| `Lock::ForNoKeyUpdate` | `FOR NO KEY UPDATE` | Allow concurrent key-locking reads |
| `Lock::ForShare` | `FOR SHARE` | Shared lock, allow concurrent shared locks |
| `Lock::ForKeyShare` | `FOR KEY SHARE` | Weakest, allow non-key writes |

### Conflict strategies

| Variant | SQL | Behaviour |
|---|---|---|
| `LockConflict::SkipLocked` | `SKIP LOCKED` | Skip rows that can't be locked |
| `LockConflict::NoWait` | `NOWAIT` | Error immediately if row is locked |

Call `.lock_conflict()` after `.lock()`:

```rust,ignore
.lock(Lock::ForUpdate)
.lock_conflict(LockConflict::SkipLocked)
```

### Active Record locking

Row-level locking is not available on `ModelQuery` directly. Convert to a DSL
`SelectBuilder` first:

```rust,ignore
use rok_fluent::dsl::{db, Lock};

let user: User = User::query()
    .where_eq("id", 42_i64)
    .into_dsl()
    .lock(Lock::ForUpdate)
    .fetch_one(&pool)
    .await?;
```

## Feature flags

```
# Advisory locks (LockService)
rok-fluent = { version = "0.4", features = ["active", "postgres"] }

# Row-level locking on SelectBuilder
rok-fluent = { version = "0.4", features = ["query", "postgres"] }
```
