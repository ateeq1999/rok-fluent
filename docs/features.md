# Feature Flags

rok-fluent uses Cargo feature flags to keep compile times and dependency trees small.
Enable only what your project needs.

## Default

```toml
rok-fluent = "0.4"   # enables: macros
```

The `default` feature pulls in `rok-fluent-macros` (the proc-macro crate) so
`#[derive(Model, Resource, Seed)]` and the `query!` macro work out of the box.
To opt out:

```toml
rok-fluent = { version = "0.4", default-features = false, features = ["postgres"] }
```

---

## Database Backends

### `postgres`

Enables the PostgreSQL executor, async connection pool, transaction wrapper, pivot-query
helpers, and the `PgModel` CRUD trait.

```toml
rok-fluent = { version = "0.4", features = ["postgres"] }
```

**Extra deps:** `sqlx` (postgres driver), `tokio` (rt), `dashmap`, `futures`, `once_cell`

### `sqlite`

Enables the SQLite executor and `SqliteModel` CRUD trait.

```toml
rok-fluent = { version = "0.4", features = ["sqlite"] }
```

**Extra deps:** `sqlx` (sqlite driver), `tokio` (rt)

### `mysql`

Enables the MySQL executor and `MySqlModel` CRUD trait.

```toml
rok-fluent = { version = "0.4", features = ["mysql"] }
```

**Extra deps:** `sqlx` (mysql driver), `tokio` (rt)

---

## Query Styles

rok-fluent ships two independent query styles. Enable one, both, or neither.

### `active`

Active Record / Eloquent style — high-level model methods, scopes, eager loading,
morph relationships, and through queries. Requires at least one database backend.

```toml
rok-fluent = { version = "0.4", features = ["active", "postgres"] }
```

Provides:
- `ModelQuery<M>` — fluent query builder on a model: `.where_eq()`, `.order_by()`, `.limit()`, `.get()`, `.first()`, `.count()`, `.paginate()`, `.cursor_paginate()`
- `PgModel` / `MySqlModel` / `SqliteModel` — CRUD traits: `.create()`, `.update()`, `.delete()`, `.find()`
- `MorphTo` / `MorphMany` — polymorphic relationships
- `ThroughQuery` — has-many-through queries
- `EagerLoadable` trait + `with_has_many`, `with_has_one`, `with_belongs_to` batch loaders

```rust
// Active Record example
let users: Vec<User> = User::query()
    .where_eq("active", true)
    .order_by_desc("created_at")
    .limit(25)
    .get()
    .await?;
```

### `query`

Drizzle-inspired typed DSL — compile-time column types, composable `Expr` tree,
SQL-mirroring syntax. Requires at least one database backend.

```toml
rok-fluent = { version = "0.4", features = ["query", "postgres"] }
```

Provides:
- `db::select()`, `db::insert_into()`, `db::update()`, `db::delete_from()` — entry points
- `SelectBuilder`, `InsertBuilder`, `UpdateBuilder`, `DeleteBuilder`
- `Column<T, V>` — typed column reference with `.eq()`, `.ne()`, `.gt()`, `.like()`, `.in_()`, `.is_null()`, `.asc()`, `.desc()`
- `Expr` — composable boolean tree with `.and()`, `.or()`, `!` (NOT)
- `#[derive(Table)]` — generates `pub mod <table> { pub const table: …; pub const <col>: Column<…>; … }`

```rust
// Typed DSL example
let user: Option<User> = db::select()
    .from(users::table)
    .where_(users::id.eq(42_i64))
    .fetch_optional::<User>(&pool)
    .await?;
```

---

## Macros

### `macros`

Enables the derive macros and `query!` shorthand. Included in `default`.

```toml
rok-fluent = { version = "0.4", features = ["macros"] }
```

Provides:
- `#[derive(Model)]` — implements `Model` trait; infers table name, primary key, columns.
  Use `#[model(table="...", pk="...", timestamps, soft_delete)]` for customization (or the
  legacy `#[rok_orm(...)]` namespace — both are supported).
- `#[derive(Table)]` — generates a typed DSL module for use with `feature = "query"`.
  Use `#[table(name="...")]` to set the table name; `#[table(skip)]` to exclude fields.
- `#[derive(Resource)]` — generates `to_resource()` for JSON API serialization
- `#[derive(Seed)]` — generates `seed(pool, n)` bulk-insert scaffolding
- `query!(Model, where_eq "col" val, limit 10)` — fluent query shorthand

