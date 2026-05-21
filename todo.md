# rok-fluent — Task List

See `plan.md` for architecture decisions, feature-flag taxonomy, and design rationale.

---

## ✅ Phase 0–12 — Consolidation (COMPLETE as of 2026-05-21)

All five original crates (`rok-orm`, `rok-orm-core`, `rok-orm-macros`, `rok-orm-factory`,
`rok-orm-migrate`) have been absorbed into `src/`. Old crate directories deleted.
Feature-matrix spot-check passes for all feature combinations.

---

## Phase 13 — Tombstone Releases (deferred — publish after v0.4.0 stabilises)

- [ ] Publish `rok-fluent` to crates.io

---

## Phase 14 — Docs & README (in progress)

- [ ] Write `README.md` at repo root: crate name, feature table, quick-start, migration guide
- [ ] Verify `cargo doc --features full --open` renders cleanly
- [ ] Update `docs/changelog.md` with v0.4.0 entry

---

## ✅ Phase 15 — Feature Flag Modernisation + `active` Style Gate (COMPLETE)

- [x] Add `active` feature to `Cargo.toml`
- [x] Add `query` feature to `Cargo.toml`
- [x] Update `full` bundle to include `active` + `query`
- [x] Gate Active Record modules behind `feature = "active"`
- [x] Update `src/lib.rs` re-exports
- [x] All feature-matrix checks pass

---

## ✅ Phase 16 — `#[model(...)]` Ergonomic Attribute Alias (COMPLETE)

- [x] `#[model(table="...", pk="...", timestamps, soft_delete, fillable="...", guarded="...")]`
      on structs (alongside existing `#[rok_orm(...)]` — both namespaces accepted)
- [x] `#[model(skip)]`, `#[model(pk)]`, `#[model(column="...")]` on fields

---

## ✅ Phase 17 — `query` DSL (Drizzle-style typed query builder) (COMPLETE)

### 17a–17c — Core types and `#[derive(Table)]` macro
- [x] `src/dsl/column.rs` — `Column<T,V>` with comparison/ordering operators
- [x] `src/dsl/table.rs` — `Table` trait
- [x] `src/dsl/expr.rs` — `Expr` composable boolean tree, `$N`/`?` rendering
- [x] `src/dsl/select.rs` — `SelectBuilder`
- [x] `src/dsl/insert.rs` — `InsertBuilder` with `.returning()`
- [x] `src/dsl/update.rs` — `UpdateBuilder`
- [x] `src/dsl/delete.rs` — `DeleteBuilder`
- [x] `src/dsl/db.rs` — `select()`, `insert_into()`, `update()`, `delete_from()` entry-points
- [x] `#[derive(Table)]` in `rok-fluent-macros` — generates typed DSL companion module

### 17d — PostgreSQL executor integration
- [x] `SelectBuilder::fetch_all/fetch_one/fetch_optional/exists/count(&pool)`
- [x] `InsertBuilder::execute/returning::fetch_one(&pool)`
- [x] `UpdateBuilder::execute(&pool)`
- [x] `DeleteBuilder::execute(&pool)`

---

## ✅ Phase 18 — DX Improvements (PARTIAL — high-priority items done)

- [x] `exists()` on `ModelQuery` / `SelectBuilder`
- [x] `first_or_default()` / `first_or_else(|| ...)` on `ModelQuery`
- [x] Cursor-based pagination: `cursor_paginate()` on `ModelQuery`
- [x] `pool::ping(&pool) -> bool` health-check
- [x] Named pool registry: `pool::register_named_pool` / `get_named_pool`
- [ ] `QueryEvent` hook: `on_query(fn(sql, params, duration_ms, table_name))`
- [ ] `EXPLAIN` helper: `query.explain(&pool)` → `String`

---

## ✅ Phase 19 — `SqlValue` Completeness (PARTIAL)

- [x] Add `SqlValue::Json(serde_json::Value)` — binds as `jsonb` on PG, text on SQLite/MySQL
- [x] Add `SqlValue::Uuid(uuid::Uuid)` — binds natively on PG, as CHAR(36) elsewhere
- [x] Update all bind helpers in `src/core/sqlx/pg.rs`, `sqlite.rs`, `mysql.rs`
- [ ] Add `SqlValue::Array(Vec<SqlValue>)` for `= ANY($1)` style PG queries

