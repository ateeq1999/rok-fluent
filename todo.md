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

## Phase 15 — Feature Flag Modernisation + `active` Style Gate

**Goal:** make the Active Record style opt-in via `active` feature; scaffold the `query`
(Drizzle-style typed DSL) behind its own flag.

- [x] Add `active` feature to `Cargo.toml` (gates `PgModel`, `MysqlModel`, `SqliteModel`,
      `ModelQuery`, `PivotQuery`, `MorphTo*`, `ThroughQuery`)
- [x] Add `query` feature to `Cargo.toml` (new Drizzle-style DSL, see Phase 17)
- [x] Update `full` bundle to include `active` + `query`
- [x] Gate `src/orm/model_query.rs`, `src/orm/morph.rs`, `src/orm/through.rs`,
      `src/orm/postgres/model.rs`, `src/orm/postgres/pivot_query.rs`,
      `src/orm/mysql/model.rs`, `src/orm/sqlite/model.rs`
      behind `feature = "active"` (in addition to their backend feature)
- [x] Update `src/lib.rs` re-exports to use `active` gate where appropriate
- [x] Verify all feature-matrix checks pass

---

## Phase 16 — `#[derive(Model)]` Auto-impl

**Goal:** zero-boilerplate Model derivation — no more manual `table_name`, `columns`, `pk`.

- [ ] Extend `rok-fluent-macros` `Model` derive to auto-generate:
  - `table_name()` → snake_case pluralised struct name (e.g. `User` → `"users"`)
  - `columns()` → `&["field1", "field2", ...]` from all non-skipped fields
  - `primary_key()` → `"id"` by default, overridable via `#[model(pk = "...")]`
  - `pk_value()` → reads the pk field via `serde_json::to_value`
- [ ] Support attributes: `#[model(table = "...")]`, `#[model(pk = "...")]`, `#[model(skip)]`
- [ ] Add `#[model(timestamps)]` — marks `created_at`/`updated_at` for `touch()` support
- [ ] Update `src/core/model.rs` to document which methods are auto-derived
- [ ] Add doc examples showing zero-boilerplate derivation
- [ ] Run full test suite

---

## Phase 17 — `query` DSL (Drizzle-style typed query builder)

**Goal:** typed, composable SQL — `db::select().from(users::table).where_(users::id.eq(1))`.

### 17a — Core types (`src/dsl/`)
- [ ] `src/dsl/mod.rs` — public re-exports, gated behind `feature = "query"`
- [ ] `src/dsl/column.rs` — `Column<Table, Value>` typed column reference
  - `.eq(v)`, `.ne(v)`, `.gt(v)`, `.lt(v)`, `.gte(v)`, `.lte(v)`
  - `.like(s)`, `.in_(vec)`, `.is_null()`, `.is_not_null()`
  - `.asc()`, `.desc()` → `OrderExpr`
- [ ] `src/dsl/table.rs` — `Table` trait: `table_name()`, `all_columns()`
- [ ] `src/dsl/expr.rs` — `Expr` enum: `Col(Column)`, `Lit(SqlValue)`, `And`, `Or`, `Not`
- [ ] `src/dsl/select.rs` — `SelectBuilder` with fluent API
- [ ] `src/dsl/insert.rs` — `InsertBuilder` with `.values(row)` + `.returning()`
- [ ] `src/dsl/update.rs` — `UpdateBuilder` with `.set(col.eq(v)).where_(expr)`
- [ ] `src/dsl/delete.rs` — `DeleteBuilder` with `.where_(expr)`

### 17b — Entry-point functions (`src/dsl/db.rs`)
- [ ] `db::select()` → `SelectBuilder`
- [ ] `db::insert_into(table)` → `InsertBuilder`
- [ ] `db::update(table)` → `UpdateBuilder`
- [ ] `db::delete_from(table)` → `DeleteBuilder`

### 17c — `#[derive(Table)]` macro
- [ ] Add `Table` derive to `rok-fluent-macros`
- [ ] Generates `mod <table_name> { pub struct table; pub static id: Column<...>; ... }`
- [ ] Each column field generates a `Column<T, FieldType>` constant
- [ ] Table struct implements `Table` trait

### 17d — PostgreSQL executor integration
- [ ] `SelectBuilder::fetch_all(&pool)`, `.fetch_one(&pool)`, `.fetch_optional(&pool)`
- [ ] `InsertBuilder::execute(&pool)`, `.returning().fetch_one(&pool)`
- [ ] `UpdateBuilder::execute(&pool)`
- [ ] `DeleteBuilder::execute(&pool)`

---

## Phase 18 — DX Improvements (both styles)

- [ ] `exists()` terminal on `ModelQuery` / `SelectBuilder` returning `bool`
- [ ] `first_or_default()` / `first_or_else(|| ...)` on query terminals
- [ ] Cursor-based pagination: `paginate_cursor(after: Option<Cursor>, limit: u64)`
- [ ] `QueryEvent` hook: `on_query(fn(sql, params, duration_ms, table_name))`
- [ ] `EXPLAIN` helper: `query.explain(&pool)` → `String`
- [ ] `Pool::ping(&pool) -> bool` health-check
- [ ] Named pool registry: `Pool::named("read_replica")`

---

## Phase 19 — `SqlValue` Completeness

- [ ] Add `SqlValue::Array(Vec<SqlValue>)` for `ANY($1)` style queries
- [ ] Add `SqlValue::Json(serde_json::Value)` (distinct from `Text`)
- [ ] Add `SqlValue::Uuid(uuid::Uuid)` (distinct from `Text`)
- [ ] Update all bind helpers in `src/core/sqlx/pg.rs`, `sqlite.rs`, `mysql.rs`

---

## Phase 20 — `rok db` CLI binary (optional convenience tool)

- [ ] Add `[[bin]]` target or separate `rok-fluent-cli` crate
- [ ] `rok db migrate` → `MigrationRunner::run()`
- [ ] `rok db rollback` → `MigrationRunner::rollback()`
- [ ] `rok db status` → `MigrationRunner::status()`
- [ ] `rok db make <name>` → scaffold a timestamped migration file
