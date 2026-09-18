# Changelog

All notable changes are documented here following [Keep a Changelog](https://keepachangelog.com)
conventions. Versions follow [Semantic Versioning](https://semver.org).

---

## [Unreleased]

### Added

- **`ModelValues` trait** (`rok_fluent::ModelValues` / `rok_fluent::core::model::ModelValues`)
  — `to_values(&self) -> Vec<(&'static str, SqlValue)>`, generated automatically by
  `#[derive(Model)]` for every non-`#[table(skip)]` field.
- **`PgModel::insert`/`save`/`destroy`** (feature `active` + `postgres`) — hook-aware
  instance methods: `user.insert(&pool).await?`, `user.save(&pool).await?`,
  `user.destroy(&pool).await?`. Require `Self: Hooks` (`insert`/`save` also require
  `ModelValues`; `destroy` also requires `Sync` since it holds `&self` across the
  `.await` to call `after_delete`). Existing static `PgModel::create`/`update_by_pk`/
  `delete_by_pk` are unchanged. See [`examples/11_hooks.rs`](../examples/11_hooks.rs).
- **`validate` feature** — integrates the [`validator`](https://docs.rs/validator) crate
  (`0.21`, `derive` feature) with `Hooks`: `impl From<validator::ValidationErrors> for
  OrmError` (`rok_fluent::orm::hooks`), so a model that also
  `#[derive(validator::Validate)]` can call `self.validate().map_err(OrmError::from)?`
  inside `before_save`/`before_create`. rok-fluent does not parse `#[validate(...)]`
  attributes itself — that's `validator`'s own derive macro. See
  [`examples/12_validation.rs`](../examples/12_validation.rs).
- **Repository / DI override** (`rok_fluent::orm::postgres::repository`, feature
  `active` + `postgres`) — `Repository<M: PgModel>` trait (`#[async_trait]`,
  default bodies delegate to the corresponding static `PgModel` method:
  `find_by_pk`, `create`, `update_by_pk`, `delete_by_pk`, `all`);
  `register::<M, R>(repo: R)` to install an override at startup; `PgModel`'s own
  `find_by_pk`/`create`/`update_by_pk`/`delete_by_pk`/`all` check the per-model
  registry first and fall through to the existing `executor::*` path when nothing
  is registered — fully additive, zero behavior change for callers who never call
  `register()`. The `active` feature now also pulls in `dep:async-trait` (already
  used by `migrate`). See [`examples/13_repository_di.rs`](../examples/13_repository_di.rs).
- **`cache` feature** — opt-in, per-query result cache (`rok_fluent::orm::cache`):
  `get::<T>`/`put::<T>`/`invalidate_table`/`clear`, a process-wide TTL-based
  `DashMap` registry mirroring the `NAMED_POOLS` pattern in `orm::postgres::pool`.
  New `SelectBuilder::fetch_all_cached`/`fetch_optional_cached` terminals (feature
  `postgres` + `cache`) return `Arc<Vec<T>>`/`Option<Arc<T>>` so a cache hit never
  requires `T: Clone`; existing `fetch_all`/`fetch_optional`/etc. are untouched.
  `InsertBuilder::execute`/`UpdateBuilder::execute`/`DeleteBuilder::execute` and
  the Active Record write path (`executor::insert`/`update`/`delete`) all call
  `invalidate_table` for the affected table after a successful write, so nothing
  is cached implicitly and nothing goes silently stale on the write paths this
  crate controls. `dashmap` is now also declared independently of `postgres`, so
  enabling `cache` alone doesn't force-enable a database backend. Out of scope
  this phase: sqlite/mysql cache read-through — the DSL's async terminals are
  PostgreSQL-only today. See [`examples/14_query_cache.rs`](../examples/14_query_cache.rs).
- **Single-flight coalescing for the query result cache** (`rok_fluent::orm::cache`,
  feature `postgres` + `cache`) — `fetch_all_cached`/`fetch_optional_cached` now route
  through an internal `get_or_populate` helper (`src/orm/cache.rs`) that coalesces
  concurrent callers for the same cold (empty or just-invalidated) cache key: N
  concurrent misses for the same key now produce exactly 1 real database query, not N,
  fixing the cold-cache "thundering herd" gap noted in the cache feature's initial
  release. A failed attempt is shared with every waiter (each gets a `sqlx::Error`, not
  a hang or a panic) and does not poison the key — the next caller retries from scratch.
  Implemented with a second process-wide `DashMap` of `tokio::sync::OnceCell`s
  (`IN_FLIGHT`), cleaned up as each attempt resolves and, via a small `Drop` guard, if
  every waiter for a key is cancelled before it resolves — no permanent growth from
  abandoned keys. This coalescing is per-process only, not cluster-wide; see
  `docs/guides/caching.md`'s "Known limitations". `examples/14_query_cache.rs` now bursts
  its 100 concurrent readers against a genuinely cold cache instead of warming it first.

### Changed

- **Breaking:** `ModelHooks` renamed to `Hooks` (`rok_fluent::orm::hooks::Hooks`). Same
  shape — `before_create`/`after_create`/`before_update`/`after_update`/`before_save`/
  `after_save`/`before_delete`/`after_delete`, all default no-op. Update any
  `impl ModelHooks for ...` to `impl Hooks for ...`.

### Fixed

- **Docs correctness pass** — `README.md` and every page under `docs/` mixed up two
  API surfaces in their code examples: (1) `Model::query()` (the plain, non-awaitable
  `core::query::QueryBuilder<T>`, meant to be handed to `M::find_where(pool, builder)`
  or the DSL bridge) was repeatedly shown as if it were chainable and awaitable with
  `.where_eq()...all()/.get().await?` — the real fluent Active Record entry points are
  `PgModel::filter(col, val)`, `PgModel::all_query()`, and `PgModel::find_query(id)`,
  which return the actually-awaitable `ModelQuery<T>` (terminals `.get()`, `.first()`,
  `.paginate()`, etc.; chaining uses `.and_where()`, not `.where_eq()`); and (2)
  `Tx::run(|tx| ...)` (`rok_fluent::orm::postgres::transaction::Tx`) doesn't exist —
  replaced with manual `Tx::begin()` / `.commit()` or `Tx::run_with_retry(&pool,
  &RetryConfig, ...)` (the retry config is a `&RetryConfig`, not a bare integer).
  Also fixed several bugs found alongside these: `ModelQuery::paginate`/
  `simple_paginate`/`cursor_paginate` argument order (`per_page` comes first, not
  `current_page`), `FilterBuilder`/`SortBuilder::apply()` being called in the wrong
  direction, `scopes::register::<T>()` being called with a stray extra type
  parameter, and a fabricated `LocalScope` type / `.scope()` method that don't exist
  (local scopes are plain `impl` methods returning `ModelQuery<Self>`).
- **Removed all documented references to a `query!` shorthand macro** (README.md,
  `docs/features.md`, `docs/getting-started.md`, `docs/index.md`,
  `docs/architecture.md`, `rok-fluent-macros/CLAUDE.md`) — this macro was never
  implemented anywhere in `src/` (confirmed via repo-wide grep for `macro_rules!`);
  the docs described a feature that doesn't exist. The fluent, chainable query API
  (`PgModel::filter()`/`.all_query()`/`.find_query()` → `ModelQuery<T>`) is unaffected
  and remains the real, working way to build queries.

### Removed

- **Breaking:** `Observer<T>`, `observe()`, `clear_observers()`, and the internal
  observer registry removed from `rok_fluent::orm::hooks`. This was dead code — the
  registry was never consulted by any dispatch path, so no working behavior is lost.
  `OrmError`, `OrmResult`, `without_events`, and `observers_muted` are unchanged.

---

## [0.4.1] — 2026-05-21

### Added

- **`TransactionService`** — `TransactionService::begin()`, `TxCtx::savepoint/rollback_to/release`, pool-free CRUD on `&mut Transaction`. See [transactions guide](guides/transactions.md).
- **`LockService`** — advisory locks (`acquire`, `try_acquire`, `release`, `acquire_xact`, `acquire_timeout`), row-level locking on `SelectBuilder` (`ForUpdate`, `ForNoKeyUpdate`, `ForShare`, `ForKeyShare`, `SkipLocked`, `NoWait`). See [locking guide](guides/locking.md).
- **`SchemaInspector`** — `columns()`, `indexes()`, `foreign_keys()` queries against `information_schema` (PostgreSQL). Used by `rok db schema dump`.
- **Window functions** — `rank()`, `row_number()`, `dense_rank()`, `ntile(n)` free functions; `Column::lag(n)/lead(n)/first_value()/last_value()`; `Window` builder with `partition_by`/`order_by`; `SelectBuilder::win_col()`. See [ORM docs](api/orm.md).
- **`TypedJson<T>`** — typed `jsonb` column wrapper via `sqlx::types::Json<T>` delegation. `From<TypedJson<T>> for SqlValue`.
- **MSRP policy** — `rust-version = "1.85"` in `Cargo.toml`; CI validates against MSRP.
- **`cargo-fuzz` targets** — SQL rendering correctness fuzzers for PostgreSQL (`sql_render_pg`) and SQLite (`sql_render_qmark`).
- **`criterion` benchmarks** — query build time benchmarks (7 benchmarks, 143ns–2.17µs range).
- **`QueryLog` structured sink** — `QueryEvent { sql, params, duration_ms, table, rows_affected }`; `tracing::trace!` span emitted when `tracing` feature is enabled.
- **`docs.rs` metadata** — `all-features = true` + `--cfg docsrs` for auto-cfg feature badges.
- **`SelectBuilder::distinct_on(cols)`** — PostgreSQL `SELECT DISTINCT ON (col, …)`.
- **OOP primary DSL API** (`#[derive(Table)]`) — `User::table()`, `User::ID`, `User::NAME` SCREAMING_SNAKE_CASE column constants on every `#[derive(Table)]` struct.
- **DSL JOIN builder** — `SelectBuilder::inner_join`, `left_join`, `right_join`, `cross_join`. `Column::references()` / `eq_col()` for typed ON clauses.
- **DSL aggregators** — `Column::count()`, `sum()`, `avg()`, `min()`, `max()`, `count_distinct()`. `SelectBuilder::group_by()`, `having()`. `AggExpr` with HAVING comparison operators.
- **DSL pagination** — `SelectBuilder::paginate()`, `simple_paginate()`, `cursor_paginate()`, `count()`, `exists()` terminals.
- **Advanced `Expr`** — `Expr::case()` / `CaseExpr`, `Expr::exists()`, `Expr::not_exists()`, `Expr::InSubquery`, `Expr::NotInSubquery`. Column functions: `lower()`, `upper()`, `length()`, `trim()`, `coalesce()`, `cast_as()`, `date_trunc()`, `extract()` → `FnExpr`.
- **Subqueries, CTEs, set operations** — `SelectBuilder::from_subquery()`, `with_cte()`, `from_cte()`, `union()`, `union_all()`, `intersect()`, `except()`.
- **Upsert + RETURNING** — `InsertBuilder::on_conflict_do_nothing()`, `on_conflict()`, `do_update_excluded()`, `do_update_values()`, `returning()`, `fetch_one/fetch_all`. `UpdateBuilder::set_col()`, `set_typed()`, `returning()`, `fetch_one/fetch_all`.
- **`Loaded<T>`** — relationship carrier enum (`NotLoaded` / `Some(T)`) with `Serialize`/`Deserialize`.
- **Service layer** (`active` + `postgres`) — `CrudService<M>`, `FilterBuilder<M>`, `SortBuilder<M>`, `BatchService<M>` in `rok_fluent::services`.
- **Remaining services** — `SoftDeleteService<M>`, `SearchService<M>` (ILIKE + full-text), `AuditService<M>`, `BatchService::bulk_update`.
- **Active Record ↔ DSL bridge** — `ModelQuery::and_expr(expr)`, `or_expr(expr)`, `into_dsl()` — gated behind `active` + `query` features.
- **DX improvements** — `.inspect()` / `.explain()` / `.explain_json()` on all DSL builders; `#[table(searchable)]` field attribute.
- **Performance** — `SqlValue::Array` + `Column::eq_any()`; `SelectBuilder::stream()` yielding rows without full buffer; `BatchService::copy_insert()` via PostgreSQL `COPY FROM STDIN` (10–50× faster); `pool::warm()` for pre-opening connections.
- **`rok db` CLI** — `[[bin]]` target gated behind `cli` feature. Commands: `migrate`, `rollback`, `status`, `make`, `seed`, `schema dump`, `schema diff`.
- **CI consolidation** — merged `ci.yml` + `publish.yml` into single workflow; uses `Swatinem/rust-cache@v2`.

### Changed
- `#[derive(Table)]` now generates `table_name()` + `name()` on `impl Table`.

---

## [0.4.0] — 2026-05-21

First crates.io release of the consolidated crate.

### Added
- **Typed query DSL** (`feature = "query"`) — `db::select().from(users::table).where_(users::id.eq(1_i64))`.
  Full `SelectBuilder`, `InsertBuilder`, `UpdateBuilder`, `DeleteBuilder` with PostgreSQL async terminals.
- **`#[derive(Table)]`** — generates a companion `pub mod users { pub const table: …; pub const id: Column<…>; … }` for DSL queries.
- **`active` feature flag** — gates Active Record style (`ModelQuery`, `PgModel`, `MorphTo*`, `ThroughQuery`) independently from backend features.
- **`query` feature flag** — gates the typed DSL independently from `active`.
- **`#[model(...)]` attribute alias** — cleaner alternative to `#[rok_orm(...)]`. Supports `table`, `pk`, `timestamps`, `soft_delete`, `fillable`, `guarded` on structs and `skip`, `pk`, `column` on fields.
- **`SqlValue::Json(serde_json::Value)`** — binds as `jsonb` on PostgreSQL, serialised text on SQLite/MySQL.
- **`SqlValue::Uuid(uuid::Uuid)`** — binds as native `UUID` on PostgreSQL, `CHAR(36)` text elsewhere.
- **`ModelQuery::first_or_default()`** — returns `M::default()` when no row matches.
- **`ModelQuery::first_or_else(|| ...)`** — calls closure when no row matches.
- **`pool::ping(&pool) -> bool`** — runs `SELECT 1`, suitable for health-check endpoints.
- **CI/CD** — GitHub Actions workflows: `ci.yml` (test on every push/PR), `publish.yml` (auto-publish to crates.io when `Cargo.toml` version bumps).
- **README.md** — feature table, DSL and Active Record quick-starts, migration example.

### Changed
- **Breaking:** consolidated `rok-orm`, `rok-orm-core`, `rok-orm-macros`, `rok-orm-factory`,
  `rok-orm-migrate` into this single `rok-fluent` crate.
- All `rok_orm::` / `rok_orm_core::` import paths become `rok_fluent::`.
- `rok-orm-macros` is now `rok-fluent-macros` (internal; users never import it directly).
- SQLx bind helpers now handle `SqlValue::Json` and `SqlValue::Uuid`.
- Feature flags renamed/consolidated (see [features.md](features.md)).

### Migration from `rok-orm` 0.3

```toml
# Before
[dependencies]
rok-orm         = { version = "0.3", features = ["postgres", "macros"] }
rok-orm-migrate = { version = "0.3", features = ["postgres"] }
rok-orm-factory = { version = "0.3", features = ["postgres"] }

# After
[dependencies]
rok-fluent = { version = "0.4", features = ["postgres", "macros", "migrate-postgres", "factory-postgres"] }
```

```rust
// Before
use rok_orm::{Model, QueryBuilder};
use rok_orm_core::SqlValue;
use rok_orm_migrate::MigrationRunner;

// After
use rok_fluent::{Model, QueryBuilder, SqlValue};
use rok_fluent::migrate::MigrationRunner;
```

---

## [0.3.x] — rok-orm era

Legacy multi-crate releases. See individual crate changelogs on crates.io:
- `rok-orm`
- `rok-orm-core`
- `rok-orm-macros`
- `rok-orm-factory`
- `rok-orm-migrate`