---

## Phase 20 — `rok db` CLI binary (optional convenience tool)

- [ ] Add `[[bin]]` target or separate `rok-fluent-cli` crate
- [ ] `rok db migrate` → `MigrationRunner::run()`
- [ ] `rok db rollback` → `MigrationRunner::rollback()`
- [ ] `rok db status` → `MigrationRunner::status()`
- [ ] `rok db make <name>` → scaffold a timestamped migration file

## ✅ Phase 21 — OOP Primary API: `User::table()` + `User::ID` (COMPLETE as of 2026-05-21)

- [x] `#[derive(Table)]` generates `UserTable` struct + `impl Table for UserTable`
- [x] `User::table() -> UserTable` factory method
- [x] `User::ID`, `User::NAME`, … SCREAMING_SNAKE_CASE `Column<T,V>` constants
- [x] Module alias `pub mod users { table, id, name, … }` (secondary)
- [x] `heck::ToShoutySnakeCase` used for constant naming

## ✅ Phase 22 — DSL JOIN Builder (COMPLETE as of 2026-05-21)

- [x] `Expr::ColEq`, `Expr::ILike`, `Expr::Between`, `Expr::NotBetween`, `Expr::Raw`
- [x] `Column::references()` / `Column::eq_col()` → `Expr::ColEq`
- [x] `SelectBuilder::inner_join()`, `left_join()`, `right_join()`, `cross_join()`
- [x] `SelectBuilder::or_where()`
- [x] `Table` trait: static `table_name()` + instance `name()` for join builders
- [x] Macro generates both methods in `impl Table for UserTable`

## ✅ Phase 23 — DSL Pagination on SelectBuilder (COMPLETE as of 2026-05-21)

- [x] `SelectBuilder::paginate(page, per_page, pool)` → `Page<T>` (offset + count)
- [x] `SelectBuilder::simple_paginate(page, per_page, pool)` → `SimplePage<T>`
- [x] `SelectBuilder::cursor_paginate(cursor_col, cursor, per_page, pool)` → `CursorPage<T>`
- [x] Cursor value extracted from raw `PgRow` before `T` deserialisation
- [x] `SelectBuilder::count(pool)` / `exists(pool)` terminals

## ✅ Phase 24 — DSL Aggregators (COMPLETE as of 2026-05-21)

- [x] `AggExpr` struct with `alias()`, comparison operators for HAVING
- [x] `Column::count()`, `count_distinct()`, `sum()`, `avg()`, `min()`, `max()`
- [x] `SelectBuilder::group_by()`, `having()`, `agg_col()`
- [x] `Expr::AggCmp` variant rendered in `to_sql_pg` / `to_sql_qmark`
- [x] Re-exported: `AggExpr`, `OrderExpr`, `NullsOrder`, `OrderDir`, `Join`, `JoinKind`

## ✅ Phase 25 — `Loaded<T>` Relationship Carrier (COMPLETE as of 2026-05-21)

- [x] `src/dsl/loaded.rs`: `Loaded<T>` enum (`NotLoaded` / `Some(T)`)
- [x] `Default`, `Serialize` (null when not loaded), `Deserialize` (None → NotLoaded)
- [x] `is_loaded()`, `as_option()`, `into_option()`, `unwrap()`, `map()`
- [x] Re-exported from `dsl::Loaded`

## ✅ Phase 29 — Advanced `Expr`: CASE, EXISTS, Subqueries, Column Functions (COMPLETE as of 2026-05-21)

- [x] `Expr::Exists(sql)`, `Expr::NotExists(sql)` — `EXISTS (subquery)`
- [x] `Expr::InSubquery(col, sql)`, `Expr::NotInSubquery(col, sql)`
- [x] `Expr::exists()`, `Expr::not_exists()` constructors
- [x] `Expr::case()` → `CaseExpr` builder with `.when(cond, val).otherwise(val)`
- [x] `Column::in_subquery()`, `Column::not_in_subquery()`
- [x] `Column::lower()`, `upper()`, `length()`, `trim()`, `coalesce()`, `cast_as()`, `date_trunc()`, `extract()` → `FnExpr`
- [x] `FnExpr` with `.alias()`, `.eq()`, `.ne()`, `.like()`, `.ilike()`, `.gt()`, `.lt()`
- [x] Re-exported: `CaseExpr`, `FnExpr`

