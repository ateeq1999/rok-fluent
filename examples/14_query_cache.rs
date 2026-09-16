//! Query result cache — `SelectBuilder::fetch_all_cached`.
//!
//! Simulates a read-heavy `GET /posts` endpoint: a burst of concurrent
//! `tokio::spawn` tasks hit the same query at once *against a cold cache* —
//! nobody has read this query yet, so without single-flight coalescing this
//! would be exactly the "thundering herd" scenario a cache is supposed to
//! protect against (100 callers all miss at once, all 100 query the
//! database). Via `orm::postgres::query_log::set_on_query` we count every
//! *real* SQL query that actually reaches the database (a cache hit — or a
//! caller that coalesces onto someone else's in-flight query — never calls
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
//! # Why the burst can safely run against a cold cache
//!
//! `rok_fluent::orm::cache::get_or_populate` (the internal helper behind
//! both `_cached` terminals, see `src/orm/cache.rs`) single-flights concurrent
//! callers for the same key: when N callers all miss a *cold* cache key at
//! once, they join the one in-flight attempt instead of each starting their
//! own, so exactly 1 real query reaches the database no matter how many
//! callers race in — not "the first request to arrive happens to win," but a
//! guarantee. This example bursts 100 concurrent readers straight at an empty
//! cache to demonstrate exactly that guarantee, rather than warming the cache
//! with a first read before bursting (which was the workaround this example
//! used before single-flight coalescing existed).

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

    // ── Burst: 100 concurrent "GET /posts"-style callers hit a completely
    // cold cache at once ─────────────────────────────────────────────────────
    // Nobody has read `posts` yet — this is the thundering-herd scenario.
    // Single-flight coalescing inside `get_or_populate` (src/orm/cache.rs)
    // means all 100 callers join the one in-flight query instead of each
    // issuing their own, so exactly 1 real query reaches the database.
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
        "\n100 concurrent callers hitting a cold cache at once → {fired_after_burst} real DB quer{suffix} total",
        suffix = if fired_after_burst == 1 { "y" } else { "ies" }
    );
    assert_eq!(
        fired_after_burst, 1,
        "100 concurrent callers racing a cold cache key must coalesce into exactly 1 real query"
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