---

## Web Integration

### `axum`

Enables `OrmLayer`, a Tower middleware that injects the database pool into request
extensions, making it available to Axum handlers without thread-locals.

Implies `postgres`.

```toml
rok-fluent = { version = "0.4", features = ["axum"] }
```

**Extra deps:** `axum`, `tower`

See [guides/axum.md](guides/axum.md).

---

## Observability

### `tracing`

Wraps every SQL query in an OpenTelemetry span with `db.statement`, `db.operation`, and
`db.table` attributes.

```toml
rok-fluent = { version = "0.4", features = ["tracing"] }
```

**Extra deps:** `tracing` 0.1

### `metrics`

Records Prometheus-compatible metrics via the `metrics` facade:
- `rok_fluent.query.duration` — histogram of query latency per table
- `rok_fluent.pool.size` — gauge of current pool size
- `rok_fluent.pool.idle` — gauge of idle connections

```toml
rok-fluent = { version = "0.4", features = ["metrics"] }
```

**Extra deps:** `metrics` 0.24

---

## Multi-Tenancy

### `tenant`

Enables `TenantLayer`, a Tower middleware that reads a tenant ID from the request
(header or JWT claim) and stores it in a task-local so every query can scope itself
automatically.

```toml
rok-fluent = { version = "0.4", features = ["tenant"] }
```

**Extra deps:** `tower`, `http`, `tokio` (rt)

See [guides/multi-tenancy.md](guides/multi-tenancy.md).

---

## Replica Routing

### `replica`

Enables `ReadStrategy` and `RoundRobinCounter` for routing read queries to replicas.
Configure via `DatabaseConfig` with multiple replica URLs.

```toml
rok-fluent = { version = "0.4", features = ["replica"] }
```

No extra deps beyond the chosen database backend.

---

## Test Utilities

### `factory`

Enables the `Factory` trait and `FactoryBuilder<T>` for building in-memory model
instances with realistic fake data, plus the `Faker` helper for generating names,
emails, UUIDs, sentences, etc.

```toml
rok-fluent = { version = "0.4", features = ["factory"] }
```

No extra deps.

### `factory-postgres`

Extends `factory` with async `.create()` and `.create_many()` methods that insert
rows into a PostgreSQL database.

```toml
rok-fluent = { version = "0.4", features = ["factory-postgres"] }
```

Implies `factory` + `postgres`.

See [guides/testing.md](guides/testing.md).

---

## Migrations

### `migrate`

Enables `MigrationRunner`, the `Schema` builder, `TableBuilder`, `ColumnBuilder`, and
the `Migration` and `RawMigration` traits. Does not pull in any database driver.

```toml
rok-fluent = { version = "0.4", features = ["migrate"] }
```

**Extra deps:** `async-trait`, `anyhow`

### `migrate-postgres`

Full PostgreSQL migration runner. Implies `migrate` + `postgres`.

```toml
rok-fluent = { version = "0.4", features = ["migrate-postgres"] }
```

### `migrate-sqlite`

Full SQLite migration runner. Implies `migrate` + `sqlite`.

```toml
rok-fluent = { version = "0.4", features = ["migrate-sqlite"] }
```

### `migrate-mysql`

Full MySQL migration runner. Implies `migrate` + `mysql`.

```toml
rok-fluent = { version = "0.4", features = ["migrate-mysql"] }
```

See [guides/migrations.md](guides/migrations.md).

---

## Convenience Bundle

### `full`

Enables everything: `macros postgres axum tracing metrics tenant replica
factory-postgres migrate-postgres`.

```toml
rok-fluent = { version = "0.4", features = ["full"] }
```

Use in examples, integration tests, and documentation builds. Not recommended for
production binaries — pick only what you need.

---

## Common Combinations

```toml
# Minimal PostgreSQL app
rok-fluent = { version = "0.4", features = ["postgres", "macros"] }

# PostgreSQL + Axum web service
rok-fluent = { version = "0.4", features = ["axum", "macros", "tracing"] }

# PostgreSQL + migrations + factories (test profile)
rok-fluent = { version = "0.4", features = ["migrate-postgres", "factory-postgres", "macros"] }

# SQLite CLI tool, no macros
rok-fluent = { version = "0.4", default-features = false, features = ["sqlite"] }

# Multi-database library crate
rok-fluent = { version = "0.4", default-features = false }
# users of your crate enable postgres/sqlite/mysql themselves
```