## ✅ Phase 31 — Upsert + RETURNING on all DSL builders (COMPLETE as of 2026-05-21)

- [x] `InsertBuilder::values_typed()` — typed `Column<T,V>` pairs
- [x] `InsertBuilder::on_conflict_do_nothing()`
- [x] `InsertBuilder::on_conflict(cols)` + `.do_update_excluded(cols)` + `.do_update_values(pairs)`
- [x] `InsertBuilder::returning()` / `returning_cols(cols)`
- [x] `InsertBuilder::fetch_one::<T>()` / `fetch_all::<T>()` (PostgreSQL, auto-adds RETURNING)
- [x] `UpdateBuilder::set_col()` / `set_typed()` — typed column setters
- [x] `UpdateBuilder::returning()` / `returning_cols(cols)`
- [x] `UpdateBuilder::fetch_one::<T>()` / `fetch_all::<T>()` (PostgreSQL)

## ✅ Phase 30 — Subqueries, CTEs, Set Operations (COMPLETE as of 2026-05-21)

- [x] `SelectBuilder::from_subquery(query, alias)` — `SELECT … FROM (…) AS alias`
- [x] `SelectBuilder::with_cte(name, query)` — `WITH name AS (…)`
- [x] `SelectBuilder::from_cte(name)` — select from a named CTE
- [x] CTE prefix rendered before the SELECT in `to_sql_pg()`
- [x] `SelectBuilder::union()`, `union_all()`, `intersect()`, `except()` — set operations
- [x] Set ops appended after LIMIT/OFFSET in rendered SQL

## ✅ Phase 32 — Service Layer (COMPLETE as of 2026-05-21)

- [x] `src/services/crud.rs` — `CrudService<M>`: pool-owning CRUD wrapper
  - [x] `all()`, `find()`, `find_or_fail()`, `count()`, `exists()`, `query()`, `filter()`
  - [x] `paginate()`, `simple_paginate()`, `cursor_paginate()` via `pool::with_pool`
  - [x] `create()`, `update()`, `delete()`, `soft_delete()`, `restore()`
  - [x] `bulk_create()`, `delete_where()`, `upsert_by()`
- [x] `src/services/filter.rs` — `FilterBuilder<M>`: composable WHERE clause builder
  - [x] `eq`, `ne`, `gt`, `gte`, `lt`, `lte`, `like`, `ilike`, `is_null`, `is_not_null`, `in_`, `not_in`
  - [x] `apply(query) -> ModelQuery<M>`
- [x] `src/services/sort.rs` — `SortBuilder<M>`: whitelist-validated sort builder
  - [x] `allow()`, `apply_user_input()`, `then_by()`, `apply(query)`
- [x] `src/services/batch.rs` — `BatchService<M>`: bulk operations
  - [x] `bulk_insert()`, `bulk_insert_chunked()`, `bulk_upsert_by()`, `delete_where()`
- [x] `src/services/mod.rs` — re-exports all four types
- [x] `src/lib.rs` — `pub mod services` added

---

## ✅ Phase 32b — Remaining Service Layer (COMPLETE as of 2026-05-21)

- [x] `src/services/soft_delete.rs` — `SoftDeleteService<M>`
  - [x] `all_active(pool)`, `all_deleted(pool)`, `with_trashed(pool)`
  - [x] `soft_delete(id, pool)`, `restore(id, pool)`, `force_delete(id, pool)`, `purge_deleted(pool)`
- [x] `src/services/search.rs` — `SearchService<M>`
  - [x] ILIKE OR-chain: `search()`, `search_paginated()`, `search_simple_paginated()`
  - [x] PostgreSQL full-text: `fts()` via `to_tsvector` / `plainto_tsquery`
- [x] `src/services/audit.rs` — `AuditService<M>`
  - [x] `touch(id, pool)` — delegates to `M::touch_by_pk`
  - [x] `history(id, pool)` — reads from `audit_log` table; returns `[]` if table absent
