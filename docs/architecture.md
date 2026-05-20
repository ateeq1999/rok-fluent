# Architecture

## Overview

rok-fluent is a single published crate (`rok-fluent`) with an internal proc-macro companion
(`rok-fluent-macros`). All source lives under `src/`. Optional subsystems are gated by Cargo
feature flags — the same pattern used by `tokio` (runtime/net/fs/macros) and `serde`
(derive/alloc/std).

The crate ships two independent query styles that can be used together or separately:

| Style | Feature | Entry point |
|---|---|---|
| **Typed DSL** | `query` | `db::select().from(users::table).where_(users::id.eq(1_i64))` |
| **Active Record** | `active` | `User::query().where_eq("id", 1_i64).first().await?` |

---

## Directory Layout

```
rok-fluent/
├── Cargo.toml                  single lib crate manifest
├── Cargo.lock
├── README.md
├── LICENSE
├── CLAUDE.md                   project rules for Claude Code
├── rok-fluent-macros/          proc-macro crate (internal, not user-facing)
│   ├── Cargo.toml
│   └── src/lib.rs              Model, Table, Resource, Seed derive implementations
├── .github/
│   └── workflows/
│       ├── ci.yml              test on every push/PR (fmt, clippy, test, feature matrix)
│       └── publish.yml         auto-publish to crates.io on version bump
└── src/
    ├── lib.rs                  public API surface + feature-gated re-exports
    ├── macros.rs               query! macro_rules! macro (always available)
    ├── core/                   foundation: traits, query builder, SQL types
    │   ├── mod.rs
    │   ├── condition.rs        SqlValue, Condition, WHERE/ORDER/LIMIT builders
    │   ├── model.rs            Model trait (table_name, primary_key, columns)
    │   ├── query.rs            QueryBuilder<T>, Dialect, Join, WindowDef, SetOp
    │   ├── replica.rs          ReadStrategy, RoundRobinCounter          [replica]
    │   ├── schema_cache.rs     runtime column metadata cache
    │   ├── tenant.rs           TenantLayer Tower middleware               [tenant]
    │   └── sqlx/               SQLx type adapters
    │       ├── mod.rs
    │       ├── pg.rs           SqlValue ↔ PgArguments                    [postgres]
    │       ├── sqlite.rs       SqlValue ↔ SqliteArguments                [sqlite]
    │       └── mysql.rs        SqlValue ↔ MySqlArguments                 [mysql]
    ├── dsl/                    typed query DSL                            [query]
    │   ├── mod.rs              re-exports db::, Column, Expr, Table trait
    │   ├── builders.rs         db::select / insert_into / update / delete_from
    │   ├── column.rs           Column<T, V> — typed column reference
    │   ├── expr.rs             Expr algebraic tree — .and(), .or(), ! (NOT)
    │   ├── select.rs           SelectBuilder — .from(), .where_(), .fetch_all(), …
    │   ├── insert.rs           InsertBuilder — .values(), .returning(), .execute()
    │   ├── update.rs           UpdateBuilder — .set(), .where_(), .execute()
    │   └── delete.rs           DeleteBuilder — .where_(), .execute()
    ├── orm/                    runtime ORM: CRUD, relationships, hooks, etc.
    │   ├── mod.rs
    │   ├── casts.rs            field serialization casts (json/encrypted/enum/csv)
    │   ├── collection.rs       in-memory model collections
    │   ├── eager.rs            batch eager-loading (N+1 prevention)
    │   ├── hooks.rs            model lifecycle callbacks and observers
    │   ├── model_query.rs      pool-free fluent query builder              [active]
    │   ├── morph.rs            polymorphic relationships                   [active]
    │   ├── n1.rs               N+1 detection utilities
    │   ├── orm_layer.rs        Tower middleware for pool injection          [axum]
    │   ├── pagination.rs       Page<T>, SimplePage<T>, CursorPage<T>
    │   ├── resource.rs         to_resource() JSON API transforms
    │   ├── scopes.rs           global and local query scopes               [active]
    │   ├── through.rs          through-relationship loading                [active]
    │   ├── postgres/           PostgreSQL-specific ORM                     [postgres]
    │   │   ├── mod.rs
    │   │   ├── executor.rs     async query runner, retry logic
    │   │   ├── model.rs        PgModel CRUD trait
    │   │   ├── pool.rs         task-local pool + metrics + ping()
    │   │   ├── query_log.rs    optional query logging
    │   │   ├── transaction.rs  Tx wrapper
    │   │   └── pivot_query.rs  junction table queries
    │   ├── mysql/              MySQL-specific ORM                          [mysql]
    │   │   ├── mod.rs
    │   │   ├── executor.rs
    │   │   └── model.rs        MySqlModel CRUD trait
    │   └── sqlite/             SQLite-specific ORM                         [sqlite]
    │       ├── mod.rs
    │       ├── executor.rs
    │       └── model.rs        SqliteModel CRUD trait
    ├── factory/                test data factories                         [factory]
    │   ├── mod.rs              Factory trait, FactoryBuilder<T>
    │   └── faker.rs            Faker helpers
    └── migrate/                database migrations                         [migrate]
        ├── mod.rs
        ├── migration.rs        Migration and RawMigration traits
        ├── runner.rs           MigrationRunner orchestration
        ├── schema.rs           Schema builder, SchemaExecutor
        ├── source.rs           EmbeddedMigrations, FileSource
        └── table.rs            TableBuilder, ColumnBuilder
```

