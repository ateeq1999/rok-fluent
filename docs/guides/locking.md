# Locking

rok-fluent supports two types of locking:

- **Advisory locks** — application-level named locks via `LockService`
- **Row-level locking** — PostgreSQL `FOR UPDATE` / `FOR SHARE` via the typed DSL

Both are PostgreSQL-specific and require `features = ["postgres"]`.

See [`examples/04_transactions_locking.rs`](../../examples/04_transactions_locking.rs)
for a runnable version of `LockService` advisory locking (row-level `FOR UPDATE`
locking is covered by the DSL snippets below).

## `LockService` (Advisory Locks)

PostgreSQL advisory locks are named locks stored in memory (not on rows). They are
useful for coordinating work across connections.

### Basic acquire / release

```rust,ignore
use rok_fluent::services::LockService;

// Blocking acquire — waits until the lock is available. Keys are `i64`; pick
// any app-specific constant per lock.
LockService::acquire(1, &pool).await?;

// Do work…
LockService::release(1, &pool).await?;
```

### Non-blocking try-acquire

```rust,ignore
if LockService::try_acquire(2, &pool).await? {
    // Lock acquired — do work, then release
    LockService::release(2, &pool).await?;
} else {
    // Lock held by another session — skip
}
```

### Acquire with timeout

```rust,ignore
use std::time::Duration;

// Fail if the lock isn't acquired within 5 seconds
LockService::acquire_timeout(3, Duration::from_secs(5), &pool).await?;
```

### Transaction-scoped advisory locks

Auto-released when the transaction commits or rolls back — no explicit release needed.

```rust,ignore
LockService::acquire_xact(4, &pool).await?;
LockService::try_acquire_xact(4, &pool).await?;
```

### Advisory lock naming

Lock keys are plain `i64` values passed straight through to the underlying
`pg_advisory_lock` / `pg_try_advisory_lock` functions — there is no string
hashing step. Use distinct, well-documented constants (e.g. one `const` per
lock) to avoid collisions.

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

let user: User = User::filter("id", 42_i64)
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