- [x] `BatchService::bulk_update(data, ids, pool)` — update rows matching a list of PKs
- [x] `CrudService::search(term, cols)` + `search_paginated(term, cols, page, per)` — delegates to `SearchService`
- [ ] `CrudService::all_with(relations)` + `paginate_with(page, per, relations)` (post Phase 25)
- [x] Update `src/services/mod.rs` re-exports
- [x] Update `docs/api/orm.md` with all new service types

---

## ✅ Phase 33 — Active Record ↔ DSL Bridge (COMPLETE as of 2026-05-21)

- [x] `ModelQuery::and_expr(expr: Expr)` — inject a typed `Expr` into an Active Record chain
- [x] `ModelQuery::or_expr(expr: Expr)` — OR variant
- [x] `ModelQuery::into_dsl() -> SelectBuilder` — convert to a DSL `SelectBuilder`
- [x] `src/orm/bridge.rs` — `expr_to_condition()`, `condition_to_expr()`, `model_query_into_select()`
- [x] Feature-gate: only available when both `active` + `query` features are enabled
- [x] Tests: 7 unit tests covering eq/and/or/in roundtrips and into_dsl table/condition/pagination preservation
- [x] Update `docs/api/orm.md` with bridge examples

---

## ✅ Phase 34 — Developer Experience (COMPLETE as of 2026-05-21)

- [x] `.inspect()` on `SelectBuilder`, `InsertBuilder`, `UpdateBuilder`, `DeleteBuilder`
  - [x] Prints rendered SQL + bound params to `stderr` (behind `tracing` feature: emits a span)
  - [x] Returns `self` unchanged (builder-chain transparent)
- [x] `.explain(&pool) -> Result<String, sqlx::Error>` on `SelectBuilder` (PostgreSQL only)
- [x] `.explain_json(&pool) -> Result<serde_json::Value, sqlx::Error>` — structured plan
- [x] `#[table(searchable)]` field attribute — marks column for `SearchService` index
  - [x] `Model::searchable_columns()` trait method (defaults to `&[]`)
  - [x] `#[derive(Model)]` generates `searchable_columns()` from `#[table(searchable)]` fields
  - [x] `SearchService` falls back to `M::searchable_columns()` when `cols` arg is empty
  - [x] `expand_table` no longer skips searchable fields from column generation
- [ ] `#[table(rename_all = "camelCase|snake_case|PascalCase")]` — rename column constants
- [ ] Better proc-macro errors — `compile_error!` pointing to the offending `#[table(...)]` attribute
- [x] Update `docs/api/core.md`, `docs/guides/debugging.md`

---

## ✅ Phase 35 — Performance & Scalability (COMPLETE as of 2026-05-21)

- [x] `SqlValue::Array(Vec<SqlValue>)` — binds as PostgreSQL array; enables `= ANY($1)` queries
  - [x] `Column::eq_any(vals)` / `QueryBuilder::where_eq_any` — renders `col = ANY(ARRAY[$1, …])`
  - [x] `Expr::EqAny` / `Condition::EqAny` in expression tree and condition tree
  - [x] `From<Vec<i64>>`, `From<Vec<String>>`, `From<Vec<&str>>` for `SqlValue`
  - [x] PG bind: `Vec<i64>` / `Vec<f64>` / `Vec<bool>` / `Vec<String>` by first-element sniff
  - [x] SQLite / MySQL: fall back to `IN (…)` rendering; bind as text literal
- [x] `SelectBuilder::stream(&pool) -> impl Stream<Item = Result<T>>` — yields rows without full buffer
  - [x] Implemented via `futures::stream::once` + `flat_map` (SQL owned by future, no lifetime extension needed)
  - [x] Uses `futures` dep (already enabled with `postgres` feature)
- [x] PostgreSQL `COPY FROM STDIN` for large batch inserts (10–50× faster)
  - [x] `BatchService::copy_insert(rows, pool)` — uses `sqlx::PgPoolCopyExt::copy_in_raw`
  - [x] CSV serialization with proper quoting for Text / Json / Array values
- [x] `pool::warm(n, pool)` — pre-open `n` connections concurrently at startup
- [x] Update `docs/api/orm.md`, `docs/guides/performance.md`
- Note: Prepared-statement string cache deferred — sqlx already caches at protocol level

---

## ✅ Phase 36 — `rok db` CLI (COMPLETE as of 2026-05-21)

