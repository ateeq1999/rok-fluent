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

## Phase 33 — Active Record ↔ DSL Bridge (approved 2026-05-21)

- [ ] `ModelQuery::and_expr(expr: Expr)` — inject a typed `Expr` into an Active Record chain
- [ ] `ModelQuery::or_expr(expr: Expr)` — OR variant
- [ ] `ModelQuery::into_dsl() -> SelectBuilder` — convert to a DSL `SelectBuilder`
- [ ] Feature-gate: only available when both `active` + `query` features are enabled
- [ ] Tests: round-trip AR → DSL produces identical SQL
- [ ] Update `docs/guides/active-record.md` with bridge examples

---

## Phase 34 — Developer Experience (approved 2026-05-21)

- [ ] `.inspect()` on `SelectBuilder`, `InsertBuilder`, `UpdateBuilder`, `DeleteBuilder`
  - [ ] Prints rendered SQL + bound params to `stderr` (behind `tracing` feature: emits a span)
  - [ ] Returns `self` unchanged (builder-chain transparent)
- [ ] `.explain(&pool) -> Result<String, sqlx::Error>` on `SelectBuilder` (PostgreSQL only)
- [ ] `.explain_json(&pool) -> Result<serde_json::Value, sqlx::Error>` — structured plan
- [ ] `#[table(searchable)]` field attribute — marks column for `SearchService` index
- [ ] `#[table(rename_all = "camelCase|snake_case|PascalCase")]` — rename column constants
- [ ] Better proc-macro errors — `compile_error!` pointing to the offending `#[table(...)]` attribute
- [ ] Update `docs/api/core.md`, `docs/guides/debugging.md`

---

## Phase 35 — Performance & Scalability (approved 2026-05-21)

- [ ] `SqlValue::Array(Vec<SqlValue>)` — binds as PostgreSQL array; enables `= ANY($1)` queries
  - [ ] `Column::eq_any(vals)` — renders `col = ANY($N)`
  - [ ] Update bind helpers in `src/core/sqlx/pg.rs`
- [ ] `SelectBuilder::stream(&pool) -> impl Stream<Item = Result<T>>` — streaming without buffering
  - [ ] Uses `sqlx::query_as(...).fetch(&pool)` internally
  - [ ] Gate behind `futures` dep (already a transitive dep)
- [ ] PostgreSQL `COPY FROM STDIN` path for `bulk_insert` (10–50× faster for large batches)
  - [ ] `BatchService::copy_insert(rows, pool)` — uses `sqlx::PgCopyIn`
  - [ ] Fallback to multi-row `INSERT` on non-PG backends
- [ ] Prepared statement cache — cache compiled `QueryBuilder` output by SQL hash
- [ ] `pool::warm(n, pool)` — pre-open `n` connections at startup
- [ ] Update `docs/api/orm.md`, `docs/guides/performance.md`

---

## Phase 36 — `rok db` CLI (approved 2026-05-21)

- [ ] Add `cli` feature to `Cargo.toml` (pulls in `clap`)
- [ ] `[[bin]]` target `rok` in `Cargo.toml` or separate `rok-fluent-cli` crate
- [ ] `rok db migrate` — runs pending migrations via `MigrationRunner`
- [ ] `rok db rollback` — rolls back the last migration
- [ ] `rok db status` — prints applied / pending migration list
- [ ] `rok db make <name>` — scaffolds a timestamped `YYYYMMDDHHMMSS_<name>.sql` file
- [ ] `rok db seed` — runs all `#[derive(Seed)]` seeders
- [ ] `rok db schema dump` — introspects live DB and emits `CREATE TABLE` DDL
- [ ] `rok db schema diff` — compares live DB to migration history
- [ ] Update `docs/guides/migrations.md`, `docs/api/migrate.md`

---

## Phase 37 — New Services: Transactions, Locking, Schema Inspection (approved 2026-05-21)

- [ ] `src/services/transaction.rs` — `TransactionService`
  - [ ] `TransactionService::run(&pool, |tx| async { … })` — closure-based transaction
  - [ ] `tx.savepoint("name")`, `tx.rollback_to("name")`, `tx.release("name")`
  - [ ] `tx.create::<M>()`, `tx.update::<M>()`, `tx.delete::<M>()` pool-free CRUD on a `&mut PgTransaction`
- [ ] `src/services/lock.rs` — `LockService`
  - [ ] `SelectBuilder::lock(Lock::ForUpdate | ForShare | SkipLocked | NoWait)` — row-level locking
  - [ ] `LockService::acquire(key, pool)` — PostgreSQL `pg_advisory_lock`
  - [ ] `LockService::try_acquire(key, pool) -> bool` — non-blocking `pg_try_advisory_lock`
  - [ ] `LockService::release(key, pool)` — `pg_advisory_unlock`
- [ ] `src/services/schema_inspector.rs` — `SchemaInspector`
  - [ ] `SchemaInspector::columns(table, pool)` → `Vec<ColumnInfo>`
  - [ ] `SchemaInspector::indexes(table, pool)` → `Vec<IndexInfo>`
  - [ ] `SchemaInspector::foreign_keys(table, pool)` → `Vec<ForeignKeyInfo>`
  - [ ] Used internally by `rok db schema dump`
- [ ] `SelectBuilder::distinct_on(cols)` — PostgreSQL `SELECT DISTINCT ON (col, …)`
- [ ] Window function support: `Column::rank()`, `row_number()`, `lag(n)`, `lead(n)` + `Window` builder
- [ ] `TypedJson<T>` column wrapper — deserializes `jsonb` directly into a typed struct
- [ ] Update `docs/api/orm.md`, `docs/guides/transactions.md`, `docs/guides/locking.md`

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
