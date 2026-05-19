# Architecture

## Overview

rok-fluent is a single published crate (`rok-fluent`) with an internal proc-macro companion
(`rok-fluent-macros`). All source lives under `src/`. Optional subsystems are gated by Cargo
feature flags — the same pattern used by `tokio` (runtime/net/fs/macros) and `serde`
(derive/alloc/std).

## Directory Layout

```
rok-fluent/
├── Cargo.toml                 single lib crate manifest
├── Cargo.lock
├── CLAUDE.md                  project rules for Claude Code
├── rok-fluent-macros/         proc-macro crate (internal, not user-facing)
│   ├── Cargo.toml
│   └── src/lib.rs             Model, Resource, Seed derive implementations
└── src/
    ├── lib.rs                 public API surface + feature-gated re-exports
    ├── macros.rs              query! macro_rules! macro (always available)
    ├── core/                  foundation: traits, query builder, SQL types
    │   ├── mod.rs
    │   ├── condition.rs       WHERE/JOIN/ORDER/LIMIT/HAVING builders
    │   ├── model.rs           Model trait definition
    │   ├── query.rs           QueryBuilder<T>, Dialect, Join, WindowDef, SetOp
    │   ├── replica.rs         ReadStrategy, RoundRobinCounter  [replica]
    │   ├── schema_cache.rs    runtime column metadata cache
    │   ├── tenant.rs          TenantLayer Tower middleware       [tenant]
    │   └── sqlx/              SQLx type adapters
    │       ├── mod.rs
    │       ├── pg.rs          SqlValue ↔ PgArguments             [postgres]
    │       ├── sqlite.rs      SqlValue ↔ SqliteArguments         [sqlite]
    │       └── mysql.rs       SqlValue ↔ MySqlArguments          [mysql]
    ├── orm/                   runtime ORM: CRUD, relationships, hooks, etc.
    │   ├── mod.rs
    │   ├── casts.rs           field serialization casts (json/encrypted/enum/csv)
    │   ├── collection.rs      in-memory model collections
    │   ├── eager.rs           batch eager-loading (N+1 prevention)
    │   ├── hooks.rs           model lifecycle callbacks and observers
    │   ├── model_query.rs     pool-free fluent query builder
    │   ├── morph.rs           polymorphic relationships
    │   ├── n1.rs              N+1 detection utilities
    │   ├── orm_layer.rs       Tower middleware for pool injection  [axum]
    │   ├── pagination.rs      Page<T>, SimplePage<T>, CursorPage<T>
    │   ├── resource.rs        to_resource() JSON API transforms
    │   ├── scopes.rs          global and local query scopes
    │   ├── through.rs         nested/through relationship loading
    │   ├── postgres/          PostgreSQL-specific ORM             [postgres]
    │   │   ├── mod.rs
    │   │   ├── executor.rs    async query runner, retry logic
    │   │   ├── model.rs       PgModel CRUD trait
    │   │   ├── pool.rs        task-local pool + metrics
    │   │   ├── query_log.rs   optional query logging
    │   │   ├── transaction.rs Tx wrapper
    │   │   └── pivot_query.rs junction table queries
    │   ├── mysql/             MySQL-specific ORM                  [mysql]
    │   │   ├── mod.rs
    │   │   ├── executor.rs
    │   │   └── model.rs       MySqlModel CRUD trait
    │   └── sqlite/            SQLite-specific ORM                 [sqlite]
    │       ├── mod.rs
    │       ├── executor.rs
    │       └── model.rs       SqliteModel CRUD trait
    ├── factory/               test data factories                  [factory]
    │   ├── mod.rs             Factory trait, FactoryBuilder<T>
    │   └── faker.rs           Faker helpers
    └── migrate/               database migrations                  [migrate]
        ├── mod.rs
        ├── migration.rs       Migration and RawMigration traits
        ├── runner.rs          MigrationRunner orchestration
        ├── schema.rs          Schema builder, SchemaExecutor
        ├── source.rs          EmbeddedMigrations, FileSource
        └── table.rs           TableBuilder, ColumnBuilder
```

## Module Dependency Graph

```
rok-fluent-macros (proc-macro crate)
  └── invoked by: rok_fluent when feature = "macros"

src/core  (always compiled)
  ├── condition   — builds SQL WHERE/JOIN/ORDER clauses
  ├── model       — Model trait (table_name, primary_key, columns)
  ├── query       — QueryBuilder<T> + Dialect (Postgres / MySQL / SQLite)
  ├── schema_cache— runtime column introspection cache
  ├── replica     ← feature: replica
  ├── tenant      ← feature: tenant
  └── sqlx/
      ├── pg      ← feature: postgres
      ├── sqlite  ← feature: sqlite
      └── mysql   ← feature: mysql

src/orm   (always compiled, DB submods are feature-gated)
  ├── casts, collection, eager, hooks, model_query
  ├── morph, n1, pagination, resource, scopes, through
  ├── orm_layer   ← feature: axum
  ├── postgres/   ← feature: postgres
  ├── mysql/      ← feature: mysql
  └── sqlite/     ← feature: sqlite

src/factory  ← feature: factory / factory-postgres
src/migrate  ← feature: migrate / migrate-{postgres,sqlite,mysql}
```

## Key Design Decisions

### Single crate
Users depend on one crate. One version to pin, one CHANGELOG to read, one `use rok_fluent::*`
import. Internal boundaries are Rust modules, not crate boundaries.

### Proc-macro isolation
`rok-fluent-macros` is a required separate crate because Rust compiles proc-macro crates as
host-platform dynamic libraries loaded by rustc. They cannot share a compilation unit with
regular library code. Users never see this crate — `rok-fluent` re-exports its items under
the `macros` feature.

The `query!` macro is a `macro_rules!` macro (pure token substitution) and lives in
`src/macros.rs`. No proc-macro needed.

### Feature-flag layering
Mirrors tokio's design:
- Small orthogonal flags compose freely (`postgres` + `tracing` + `tenant`)
- Convenience bundles (`full`, `factory-postgres`, `migrate-postgres`) are just
  aliases that pull in the right combination
- `default = ["macros"]` keeps the zero-config experience ergonomic while remaining
  zero-cost — if you disable default features, zero async/DB deps are pulled in

### SqlValue
`SqlValue` is the type-erased SQL parameter type. It implements `From` for all common
Rust primitive types and is bound to database-specific argument types in `core/sqlx/`.
This decouples `QueryBuilder<T>` from any particular SQLx driver.

### Schema cache
`schema_cache` stores `TableSchema` (column names and types) at runtime, populated
during migrations. This lets the ORM avoid runtime `INFORMATION_SCHEMA` queries and
generate correct DDL for operations like `INSERT … ON CONFLICT`.

## Compile-Time Surface

With `default-features = false` and no features:
- Compiles `core` (condition, model, query, schema_cache) and the always-on ORM modules
- Zero async runtime deps
- Suitable as a base for other crates that just need `Model` + `QueryBuilder`

With `features = ["postgres", "macros"]` (typical app):
- Adds `sqlx` (postgres), `tokio`, `dashmap`, `futures`, `once_cell`
- Adds `rok-fluent-macros` (syn, quote, proc-macro2, heck)
- Cold build: ~20–40 s (dominated by sqlx compilation)
- Incremental: <2 s for application code changes
