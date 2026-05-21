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
morph relationships, through queries, and the built-in service layer.
Requires at least one database backend.

```toml
rok-fluent = { version = "0.4", features = ["active", "postgres"] }
```

Provides:
- `ModelQuery<M>` — fluent query builder: `.where_eq()`, `.order_by()`, `.paginate()`, `.cursor_paginate()`, `.exists()`, `.count()`
- `PgModel` / `MySqlModel` / `SqliteModel` — CRUD traits: `create`, `update`, `delete`, `find`, `bulk_create`, `upsert_returning`, soft-delete, restore
- `MorphTo` / `MorphMany` — polymorphic relationships
- `ThroughQuery` — has-many-through queries
- `EagerLoadable` — `with_has_many`, `with_has_one`, `with_belongs_to` batch loaders
- **`rok_fluent::services`** — `CrudService<M>`, `FilterBuilder<M>`, `SortBuilder<M>`, `BatchService<M>`, `SoftDeleteService<M>`, `SearchService<M>`, `AuditService<M>` *(in progress)*
- **`TransactionService`** — composable transactions with savepoints *(planned)*
- **`LockService`** — advisory and row-level locks *(planned)*

```rust
// Active Record example
let users: Vec<User> = User::query()
    .where_eq("active", true)
    .order_by_desc("created_at")
    .limit(25)
    .get()
    .await?;

// Service layer example
let svc = CrudService::<User>::new(pool.clone());
let page = svc.paginate(1, 25).await?;
let user = svc.upsert_by("email", &[("email", "a@b.com".into()), ("name", "Alice".into())]).await?;
```

### `query`

Drizzle-inspired typed DSL — compile-time column types, composable `Expr` tree,
SQL-mirroring syntax. Requires at least one database backend.

```toml
rok-fluent = { version = "0.4", features = ["query", "postgres"] }
```

Provides:
- `db::select()`, `db::insert_into()`, `db::update()`, `db::delete_from()` — entry points
- `SelectBuilder` — JOINs, GROUP BY / HAVING, CTEs, set operations, subqueries, pagination, aggregates
- `InsertBuilder` — `on_conflict`, `returning`, typed `Column<T,V>` value pairs
- `UpdateBuilder` — `set_typed()`, `returning()`, `fetch_one/fetch_all`
- `DeleteBuilder` — `returning()`
- `Column<T, V>` — typed column with `.eq()`, `.gt()`, `.like()`, `.in_()`, `.asc()`, `.desc()`, aggregates (`.count()`, `.sum()`, …), functions (`.lower()`, `.date_trunc()`, …)
- `Expr` — composable boolean tree: `.and()`, `.or()`, `!`, `Expr::case()`, `Expr::exists()`
- `AggExpr` / `FnExpr` / `CaseExpr` — projection and HAVING expressions
- `Loaded<T>` — relationship carrier (`NotLoaded` / `Some(T)`)
- `#[derive(Table)]` — generates `User::table()`, `User::ID`, `User::NAME`, … + `pub mod users { … }`
- Window functions: `.rank()`, `.row_number()`, `.lag(n)`, `.over(Window)` *(planned)*
- `SelectBuilder::distinct_on(cols)` — PostgreSQL `DISTINCT ON` *(planned)*
- `SelectBuilder::lock(Lock::ForUpdate)` — row-level locking *(planned)*

```rust
// OOP style (primary)
let user: Option<User> = db::select()
    .from(User::table())
    .where_(User::EMAIL.like("%@example.com").and(User::ID.gt(0_i64)))
    .order_by(User::NAME.asc())
    .limit(25)
    .fetch_optional::<User>(&pool).await?;

// Join with typed ON clause
db::select()
    .from(User::table())
    .inner_join(Post::table(), Post::USER_ID.references(User::ID))
    .where_(User::ACTIVE.eq(true))
    .fetch_all::<UserPost>(&pool).await?;
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

## Planned / Upcoming Features

The following are approved for implementation. See [todo.md](../todo.md) for the full task breakdown.

| Feature | Flag | Phase | Status |
|---|---|---|---|
| `SoftDeleteService<M>` | `active` | 32b | In progress |
| `SearchService<M>` — full-text + LIKE | `active` | 32b | In progress |
| `AuditService<M>` — touch / history | `active` | 32b | In progress |
| `BatchService::bulk_update` | `active` | 32b | In progress |
| AR ↔ DSL bridge (`and_expr`, `into_dsl`) | `active` + `query` | 33 | Approved |
| `.inspect()` / `.explain()` on builders | any | 34 | Approved |
| `#[table(searchable)]` + `#[table(rename_all)]` | `query` | 34 | Approved |
| `SqlValue::Array` + `Column::eq_any` | `postgres` | 35 | Approved |
| `SelectBuilder::stream()` | `query` | 35 | Approved |
| `COPY FROM STDIN` bulk path | `postgres` | 35 | Approved |
| `rok db` CLI | `cli` | 36 | Approved |
| `TransactionService` — savepoints | `active` | 37 | Approved |
| `LockService` — advisory locks | `postgres` | 37 | Approved |
| `SchemaInspector` | `postgres` | 37 | Approved |
| `SelectBuilder::distinct_on` | `query` | 37 | Approved |
| Window functions | `query` | 37 | Approved |
| `TypedJson<T>` column wrapper | `query` | 37 | Approved |
| `QueryLog` structured sink | `tracing` | 38 | Approved |

---

## Common Combinations

```toml
# Minimal PostgreSQL app
rok-fluent = { version = "0.4", features = ["postgres", "macros"] }

# PostgreSQL + Axum web service
rok-fluent = { version = "0.4", features = ["axum", "macros", "tracing"] }

# PostgreSQL + Active Record service layer
rok-fluent = { version = "0.4", features = ["active", "postgres", "macros"] }

# PostgreSQL + typed DSL
rok-fluent = { version = "0.4", features = ["query", "postgres", "macros"] }

# Both query styles + Axum
rok-fluent = { version = "0.4", features = ["active", "query", "axum", "macros", "tracing"] }

# PostgreSQL + migrations + factories (test profile)
rok-fluent = { version = "0.4", features = ["migrate-postgres", "factory-postgres", "macros"] }

# SQLite CLI tool, no macros
rok-fluent = { version = "0.4", default-features = false, features = ["sqlite"] }

# Multi-database library crate
rok-fluent = { version = "0.4", default-features = false }
# users of your crate enable postgres/sqlite/mysql themselves
```
