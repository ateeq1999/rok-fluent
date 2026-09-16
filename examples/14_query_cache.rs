//! Query result cache — `SelectBuilder::fetch_all_cached`.
//!
//! Simulates a read-heavy `GET /posts` endpoint: a burst of concurrent
//! `tokio::spawn` tasks hit the same query at once. Via
//! `orm::postgres::query_log::set_on_query` we count every *real* SQL query
//! that actually reaches the database (a cache hit never calls
//! `fetch_all_as`/`log_query`) and show a write immediately busts the cache
//! so the next read hits the database again.
//!
//! Requires a live PostgreSQL database:
//!
//! ```sh
//! DATABASE_URL=postgres://user:pass@localhost/rok_fluent_db \
//!   cargo run --example 14_query_cache --features postgres,query,cache
//! ```
//!
//! # Why PostgreSQL-only
//!
//! Like `examples/02_crud.rs` and `examples/09_window_functions_cte.rs`, the
//! DSL's async terminals (`SelectBuilder::fetch_all`, etc.) only exist for
//! PostgreSQL (`#[cfg(feature = "postgres")]` in `src/dsl/select.rs`), so the
//! `cache`-gated `fetch_all_cached`/`fetch_optional_cached` terminals added on
//! top of them are PostgreSQL-only too — sqlite/mysql have no DSL terminals
//! to cache in front of yet.
//!
//! # Why the burst runs against an already-warm cache
//!
//! `rok_fluent::orm::cache` is a plain `get` → (miss) → run query → `put`
//! sequence with no per-key lock held across the query (see
//! `src/orm/cache.rs`'s module doc: "DashMap ops are sync + short — never
//! hold a guard across an `.await`"). That means it has no single-flight /
//! request-coalescing behavior: if 100 callers all miss a *cold* cache at
//! once, they can all race to query the database before any of them has
//! stored a result — the cache guarantees "don't repeat a *satisfied* read
//! within the TTL," not "collapse concurrent first-time misses into one
//! query." So this example first issues one read to populate the cache (the
//! realistic case — some request is always first), *then* bursts 100
//! concurrent readers against the now-warm cache to demonstrate the actual
//! guarantee: repeated reads within the TTL cost zero additional queries.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rok_fluent::dsl::db;
use rok_fluent::orm::postgres::query_log::{self, QueryEvent};

#[derive(Debug, sqlx::FromRow, rok_fluent::TableDerive)]
#[table(name = "posts")]
pub struct Post {
    pub id: i64,
    pub title: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/rok_fluent_db".into());
    let pool = sqlx::PgPool::connect(&database_url).await?;

    sqlx::query("DROP TABLE IF EXISTS posts")
        .execute(&pool)
        .await?;
    sqlx::query(
        "CREATE TABLE posts (
            id    BIGSERIAL PRIMARY KEY,
            title TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await?;

    db::insert_into(Post::table())
        .values([("title", "Hello, cache")])
        .execute(&pool)
        .await?;

    // Count every real SQL query the cached terminal actually issues — a
    // cache hit returns before ever calling `fetch_all_as`/`log_query`.
    let real_queries = Arc::new(AtomicU64::new(0));
    let counter = real_queries.clone();
    query_log::set_on_query(Arc::new(move |event: &QueryEvent| {
        counter.fetch_add(1, Ordering::SeqCst);
        println!(
            "[db] real query #{}: {}",
            counter.load(Ordering::SeqCst),
            event.sql
        );
    }));

    // ── Warm the cache with the first, cache-populating read ───────────────
    db::select()
        .from(Post::table())
        .fetch_all_cached::<Post>(&pool, Duration::from_secs(30))
        .await?;
    assert_eq!(
        real_queries.load(Ordering::SeqCst),
        1,
        "first read must hit the DB"
    );
    println!("Cache warmed with 1 real query.\n");

    // ── Burst: 100 concurrent "GET /posts"-style callers against the warm
    // cache ──────────────────────────────────────────────────────────────────
    let mut handles = Vec::with_capacity(100);
    for _ in 0..100 {
        let pool = pool.clone();
        handles.push(tokio::spawn(async move {
            db::select()
                .from(Post::table())
                .fetch_all_cached::<Post>(&pool, Duration::from_secs(30))
                .await
        }));
    }
    for h in handles {
        h.await??;
    }

    let fired_after_burst = real_queries.load(Ordering::SeqCst);
    println!(
        "\n100 concurrent cache-hitting callers → {fired_after_burst} real DB quer{suffix} total",
        suffix = if fired_after_burst == 1 { "y" } else { "ies" }
    );
    assert_eq!(
        fired_after_burst, 1,
        "100 concurrent reads against a warm cache must not add any real queries"
    );

    // ── Write busts the cache ───────────────────────────────────────────────
    // `InsertBuilder::execute` calls `cache::invalidate_table("posts")` on
    // success (see src/dsl/insert.rs), evicting every entry keyed under "posts".
    db::insert_into(Post::table())
        .values([("title", "Second post")])
        .execute(&pool)
        .await?;

    // ── Next read misses the cache and hits the DB again ────────────────────
    let posts = db::select()
        .from(Post::table())
        .fetch_all_cached::<Post>(&pool, Duration::from_secs(30))
        .await?;
    let titles: Vec<&str> = posts.iter().map(|p| p.title.as_str()).collect();
    println!("\nAfter write: {} post(s): {titles:?}", posts.len());

    let fired_after_write = real_queries.load(Ordering::SeqCst);
    println!("After a write + one more read → {fired_after_write} real DB queries total");
    assert_eq!(
        fired_after_write, 2,
        "the write must invalidate the cache, so the next read is exactly one more real query"
    );

    query_log::clear_on_query();
    Ok(())
}
