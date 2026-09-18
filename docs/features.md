# Feature Flags

rok-fluent uses Cargo feature flags to keep compile times and dependency trees small.
Enable only what your project needs.

## Default

```toml
rok-fluent = "0.4"   # enables: macros
```

The `default` feature pulls in `rok-fluent-macros` (the proc-macro crate) so
`#[derive(Model, Resource, Seed)]` work out of the box. To opt out:

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
- `ModelQuery<M>` — fluent query builder: `.and_where()`, `.order_by()`, `.paginate()`, `.cursor_paginate()`, `.exists()`, `.count()`
- `PgModel` / `MySqlModel` / `SqliteModel` — CRUD traits: `create`, `update`, `delete`, `find`, `bulk_create`, `upsert_returning`, soft-delete, restore
- `MorphTo` / `MorphMany` — polymorphic relationships
- `ThroughQuery` — has-many-through queries
- `EagerLoadable` — `with_has_many`, `with_has_one`, `with_belongs_to` batch loaders
- **`rok_fluent::services`** — `CrudService<M>`, `FilterBuilder<M>`, `SortBuilder<M>`, `BatchService<M>`, `SoftDeleteService<M>`, `SearchService<M>`, `AuditService<M>` *(in progress)*
- **`TransactionService`** — composable transactions with savepoints *(planned)*
- **`LockService`** — advisory and row-level locks *(planned)*
- **`orm::postgres::repository`** *(`postgres` + `active`)* — `Repository<M>` / DI override
  registry: register an `Arc<dyn Repository<M>>` per model and `PgModel`'s
  `find_by_pk`/`create`/`update_by_pk`/`delete_by_pk`/`all` transparently delegate to it.
  See [ORM docs](api/orm.md#repository--di-override-rok_fluentormpostgresrepository--feature-active--postgres).

**Extra deps:** `async-trait` 0.1 (also pulled in by `migrate`; used by `Repository<M>`'s
`#[async_trait]` trait so it can be stored as `Arc<dyn Repository<M>>`)

```rust
// Active Record example
let users: Vec<User> = User::filter("active", true)
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

Enables the derive macros. Included in `default`.

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

## Validation

### `validate`

Integrates the [`validator`](https://docs.rs/validator) crate with
[`Hooks`](api/orm.md#hooks-rok_fluentormhooks--feature-active--postgres-for-the-instance-methods):
adds `impl From<validator::ValidationErrors> for OrmError`, so a model that also
`#[derive(validator::Validate)]` with its own `#[validate(...)]` field attributes
(`email`, `length(...)`, `range(...)`, `custom(...)`, ...) can call
`self.validate().map_err(OrmError::from)?` inside `before_save`/`before_create`.
rok-fluent does not parse `#[validate(...)]` attributes itself — `validator`'s own
derive macro does that work independently.

```toml
rok-fluent = { version = "0.4", features = ["validate"] }
```

```rust,ignore
use rok_fluent::orm::hooks::{Hooks, OrmError, OrmResult};
use validator::Validate;

#[derive(validator::Validate)]
struct User {
    #[validate(email)]
    email: String,
}

impl Hooks for User {
    fn before_save(&mut self) -> OrmResult<()> {
        self.validate().map_err(OrmError::from)
    }
}
```

**Extra deps:** `validator` 0.21 (`derive` feature)

See [`examples/12_validation.rs`](../examples/12_validation.rs).

---

## Query Result Cache

### `cache`

Opt-in, per-query result cache (`rok_fluent::orm::cache`) — a process-wide, TTL-based,
table-keyed registry for read-heavy endpoints that repeat the same query. Nothing is
cached implicitly: callers opt in per query by calling a `_cached` terminal and
supplying a TTL; every other terminal's behavior is unchanged.

```toml
rok-fluent = { version = "0.4", features = ["cache"] }
```

Provides:
- `orm::cache::get::<T>(key) -> Option<Arc<T>>` / `put::<T>(key, Arc<T>, ttl)` — the
  generic get/put primitives (mirrors the `NAMED_POOLS` registry pattern in
  `orm::postgres::pool`)
- `orm::cache::invalidate_table(table)` — evict every entry keyed under `table`
- `orm::cache::clear()` — evict everything
- `SelectBuilder::fetch_all_cached::<T>(&pool, ttl) -> Result<Arc<Vec<T>>, sqlx::Error>`
  *(feature `postgres` + `cache`)* — same query as `.fetch_all()`, but checks the
  cache first and stores the result on a miss. Returns `Arc<Vec<T>>` (not `Vec<T>`)
  so a cache hit hands every caller the same allocation without requiring `T: Clone`.
- `SelectBuilder::fetch_optional_cached::<T>(&pool, ttl) -> Result<Option<Arc<T>>, sqlx::Error>`
  *(feature `postgres` + `cache`)* — same, for `.fetch_optional()`.
- Automatic invalidation on write: `InsertBuilder::execute`, `UpdateBuilder::execute`,
  `DeleteBuilder::execute` (feature `postgres` + `cache`), and the Active Record
  write path (`orm::postgres::executor::insert`/`update`/`delete`) all call
  `invalidate_table` for the affected table after a successful write.
- If the `metrics` feature is also enabled, `orm::cache::get` increments
  `rok_fluent_cache_hit_total` / `rok_fluent_cache_miss_total`.

```rust,ignore
use std::time::Duration;

// First call misses the cache and queries the database; every call within
// the TTL after that is served from the cache with zero DB round trips.
let posts: Arc<Vec<Post>> = db::select()
    .from(Post::table())
    .fetch_all_cached::<Post>(&pool, Duration::from_secs(30))
    .await?;

// A write through the DSL busts the cache for "posts" automatically.
db::insert_into(Post::table())
    .values([("title", "New post")])
    .execute(&pool)
    .await?;

// Manual escape hatch for writes made outside the DSL/AR paths (e.g. raw SQL):
rok_fluent::orm::cache::invalidate_table("posts");
```

**Extra deps:** `dashmap` 6 (declared independently of `postgres`, which also pulls
in `dashmap` for its own named-pool registry, so `cache` doesn't force-enable a
database backend just for the dependency)

**Out of scope for this phase:** sqlite/mysql cache read-through terminals — the
DSL's async terminals (`SelectBuilder::fetch_all`, etc.) only exist for PostgreSQL
today, so `cache`'s read-through terminals are inherently PostgreSQL-scoped too.

See [`examples/14_query_cache.rs`](../examples/14_query_cache.rs).

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
rows into a PostgreSQL database via `PgModel::create_returning`. Also requires
`active` (`PgModel` is gated behind it).

```toml
rok-fluent = { version = "0.4", features = ["factory-postgres", "active"] }
```

Implies `factory` + `postgres`.

The same `.create()`/`.create_many()` methods also work against SQLite — enable
`factory` + `sqlite` + `active` instead (no separate `factory-sqlite` feature
needed; the pool type you pass in, `&PgPool` vs `&SqlitePool`, selects the
backend). See [`examples/06_factories_faker.rs`](../examples/06_factories_faker.rs).

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

## CLI

### `cli`

Enables the `rok` binary — a command-line tool for running migrations and inspecting
the database schema. Implies `migrate-postgres`.

```toml
rok-fluent = { version = "0.4", features = ["cli"] }
```

```sh
# Install locally
cargo install --path . --features cli

# Or run without installing
cargo run --features cli --bin rok -- db migrate
```

**Commands:**

| Command | Description |
|---|---|
| `rok db migrate [--dir migrations]` | Run all pending migration files |
| `rok db rollback [--dir migrations]` | Roll back the last applied batch |
| `rok db status [--dir migrations]` | Print Applied / Pending for every migration |
| `rok db make <name> [--dir migrations]` | Create a timestamped `.sql` file in `--dir` |
| `rok db seed` | Prints guidance (seeders are registered in code) |
| `rok db schema dump` | Emit approximate `CREATE TABLE` DDL from live DB |
| `rok db schema diff [--dir migrations]` | Show Applied / Pending / Orphan per file |

`DATABASE_URL` must be set in the environment.

**Extra deps:** `clap` 4

---

## Convenience Bundle

### `full`

Enables everything: `macros postgres axum tracing metrics tenant replica
factory-postgres migrate-postgres cli`.

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
| `rok db` CLI | `cli` | 36 | **Complete** |
| `TransactionService` — savepoints | `active` | 37 | Approved |
| `LockService` — advisory locks | `postgres` | 37 | Approved |
| `SchemaInspector` | `postgres` | 37 | Approved |
| `SelectBuilder::distinct_on` | `query` | 37 | Approved |
| `Repository<M>` / DI override | `active` + `postgres` | 42 | **Complete** |
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
