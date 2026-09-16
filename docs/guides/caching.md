# Guide: Query Caching

## Feature

```toml
rok-fluent = { version = "0.4", features = ["postgres", "query", "cache", "macros"] }
```

`orm::cache`'s generic `get`/`put`/`invalidate_table`/`clear` primitives only need the
`cache` feature. The `_cached` terminals on `SelectBuilder` — the ones you'll actually
call day to day — additionally need `postgres` + `query`, since they're built on top of
`SelectBuilder::fetch_all`/`fetch_optional`, which are PostgreSQL-only today.

## The problem: a read-heavy endpoint

Say you have a `GET /posts` endpoint doing ~100 req/s, and most of those requests run
the exact same query — the same filters, the same sort, the same page. Without caching,
every request round-trips to PostgreSQL even though the underlying data barely changes
between requests. `rok_fluent::orm::cache` gives you an opt-in, per-query, TTL-based
cache in front of exactly that kind of query, so repeated reads within the TTL window
cost zero additional database round trips.

Nothing is cached implicitly — you opt in **per query** by calling a `_cached` terminal
and supplying a TTL. Every other terminal (`fetch_all`, `fetch_optional`, `count`, etc.)
is completely unaffected by this feature, so enabling `cache` never changes behavior you
didn't explicitly ask for.

## Opting in: `fetch_all_cached` / `fetch_optional_cached`

```rust,no_run
use std::time::Duration;
use std::sync::Arc;
use rok_fluent::dsl::db;

# #[derive(Debug, sqlx::FromRow, rok_fluent::TableDerive)]
# #[table(name = "posts")]
# struct Post { id: i64, title: String }
# async fn example(pool: sqlx::PgPool) -> Result<(), sqlx::Error> {
// First call misses the cache and queries the database; every call within
// the TTL after that is served from the cache with zero DB round trips.
let posts: Arc<Vec<Post>> = db::select()
    .from(Post::table())
    .fetch_all_cached::<Post>(&pool, Duration::from_secs(30))
    .await?;

let post: Option<Arc<Post>> = db::select()
    .from(Post::table())
    .where_(Post::ID.eq(1_i64))
    .fetch_optional_cached::<Post>(&pool, Duration::from_secs(30))
    .await?;
# Ok(())
# }
```

Both terminals mirror their non-cached counterparts (`fetch_all`/`fetch_optional`)
exactly in *what* query runs — the only difference is the cache check/store wrapped
around it. They return `Arc<Vec<T>>` / `Option<Arc<T>>` rather than `Vec<T>` /
`Option<T>`: a cache hit hands every caller a clone of the same `Arc` (cheap, no data
copy) without requiring `T: Clone`. Only the caller that actually causes a miss pays for
running the query; everyone else within the TTL gets the cached `Arc`.

## How the cache key is constructed

`SqlValue` and `SelectBuilder` have no `Hash`/`Eq` implementation, so the cache key is
derived from the *rendered* query output rather than the builder itself:

```text
"{table}:{rendered_sql}|{params joined by SqlValue::to_sql_literal()}"
```

This means two structurally different queries — different `WHERE` clauses, different
`ORDER BY`, different bound values — always render to different SQL strings and/or
different parameter lists, so they land under different keys and never collide, even
though they both start with the same `"{table}:"` prefix. The `"{table}:"` prefix is
also exactly what table-based invalidation matches against (see below).

## Invalidation

A write through any path this crate controls busts the cache for the table(s) it
touched automatically:

- `InsertBuilder::execute`, `UpdateBuilder::execute`, `DeleteBuilder::execute` (the
  typed DSL write terminals, feature `postgres` + `cache`)
- The Active Record write path — `orm::postgres::executor::insert`/`update`/`delete`

Each of these calls `orm::cache::invalidate_table(table)` after a successful write,
which evicts every cache entry keyed under that table's `"{table}:"` prefix — so nothing
goes silently stale on any write path the DSL or Active Record layer controls:

```rust,no_run
use rok_fluent::dsl::db;
# #[derive(Debug, sqlx::FromRow, rok_fluent::TableDerive)]
# #[table(name = "posts")]
# struct Post { id: i64, title: String }
# async fn example(pool: sqlx::PgPool) -> Result<(), sqlx::Error> {
// A write through the DSL busts the cache for "posts" automatically.
db::insert_into(Post::table())
    .values([("title", "New post")])
    .execute(&pool)
    .await?;
# Ok(())
# }
```

### Manual invalidation for writes outside the builders

Anything that writes to the database *outside* the DSL/AR paths — raw `sqlx::query`
calls, a trigger-driven update, a bulk `COPY` run by another process — has no way for
rok-fluent to know it happened, so the cache won't be invalidated automatically. Call
`invalidate_table`/`clear` yourself right after such a write:

```rust,no_run
# async fn example(pool: sqlx::PgPool) -> Result<(), sqlx::Error> {
sqlx::query("UPDATE posts SET title = $1 WHERE id = $2")
    .bind("Updated title")
    .bind(1_i64)
    .execute(&pool)
    .await?;

// Manual escape hatch — nothing else will evict "posts" entries for this write.
rok_fluent::orm::cache::invalidate_table("posts");

// Or, to evict everything regardless of table:
rok_fluent::orm::cache::clear();
# Ok(())
# }
```

## Known limitations

**Single-flight coalescing is per-process only.** `fetch_all_cached`/`fetch_optional_cached`
route through an internal `orm::cache::get_or_populate` helper that coalesces concurrent
callers for the same cold (empty or just-invalidated) cache key: when N callers miss the
same key at once, they join the one in-flight query instead of each starting their own —
so a cold-cache stampede produces exactly 1 real database query for N concurrent
first-time callers within a single process, not N. A failed attempt (the query itself
returns `Err`) is shared the same way and does not poison the key — the next caller
retries from scratch. See [`examples/14_query_cache.rs`](../../examples/14_query_cache.rs)
for this demonstrated directly against a cold cache.

This coalescing is scoped to one process: `QUERY_CACHE` and the in-flight registry behind
it are both process-local `DashMap`s (per the module doc in `src/orm/cache.rs`), with no
cross-process or cross-instance coordination. If you run multiple instances behind a load
balancer, each instance's cold-cache stampede is collapsed to 1 query *per instance*, not
1 query cluster-wide — e.g. 4 instances each independently missing the same key at once
still produces up to 4 real queries in aggregate, one per instance. A shared cache tier
(Redis, etc.) with its own coordination is out of scope for `orm::cache`, which remains
intentionally a simple in-process registry.

## See also

- [`examples/14_query_cache.rs`](../../examples/14_query_cache.rs) — a `GET /posts`-style
  burst of 100 concurrent `tokio::spawn` readers hitting a completely *cold* cache at
  once (1 real query, not 100 — the single-flight guarantee above), plus a write that
  busts the cache and forces the next read to hit the database again.
- [ORM API docs — Query Result Cache](../api/orm.md#query-result-cache-rok_fluentormcache--feature-cache-terminals-also-need-postgres--query)
- [Features — `cache`](../features.md#cache)