---

## Module Dependency Graph

```
rok-fluent-macros (proc-macro crate)
  └── invoked by: rok_fluent when feature = "macros"
      generates: Model, Table, Resource, Seed derive impls + query! helper

src/core  (always compiled)
  ├── condition   — SqlValue, Condition, WHERE/ORDER/LIMIT/HAVING builders
  ├── model       — Model trait (table_name, primary_key, columns)
  ├── query       — QueryBuilder<T> + Dialect (Postgres / MySQL / SQLite)
  ├── schema_cache— runtime column introspection cache
  ├── replica     ← feature: replica
  ├── tenant      ← feature: tenant
  └── sqlx/
      ├── pg      ← feature: postgres
      ├── sqlite  ← feature: sqlite
      └── mysql   ← feature: mysql

src/dsl   ← feature: query  (DSL query builder; independent of active)
  ├── column      — Column<T, V> typed column reference
  ├── expr        — Expr algebraic tree
  └── builders    — SelectBuilder, InsertBuilder, UpdateBuilder, DeleteBuilder

src/orm   (always compiled, most submods are feature-gated)
  ├── model_query ← feature: active
  ├── morph       ← feature: active
  ├── scopes      ← feature: active
  ├── through     ← feature: active
  ├── eager, hooks, casts, collection, n1, pagination, resource
  ├── orm_layer   ← feature: axum
  ├── postgres/   ← feature: postgres
  ├── mysql/      ← feature: mysql
  └── sqlite/     ← feature: sqlite

src/factory  ← feature: factory / factory-postgres
src/migrate  ← feature: migrate / migrate-{postgres,sqlite,mysql}
```

---

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

### Two independent query styles
The `query` (Typed DSL) and `active` (Active Record) features are fully orthogonal:

- **`query` only** — zero overhead, compile-time column types, SQL-mirroring syntax. Good for
  libraries and teams that want SQL-level control without a runtime.
- **`active` only** — model-centric methods, scopes, lifecycle hooks, polymorphic relationships.
  Good for application code that prefers high-level CRUD.
- **Both** — the bridge `ModelQuery::and_expr()` accepts any `Expr` from the DSL, so you can
  use typed column predicates inside an Active Record query.

### `users::table` vs `User::table()` — dual API (Phase 21)
`#[derive(Table)]` generates both access styles:

```rust
// Module-based (primary, SQL-mirroring, zero runtime cost):
db::select().from(users::table).where_(users::id.eq(1_i64))

// Struct-associated (convenience alias, same underlying constant):
db::select().from(User::table()).where_(User::ID.eq(1_i64))
```

The module-based form (`users::table`) is the canonical style because it mirrors SQL identifiers
exactly and makes the table origin unambiguous in complex multi-join queries. The struct-associated
form is a convenience alias generated as:

