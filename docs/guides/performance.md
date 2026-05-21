# Guide: Performance Tuning

## Pool warming

Pre-open connections at startup so the pool is ready for immediate burst traffic:

```rust,no_run
use rok_fluent::orm::postgres::pool;

async fn main() {
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();

    // Open 5 connections before accepting requests.
    pool::warm(5, &pool).await.unwrap();

    // Start serving...
}
```

Without warming, the first `n` concurrent requests each pay the TCP + TLS + auth
round-trip to open a new connection. Warming shifts that cost to startup.

---

## Bulk insert: `copy_insert` vs `bulk_insert`

For inserting large numbers of rows, prefer `BatchService::copy_insert` over
`bulk_insert` when inserting 100+ rows:

| Method | Mechanism | Speed | Notes |
|---|---|---|---|
| `bulk_insert` | Multi-row `INSERT INTO … VALUES (…)` | Baseline | Hits PostgreSQL's 65535 parameter limit for wide tables |
| `copy_insert` | `COPY FROM STDIN (FORMAT CSV)` | 10–50× faster | No parameter limit; no RETURNING support |
| `bulk_insert_chunked` | Chunked multi-row INSERT | Bounded memory | Use when you need RETURNING or must stay under param limits |

```rust,no_run
use rok_fluent::services::BatchService;

// Small batch (< 100 rows) or when you need RETURNING:
BatchService::<User>::bulk_insert(&rows, &pool).await?;

// Large batch (100+ rows), no RETURNING needed:
BatchService::<User>::copy_insert(&rows, &pool).await?;
```

### `copy_insert` caveats

- No `RETURNING` clause — you won't get the inserted rows back.
- All rows must provide the same columns in the same order.
- Bypasses column defaults for omitted columns (unlike INSERT which applies them).
- Triggers and row-level security policies **do** fire (unlike `COPY` from superuser context).

---

## `= ANY($1)` vs `IN ($1, $2, …)`

For filtering by a list of values, `= ANY(ARRAY[…])` uses a fixed number of
parameters regardless of list length, which allows the database to reuse the same
prepared statement:

```rust,no_run
// IN — N parameters, N different prepared statements for N different list lengths:
User::query().where_in("id", vec![1_i64, 2, 3]).get().await?;

// = ANY — always 1 parameter, 1 prepared statement:
User::query().where_eq_any("id", vec![1_i64, 2, 3]).get().await?;

// DSL equivalent:
db::select()
    .from(User::table())
    .where_(User::ID.eq_any([1_i64, 2, 3]))
    .fetch_all::<User>(&pool)
    .await?;
```

Use `eq_any` / `where_eq_any` in hot paths where the same query runs many times
with different list sizes (e.g., loading a user's permissions, filtering by tag IDs).

### When to prefer `IN`

- Lists with mixed value types (fall back to `IN`).
- SQLite and MySQL backends (rok-fluent renders `= ANY` as `IN` automatically).
- When you need the explicit `NOT IN` semantics (`not_in` / `where_not_in` still expand individually).

---

## Streaming large result sets

For queries that return many rows, use `stream()` on `SelectBuilder` to avoid
buffering the full result in memory:

```rust,no_run
use futures::TryStreamExt;

let mut stream = db::select()
    .from(Order::table())
    .where_(Order::STATUS.eq("pending"))
    .order_by(Order::CREATED_AT.asc())
    .stream::<Order>(&pool);

while let Some(order) = stream.try_next().await? {
    process(&order).await;
}
```

This yields rows one at a time and is suitable for export pipelines, background
processing jobs, and reporting queries where the full result set might be too large
to hold in memory.

---

## Pool metrics

Monitor connection pool health to tune `max_connections`:

```rust,no_run
use rok_fluent::orm::postgres::pool;

let m = pool::snapshot(&pool);
println!("size={} idle={} active={}", m.size, m.idle, m.active);

// With the `metrics` feature, emit as Prometheus gauges:
#[cfg(feature = "metrics")]
pool::emit_metrics(&pool, "primary");
```

Signs you need a larger pool: `active` consistently near `size` with high query latency.
Signs of an over-provisioned pool: `idle` is almost always equal to `size`.
