# rok-fluent — Task List

See `plan.md` for architecture decisions, feature-flag taxonomy, and design rationale.

---

## ✅ Phase 0–12 — Consolidation (COMPLETE as of 2026-05-21)

All five original crates (`rok-orm`, `rok-orm-core`, `rok-orm-macros`, `rok-orm-factory`,
`rok-orm-migrate`) have been absorbed into `src/`. Old crate directories deleted.
Feature-matrix spot-check passes for all feature combinations.

---

## ✅ Phase 13 — Tombstone Releases (COMPLETE as of 2026-05-21)

- [x] Publish `rok-fluent` and `rok-fluent-macros` v0.4.1 to crates.io

---

## ✅ Phase 14 — Docs & README (COMPLETE as of 2026-05-21)

- [x] Write `README.md` at repo root: feature table, quick-start (DSL + AR + Service + Transactions), migration guide
- [x] Verify `cargo doc --features full` renders cleanly (no warnings)
- [x] Update `docs/changelog.md` with v0.4.1 entry covering all Phases 37–38 work
- [x] Fix `docs/index.md` stale "(planned)" markers on transactions/locking guides

---

## ✅ Phase 15 — Feature Flag Modernisation + `active` Style Gate (COMPLETE)

---

## ✅ Phase 16 — `#[model(...)]` Ergonomic Attribute Alias (COMPLETE)

---

## ✅ Phase 17 — `query` DSL (Drizzle-style typed query builder) (COMPLETE)

---

## ✅ Phase 18 — DX Improvements (COMPLETE as of 2026-05-21)

- [x] `exists()` on `ModelQuery` / `SelectBuilder`
- [x] `first_or_default()` / `first_or_else(|| ...)` on `ModelQuery`
- [x] Cursor-based pagination: `cursor_paginate()` on `ModelQuery`
- [x] `pool::ping(&pool) -> bool` health-check
- [x] Named pool registry: `pool::register_named_pool` / `get_named_pool`
- [x] `QueryEvent` hook: `set_on_query(fn(QueryEvent))` + `clear_on_query()` (fires on every query)
- [x] `EXPLAIN` helper: `SelectBuilder::explain(&pool) -> String` + `explain_json` (Phase 34)

---

## ✅ Phase 19 — `SqlValue` Completeness (COMPLETE as of 2026-05-21)

- [x] `SqlValue::Json(serde_json::Value)` — binds as `jsonb` on PG, text on SQLite/MySQL
- [x] `SqlValue::Uuid(uuid::Uuid)` — binds natively on PG, as CHAR(36) elsewhere
- [x] `SqlValue::Array(Vec<SqlValue>)` for `= ANY($1)` style PG queries (Phase 35)
- [x] Update all bind helpers in `src/core/sqlx/pg.rs`, `sqlite.rs`, `mysql.rs`

---

## ✅ Phase 20 — `rok db` CLI binary (COMPLETE as of 2026-05-21 — merged into Phase 36)

---

## ✅ Phase 21 — OOP Primary API: `User::table()` + `User::ID` (COMPLETE)

---

## ✅ Phase 22 — DSL JOIN Builder (COMPLETE)

---

## ✅ Phase 23 — DSL Pagination on SelectBuilder (COMPLETE)

---

## ✅ Phase 24 — DSL Aggregators (COMPLETE)

---

## ✅ Phase 25 — `Loaded<T>` Relationship Carrier (COMPLETE)

---

## ✅ Phase 29 — Advanced `Expr`: CASE, EXISTS, Subqueries, Column Functions (COMPLETE)

---

## ✅ Phase 30 — Subqueries, CTEs, Set Operations (COMPLETE)

---

## ✅ Phase 31 — Upsert + RETURNING on all DSL builders (COMPLETE)

---

## ✅ Phase 32 — Service Layer (COMPLETE as of 2026-05-21)

---

## ✅ Phase 32b — Remaining Service Layer (COMPLETE as of 2026-05-21)

- [x] All prior items plus:
- [x] `CrudService::all_with(relations)` — eagerly load relations on all rows
- [x] `CrudService::paginate_with(page, per, relations)` — paginate with eager loading

---

## ✅ Phase 33 — Active Record ↔ DSL Bridge (COMPLETE)

---

## ✅ Phase 34 — Developer Experience (COMPLETE as of 2026-05-21)

- [x] `.inspect()` / `.explain()` / `.explain_json()` on all DSL builders
- [x] `#[table(searchable)]` field attribute + `SearchService` integration
- [x] `#[table(rename_all = "camelCase" | "snake_case" | "PascalCase")]` — auto-transforms column names from field names. 4 unit tests.
- [x] Better proc-macro errors — every unknown attribute error now includes the actual attribute name (e.g. `` unknown #[table(...)] struct attribute `foobar` ``)

---

## ✅ Phase 35 — Performance & Scalability (COMPLETE as of 2026-05-21)

- [x] `SqlValue::Array` + `eq_any()`; `SelectBuilder::stream()`; `BatchService::copy_insert()`; `pool::warm()`

---

## ✅ Phase 36 — `rok db` CLI (COMPLETE as of 2026-05-21)

- [x] All CLI commands: migrate, rollback, status, make, seed, schema dump, schema diff

---

## ✅ Phase 37 — New Services: Transactions, Locking, Schema Inspection (COMPLETE 2026-05-21)

- [x] `TransactionService`, `LockService`, `SchemaInspector`, window functions, `TypedJson<T>`, `distinct_on`

---

## ✅ Phase 38 — Ecosystem & Publish Quality (COMPLETE as of 2026-05-21)

- [x] `docs.rs` metadata: `all-features = true` + `#[cfg_attr(docsrs, feature(doc_auto_cfg))]`
- [x] MSRV policy: `rust-version = "1.85"` in both Cargo.tomls; CI validates against 1.85.0
- [x] `cargo-fuzz` targets: `sql_render_pg` + `sql_render_qmark` (compile on nightly)
- [x] `criterion` benchmarks: query build time (7 benchmarks, 143ns–2.17µs range)
- [x] `QueryLog` structured sink: `QueryEvent { sql, params, duration_ms, table, rows_affected }`; `tracing::trace!` span when `tracing` feature is on; `set_on_query(fn(QueryEvent))` fires on every query; `set_logger(fn(QueryEvent))` fires on slow queries (threshold configurable)
- [x] Publish `rok-fluent-macros` + `rok-fluent` v0.4.1 to crates.io