```rust
impl User {
    pub fn table() -> users::TableMarker { users::table }
    pub const ID: Column<User, i64> = users::id;
    // … one const per field
}
```

### `Loaded<T>` relationship carrier (Phase 23)
Relationship fields use `Loaded<T>` instead of `Option<T>`:

```rust
pub enum Loaded<T> {
    NotLoaded,
    Loaded(T),
}
```

`Option<T>` is ambiguous — it could mean SQL NULL or "not fetched yet". `Loaded<T>` makes
the distinction explicit: `NotLoaded` = relationship was not included in the query;
`Loaded(val)` = the data is present. Code that forgets to `.with(users::posts)` gets a
`NotLoaded` variant, not a silent `None`.

### Relationship types (Phase 23–25)
All 11 relationship types are declared as attributes on `#[derive(Table)]` structs:

| Annotation | SQL pattern |
|---|---|
| `has_one = Post, fk = "user_id"` | `WHERE posts.user_id = users.id LIMIT 1` |
| `has_many = Post, fk = "user_id"` | `WHERE posts.user_id = users.id` |
| `belongs_to = User, fk = "user_id"` | `WHERE users.id = posts.user_id` |
| `many_to_many = Tag, through = "post_tags", fk = "post_id", other_fk = "tag_id"` | JOIN through junction table |
| `has_one_through = Country, through = Address, via = "country_id"` | two-hop join |
| `has_many_through = Permission, through = Role, via = "role_id"` | two-hop join |
| `belongs_to_through = Org, through = Team, via = "org_id"` | two-hop join |
| `morph_one = Image, as_ = "imageable"` | polymorphic 1:1 |
| `morph_many = Comment, as_ = "commentable"` | polymorphic 1:N |
| `morph_to = "imageable"` | inverse polymorphic |
| `morph_to_many = Tag, through = "taggables", name = "taggable"` | polymorphic M:M |

Relationships are loaded three ways:
1. **Batch load** — `.with(users::posts)` on `SelectBuilder` issues one extra query and
   stitches results in Rust (no N+1).
2. **JOIN builder** — `.inner_join(posts::table).on(users::id.eq_col(posts::user_id))` for
   queries that need JOIN semantics.
3. **After-fetch** — `.include(users::posts)` on an already-fetched `Vec<User>` for lazy
   post-processing.

### Feature-flag layering
Mirrors tokio's design:
- Small orthogonal flags compose freely (`postgres` + `tracing` + `tenant`)
- Convenience bundles (`full`, `factory-postgres`, `migrate-postgres`) are just
  aliases that pull in the right combination
- `default = ["macros"]` keeps the zero-config experience ergonomic while remaining
  zero-cost — if you disable default features, zero async/DB deps are pulled in

### SqlValue
`SqlValue` is the type-erased SQL parameter type. It implements `From` for all common
Rust primitive types (and `serde_json::Value`, `uuid::Uuid`) and is bound to
database-specific argument types in `core/sqlx/`. This decouples `QueryBuilder<T>` and
`Column<T, V>` from any particular SQLx driver.

| Rust type | `SqlValue` variant | PG binding |
|---|---|---|
| `&str`, `String` | `Text(String)` | `text` |
| integer types | `Integer(i64)` | `int8` |
| float types | `Float(f64)` | `float8` |
| `bool` | `Bool(bool)` | `bool` |
| `serde_json::Value` | `Json(Value)` | `jsonb` |
| `uuid::Uuid` | `Uuid(Uuid)` | `uuid` |
| `None` | `Null` | `NULL` |

### Schema cache
`schema_cache` stores `TableSchema` (column names and types) at runtime, populated
during migrations. This lets the ORM avoid runtime `INFORMATION_SCHEMA` queries and
generate correct DDL for operations like `INSERT … ON CONFLICT`.

---

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

With `features = ["query", "postgres", "macros"]` (DSL app):
- Same deps as above, plus the `dsl/` module (zero extra deps — pure Rust types)
- `#[derive(Table)]` generates companion modules at compile time

With `features = ["full"]` (examples / integration tests):
- Everything above plus axum, tower, tracing, metrics, uuid, serde_json
- Not recommended for production binaries
