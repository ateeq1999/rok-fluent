# Changelog

All notable changes are documented here following [Keep a Changelog](https://keepachangelog.com)
conventions. Versions follow [Semantic Versioning](https://semver.org).

---

## [Unreleased]

Planning Phase 21 (dual `users::table` + `User::table()` API) and Phase 22 (JOIN builder).
See [plan.md](../plan.md) for the full roadmap.

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
