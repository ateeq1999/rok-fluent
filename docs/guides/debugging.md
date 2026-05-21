# Guide: Debugging Queries

Two complementary tools for inspecting what SQL rok-fluent generates.

## `.inspect()` — print without leaving the chain

Every query builder (`SelectBuilder`, `InsertBuilder`, `UpdateBuilder`, `DeleteBuilder`) has an
`.inspect()` method that prints the rendered SQL and bound parameters to `stderr` and then returns
`self` unchanged, so it can be dropped anywhere into a builder chain without modifying behavior.

```rust,no_run
use rok_fluent::dsl::db;

let rows = db::select()
    .from(User::table())
    .where_(User::EMAIL.like("%@example.com"))
    .order_by(User::NAME.asc())
    .limit(10)
    .inspect()          // ← prints SQL + params to stderr, then passes through
    .fetch_all::<User>(&pool)
    .await?;
```

Output on `stderr`:
```
[rok-fluent] SELECT * FROM "users" WHERE "users"."email" LIKE $1 ORDER BY "users"."name" ASC LIMIT 10
[rok-fluent] params: [Text("%@example.com")]
```

When the `tracing` feature is enabled, `.inspect()` also emits a `tracing::debug!` event with
`sql` and `params` fields, which flows into your existing subscriber (Jaeger, Datadog, stdout JSON,
etc.).

### Usage patterns

**Debug a specific query during development:**
```rust,no_run
User::query()
    .where_eq("role", "admin")
    .inspect()
    .get()
    .await?;
```

**Keep inspect in staging, remove in prod (feature-gate):**
```rust,no_run
let q = User::query().where_eq("role", "admin");
#[cfg(debug_assertions)]
let q = q.inspect();
q.get().await?;
```

## `.explain()` / `.explain_json()` — ask PostgreSQL for the plan

`SelectBuilder` has two `EXPLAIN`-based methods (requires the `postgres` feature).

### `.explain(pool)` — human-readable plan

Returns `EXPLAIN` output as a single `String` with newline-separated plan lines.

```rust,no_run
let plan = db::select()
    .from(User::table())
    .where_(User::EMAIL.like("%@example.com"))
    .explain(&pool)
    .await?;

println!("{plan}");
// Seq Scan on users  (cost=0.00..18.50 rows=1 width=128)
//   Filter: ((email)::text ~~ '%@example.com'::text)
```

### `.explain_json(pool)` — machine-readable plan

Returns `EXPLAIN (FORMAT JSON)` output as a `serde_json::Value`.

```rust,no_run
let plan_json = db::select()
    .from(User::table())
    .where_(User::EMAIL.like("%@example.com"))
    .explain_json(&pool)
    .await?;

let total_cost = plan_json[0]["Plan"]["Total Cost"].as_f64();
```

### Reading query plans

Key fields to watch for in PostgreSQL plans:

| Field | Watch for |
|---|---|
| `Node Type` | `Seq Scan` on large tables → add index |
| `Total Cost` | Higher = slower. Compare before/after adding indexes. |
| `Rows` | Large estimates on filtered queries → statistics stale, run `ANALYZE`. |
| `Loops` | High loop counts in nested loops → potential N+1. |
| `Filter` | Filters applied *after* the scan → index not used, check column selectivity. |

## Combining both

```rust,no_run
let q = db::select()
    .from(Order::table())
    .where_(Order::USER_ID.eq(user_id))
    .where_(Order::STATUS.eq("pending"))
    .order_by(Order::CREATED_AT.desc())
    .limit(50);

// See the SQL
let q = q.inspect();

// Also see the plan (postgres only, dev/staging)
#[cfg(debug_assertions)]
{
    let plan = q.clone_for_explain().explain(&pool).await?;
    eprintln!("[plan]\n{plan}");
}

let orders = q.fetch_all::<Order>(&pool).await?;
```

> **Note:** `.explain()` consumes the builder. If you need both the plan and the results, build
> the query twice or keep the parameters separate.
