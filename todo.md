# rok-fluent — Task List

See `plan.md` for architecture decisions, feature-flag taxonomy, and design rationale.

---

## ✅ Phase 0–12 — Consolidation (COMPLETE as of 2026-05-21)

All five original crates (`rok-orm`, `rok-orm-core`, `rok-orm-macros`, `rok-orm-factory`,
`rok-orm-migrate`) have been absorbed into `src/`. Old crate directories deleted.
Feature-matrix spot-check passes for all feature combinations.

---

## Phase 13 — Tombstone Releases (deferred — publish after v0.4.0 stabilises)

- [ ] Publish `rok-orm@0.3.99` with `deprecated = true` + `rok-fluent` migration note
- [ ] Publish tombstones for `rok-orm-core`, `rok-orm-factory`, `rok-orm-migrate`

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
