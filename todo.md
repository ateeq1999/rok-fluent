# rok-fluent Consolidation — Task List

All source moves to `src/`. Feature-flag gating mirrors tokio/serde.
See `plan.md` for architecture decisions and the full feature-flag taxonomy.

---

## Phase 0 — Repo Scaffolding

- [ ] Add `[workspace]` section to root `Cargo.toml` (members: `.`, `rok-fluent-macros`)
- [ ] Convert root crate from binary (`main.rs`) to library (`lib.rs`); rename `name` to `rok-fluent`
- [ ] Bump root version to `0.4.0`, set `edition = "2021"`
- [ ] Create `rok-fluent-macros/` directory (copy of `rok-orm-macros/` with renamed crate)
- [ ] Update `rok-fluent-macros/Cargo.toml` — name `rok-fluent-macros`, version `0.4.0`
- [ ] Delete `src/main.rs`; create empty `src/lib.rs`
- [ ] Create `src/core/`, `src/orm/`, `src/orm/postgres/`, `src/orm/mysql/`, `src/orm/sqlite/`, `src/factory/`, `src/migrate/` directories
- [ ] Move `rok-orm/tests/` → `tests/` at repo root

---

## Phase 1 — Absorb `rok-orm-core` → `src/core/`

- [ ] Copy `rok-orm-core/src/condition.rs`   → `src/core/condition.rs`
- [ ] Copy `rok-orm-core/src/model.rs`        → `src/core/model.rs`
- [ ] Copy `rok-orm-core/src/query.rs`        → `src/core/query.rs`
- [ ] Copy `rok-orm-core/src/replica.rs`      → `src/core/replica.rs`
- [ ] Copy `rok-orm-core/src/schema_cache.rs` → `src/core/schema_cache.rs`
- [ ] Copy `rok-orm-core/src/tenant.rs`       → `src/core/tenant.rs`
- [ ] Copy `rok-orm-core/src/sqlx_pg.rs`      → `src/core/sqlx_pg.rs`
- [ ] Copy `rok-orm-core/src/sqlx_sqlite.rs`  → `src/core/sqlx_sqlite.rs`
- [ ] Copy `rok-orm-core/src/sqlx_mysql.rs`   → `src/core/sqlx_mysql.rs`
- [ ] Write `src/core/mod.rs` with correct `#[cfg(feature)]` gates:
  - Always: `condition`, `model`, `query`, `schema_cache`
  - `#[cfg(feature = "replica")]`: `replica`
  - `#[cfg(feature = "tenant")]`: `tenant`
  - `#[cfg(feature = "postgres")]`: `sqlx_pg`
  - `#[cfg(feature = "sqlite")]`: `sqlx_sqlite`
  - `#[cfg(feature = "mysql")]`: `sqlx_mysql`
- [ ] Replace all `crate::` paths in core files (were `rok_orm_core::`) with new `crate::core::` paths

---

## Phase 2 — Absorb `rok-orm` (database-agnostic files) → `src/orm/`

- [ ] Copy `rok-orm/src/casts.rs`       → `src/orm/casts.rs`
- [ ] Copy `rok-orm/src/collection.rs`  → `src/orm/collection.rs`
- [ ] Copy `rok-orm/src/eager.rs`       → `src/orm/eager.rs`
- [ ] Copy `rok-orm/src/hooks.rs`       → `src/orm/hooks.rs`
- [ ] Copy `rok-orm/src/model_query.rs` → `src/orm/model_query.rs`
- [ ] Copy `rok-orm/src/morph.rs`       → `src/orm/morph.rs`
- [ ] Copy `rok-orm/src/n1.rs`          → `src/orm/n1.rs`
- [ ] Copy `rok-orm/src/pagination.rs`  → `src/orm/pagination.rs`
- [ ] Copy `rok-orm/src/resource.rs`    → `src/orm/resource.rs`
- [ ] Copy `rok-orm/src/scopes.rs`      → `src/orm/scopes.rs`
- [ ] Copy `rok-orm/src/through.rs`     → `src/orm/through.rs`
- [ ] Copy `rok-orm/src/orm_layer.rs`   → `src/orm/orm_layer.rs`  (axum-gated)
- [ ] Write `src/orm/mod.rs` with correct `#[cfg(feature)]` gates

---

## Phase 3 — Absorb `rok-orm` (postgres) → `src/orm/postgres/`

- [ ] Copy `rok-orm/src/executor.rs`    → `src/orm/postgres/executor.rs`
- [ ] Copy `rok-orm/src/pg_model.rs`    → `src/orm/postgres/model.rs`
- [ ] Copy `rok-orm/src/pool.rs`        → `src/orm/postgres/pool.rs`
- [ ] Copy `rok-orm/src/query_log.rs`   → `src/orm/postgres/query_log.rs`
- [ ] Copy `rok-orm/src/transaction.rs` → `src/orm/postgres/transaction.rs`
- [ ] Copy `rok-orm/src/pivot_query.rs` → `src/orm/postgres/pivot_query.rs`
- [ ] Write `src/orm/postgres/mod.rs`

---

## Phase 4 — Absorb `rok-orm` (mysql + sqlite) → `src/orm/mysql/` and `src/orm/sqlite/`

