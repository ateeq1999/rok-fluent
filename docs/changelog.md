# Changelog

All notable changes are documented here following [Keep a Changelog](https://keepachangelog.com)
conventions. Versions follow [Semantic Versioning](https://semver.org).

---

## [Unreleased]

### Changed
- **Breaking:** consolidated five crates (`rok-orm`, `rok-orm-core`, `rok-orm-macros`,
  `rok-orm-factory`, `rok-orm-migrate`) into a single `rok-fluent` crate.
- All `rok_orm::` / `rok_orm_core::` import paths become `rok_fluent::`.
- `rok-orm-macros` renamed to `rok-fluent-macros` (internal; never imported directly).
- `query!` moved from proc-macro to `macro_rules!` (same syntax, faster compile).
- SQLx adapter modules moved from `core/sqlx_pg.rs` etc. to `core/sqlx/{pg,sqlite,mysql}.rs`.
- Feature flags renamed/consolidated (see [docs/features.md](features.md)).

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