- [x] Add `cli` feature to `Cargo.toml` (pulls in `clap`)
- [x] `[[bin]]` target `rok` in `Cargo.toml` with `required-features = ["cli"]`
- [x] `rok db migrate` — runs pending migrations via `MigrationRunner`
- [x] `rok db rollback` — rolls back the last migration batch
- [x] `rok db status` — prints Applied / Pending migration list
- [x] `rok db make <name>` — scaffolds a timestamped `YYYYMMDDHHMMSS_<name>.sql` file
- [x] `rok db seed` — prints guidance (seeders registered programmatically)
- [x] `rok db schema dump` — queries `information_schema`, emits approximate `CREATE TABLE` DDL
- [x] `rok db schema diff` — compares files in `--dir` to `_migrations` table (Applied/Pending/Orphan)
- [x] Update `docs/guides/migrations.md` (rok db section), `docs/features.md` (cli section)

---

## ✅ Phase 37 — New Services: Transactions, Locking, Schema Inspection (COMPLETE 2026-05-21)

- [x] `src/services/transaction.rs` — `TransactionService`
  - [x] `TransactionService::begin(&pool) -> TxCtx`
  - [x] `TxCtx::savepoint(name)`, `rollback_to(name)`, `release(name)`
  - [x] `TxCtx::create/update/delete/fetch_all/fetch_optional` — pool-free CRUD on `&mut Transaction`
- [x] `src/services/lock.rs` — `LockService`
  - [x] `SelectBuilder::lock(Lock::ForUpdate | ForNoKeyUpdate | ForShare | ForKeyShare)` — row-level locking
  - [x] `SelectBuilder::lock_conflict(LockConflict::SkipLocked | NoWait)`
  - [x] `LockService::acquire(key, pool)` — PostgreSQL `pg_advisory_lock`
  - [x] `LockService::try_acquire(key, pool) -> bool` — non-blocking `pg_try_advisory_lock`
  - [x] `LockService::release(key, pool)` — `pg_advisory_unlock`
  - [x] `LockService::acquire_xact/try_acquire_xact` — transaction-scoped advisory locks
  - [x] `LockService::acquire_timeout(key, timeout, pool)` — `set_lock_timeout` wrapper
- [x] `src/services/schema_inspector.rs` — `SchemaInspector`
  - [x] `SchemaInspector::columns(table, pool)` → `Vec<ColumnInfo>`
  - [x] `SchemaInspector::indexes(table, pool)` → `Vec<IndexInfo>`
  - [x] `SchemaInspector::foreign_keys(table, pool)` → `Vec<ForeignKeyInfo>`
  - [x] Used internally by `rok db schema dump`
- [x] `SelectBuilder::distinct_on(cols)` — PostgreSQL `SELECT DISTINCT ON (col, …)`
- [x] Window function support: `rank()`, `row_number()`, `dense_rank()`, `ntile(n)` free functions; `Column::lag(n)`, `lead(n)`, `first_value()`, `last_value()`; `Window` builder with `partition_by`/`order_by`; `WinExpr` with `.over(w)`/`.alias(n)`; `SelectBuilder::win_col()`
- [x] `TypedJson<T>` column wrapper — deserializes `jsonb` directly into a typed struct via `sqlx::types::Json<T>`; `From<TypedJson<T>> for SqlValue`
- [x] Update `docs/api/orm.md`, `docs/guides/transactions.md`, `docs/guides/locking.md` — add `TransactionService`, `LockService`, `SchemaInspector`, window functions, `TypedJson<T>`

---

## Phase 38 — Ecosystem & Publish Quality (approved 2026-05-21)

- [ ] publish `rok-fluent`
- [ ] `docs.rs` feature metadata: `all-features = true` + `rustdoc-args = ["--cfg", "docsrs"]`
- [ ] MSRV policy: declare `rust-version` in `Cargo.toml`, test against stable - 2 in CI
- [ ] `cargo-fuzz` targets: SQL rendering correctness for `SelectBuilder`, `InsertBuilder`
- [ ] `criterion` benchmarks: query build time, bind time, vs raw `sqlx`
- [ ] `QueryLog` structured sink: `on_query(fn(QueryEvent))` behind `tracing` feature
  - [ ] `QueryEvent { sql, params, duration_ms, table, rows_affected }`
  - [ ] OpenTelemetry span emitted automatically when `tracing` feature is on