- [ ] Copy `rok-orm/src/mysql_executor.rs` → `src/orm/mysql/executor.rs`
- [ ] Copy `rok-orm/src/mysql_model.rs`    → `src/orm/mysql/model.rs`
- [ ] Write `src/orm/mysql/mod.rs`
- [ ] Copy `rok-orm/src/sqlite_executor.rs` → `src/orm/sqlite/executor.rs`
- [ ] Copy `rok-orm/src/sqlite_model.rs`    → `src/orm/sqlite/model.rs`
- [ ] Write `src/orm/sqlite/mod.rs`

---

## Phase 5 — Absorb `rok-orm-factory` → `src/factory/`

- [ ] Copy `rok-orm-factory/src/lib.rs`   → `src/factory/mod.rs`
- [ ] Copy `rok-orm-factory/src/faker.rs` → `src/factory/faker.rs`
- [ ] Fix `crate::` → `crate::factory::` path references
- [ ] Gate all of `src/factory/` behind `#[cfg(feature = "factory")]`

---

## Phase 6 — Absorb `rok-orm-migrate` → `src/migrate/`

- [ ] Copy `rok-orm-migrate/src/lib.rs`        → `src/migrate/mod.rs`
- [ ] Copy `rok-orm-migrate/src/migration.rs`  → `src/migrate/migration.rs`
- [ ] Copy `rok-orm-migrate/src/runner.rs`     → `src/migrate/runner.rs`
- [ ] Copy `rok-orm-migrate/src/schema.rs`     → `src/migrate/schema.rs`
- [ ] Copy `rok-orm-migrate/src/source.rs`     → `src/migrate/source.rs`
- [ ] Copy `rok-orm-migrate/src/table.rs`      → `src/migrate/table.rs`
- [ ] Fix `crate::` → `crate::migrate::` path references
- [ ] Gate all of `src/migrate/` behind `#[cfg(feature = "migrate")]`
- [ ] Gate `runner.rs` postgres/sqlite/mysql paths with matching DB feature flags

---

## Phase 7 — Rename `rok-orm-macros` → `rok-fluent-macros`

- [ ] Rename crate directory `rok-orm-macros/` → `rok-fluent-macros/`
- [ ] Update `rok-fluent-macros/Cargo.toml`:
  - `name = "rok-fluent-macros"`
  - `version = "0.4.0"`
  - Remove any path dependency on `rok-orm-core`; replace with paths into `src/core/` via `rok-fluent`
- [ ] Update macro code: replace any `rok_orm_core::` references with `rok_fluent::core::` (or inline the needed types)
- [ ] Add `rok-fluent-macros` as optional dep in root `Cargo.toml` under `macros` feature

---

## Phase 8 — Wire up `src/lib.rs`

- [ ] Write top-level re-exports with `#[cfg(feature)]` guards (see plan.md `src/lib.rs` section)
- [ ] Ensure `pub use crate::core::{Model, QueryBuilder, Dialect, ...}` is always on
- [ ] Ensure `pub use rok_fluent_macros::{Model, Resource, Seed, query}` is behind `macros`
- [ ] Add `#[cfg(docsrs)]` all-features hint in doc comment for docs.rs

---

## Phase 9 — Cargo.toml

- [ ] Replace root `Cargo.toml` with the consolidated manifest (see plan.md Cargo.toml Design section)
- [ ] Verify all `[dependencies]` use `optional = true` where appropriate
- [ ] Add `[dev-dependencies]` (tokio full, any test helpers)
- [ ] Add `[package.metadata.docs.rs]` with `all-features = true`

---

## Phase 10 — Fix Imports & Compile

- [ ] Fix all internal `use rok_orm_core::` → `use crate::core::`
- [ ] Fix all internal `use rok_orm::` → `use crate::orm::`
- [ ] Run `cargo check --no-default-features` — must pass
- [ ] Run `cargo check --features postgres` — must pass
- [ ] Run `cargo check --features sqlite` — must pass
- [ ] Run `cargo check --features mysql` — must pass
- [ ] Run `cargo check --features full` — must pass
- [ ] Run `cargo check --features "migrate-postgres,factory-postgres,axum,tracing,metrics,tenant"` — must pass

---

## Phase 11 — Tests

- [ ] Move `rok-orm/tests/integration.rs`    → `tests/integration.rs`
- [ ] Move `rok-orm/tests/pg_integration.rs` → `tests/pg_integration.rs`
- [ ] Update integration test imports from `rok_orm::` → `rok_fluent::`
- [ ] Run `cargo test --features postgres` (requires live DB or CI fixture)

---

## Phase 12 — Cleanup Old Crates

- [ ] Delete `rok-orm/` directory
- [ ] Delete `rok-orm-core/` directory
- [ ] Delete `rok-orm-macros/` directory
- [ ] Delete `rok-orm-factory/` directory
- [ ] Delete `rok-orm-migrate/` directory
- [ ] Update `.gitignore` if it referenced any of the above paths

---

## Phase 13 — Tombstone Releases (optional, for crates.io users)

- [ ] Publish `rok-orm@0.3.99` with `[package] deprecated = true` and a `rok-fluent` re-export shim
- [ ] Publish `rok-orm-core@0.3.99` tombstone
- [ ] Publish `rok-orm-factory@0.3.99` tombstone
- [ ] Publish `rok-orm-migrate@0.3.99` tombstone
- [ ] Publish `rok-fluent@0.4.0` as the canonical release

---

## Phase 14 — Docs & README

- [ ] Update `README.md` at repo root: new crate name, feature table, migration guide
- [ ] Delete per-crate `README.md` files (or convert to `//!` doc comments in `lib.rs`)
- [ ] Verify `cargo doc --features full --open` renders cleanly
