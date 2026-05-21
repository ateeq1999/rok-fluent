# rok-fluent

**Eloquent-inspired async ORM for Rust** — PostgreSQL, MySQL, SQLite via SQLx.

```toml
[dependencies]
rok-fluent = { version = "0.4", features = ["active", "query", "postgres"] }
```

## Features

| Flag | What it enables |
|------|----------------|
| `macros` *(default)* | `#[derive(Model, Table, Resource, Seed)]`, `query!` macro |
| `active` | Active Record — `ModelQuery`, `PgModel`, `CrudService`, scopes, eager loading |
| `query` | Typed DSL — `db::select().from(table).where_(…)`, JOINs, CTEs, aggregates |
| `postgres` / `sqlite` / `mysql` | Database backend |
| `axum` | `OrmLayer` middleware (implies `postgres`) |
| `tracing` / `metrics` | OpenTelemetry spans / Prometheus counters |
| `tenant` | Multi-tenancy via Tower layer |
| `replica` | Read-replica routing strategies |
| `factory` | Test factories with `Faker` data generator |
| `migrate` / `migrate-postgres` / `migrate-sqlite` / `migrate-mysql` | Schema migration runner |
| `cli` | `rok db` CLI — migrate, rollback, status, schema dump |
| `full` | Everything above |

## Quick Start

### Typed DSL (`query` feature)

```rust
use rok_fluent::dsl::db;

#[derive(Debug, sqlx::FromRow, rok_fluent::Table)]
#[table(name = "users")]
pub struct User {
    pub id:    i64,
    pub name:  String,
    pub email: String,
}

// SELECT … FROM users WHERE email LIKE $1 ORDER BY name ASC LIMIT 25
let users: Vec<User> = db::select()
    .from(User::table())
    .where_(User::EMAIL.like("%@example.com"))
    .order_by(User::NAME.asc())
    .limit(25)
    .fetch_all(&pool).await?;

// INSERT … RETURNING *
let user: User = db::insert_into(User::table())
    .values([("name", "Alice"), ("email", "alice@example.com")])
    .returning()
    .fetch_one(&pool).await?;

// Upsert — INSERT … ON CONFLICT DO UPDATE
let user: User = db::insert_into(User::table())
    .values_typed([(User::EMAIL, "alice@example.com"), (User::NAME, "Alice")])
    .on_conflict(User::EMAIL).do_update([(User::NAME, "Alice")])
    .returning()
    .fetch_one(&pool).await?;

// Join with typed ON clause
let rows: Vec<(User, Post)> = db::select()
    .from(User::table())
    .inner_join(Post::table(), Post::USER_ID.references(User::ID))
    .fetch_all(&pool).await?;

// Pagination
let page: Page<User> = db::select()
    .from(User::table())
    .paginate(1, 25, &pool).await?;
```

### Active Record (`active` + `postgres` features)

```rust
use rok_fluent::Model;

#[derive(Debug, sqlx::FromRow, Model)]
#[model(table = "users", timestamps)]
pub struct User {
    pub id:    i64,
    pub name:  String,
    pub email: String,
    pub active: bool,
}

// Fluent query
let users = User::query()
    .where_eq("active", true)
    .order_by_desc("created_at")
    .limit(20)
    .all().await?;

// Find by primary key
let user = User::find(42_i64).await?;

// Paginate
let page: Page<User> = User::query()
    .where_eq("active", true)
    .paginate(1, 25).await?;
```

### Service Layer (`active` + `postgres`)

```rust
use rok_fluent::services::{CrudService, FilterBuilder, SortBuilder, BatchService};

let crud = CrudService::<User>::new(pool.clone());

// CRUD
let user = crud.find(42).await?;
let page = crud.paginate(1, 25).await?;
let created = crud.create(&[("name", "Alice".into()), ("email", "a@b.com".into())]).await?;
let updated = crud.update(42, &[("name", "Bob".into())]).await?;
crud.delete(42).await?;

// Filtering & sorting
let results = crud.query()
    .apply(FilterBuilder::default().eq("active", true))
    .apply(SortBuilder::default().allow("name", "created_at").then_by("name", "asc"))
    .paginate(1, 25).await?;

// Batch operations
let batch = BatchService::<User>::new(pool.clone());
let ids = batch.bulk_insert(&[user1, user2, user3]).await?;
batch.bulk_upsert_by("email", &[user1, user2]).await?;
```

## Migration from `rok-orm` 0.3

```toml
# Before (5 crates)
rok-orm         = { version = "0.3", features = ["postgres", "macros"] }
rok-orm-core    = { version = "0.3" }
rok-orm-macros  = { version = "0.3" }
rok-orm-migrate = { version = "0.3", features = ["postgres"] }
rok-orm-factory = { version = "0.3", features = ["postgres"] }

# After (1 crate)
rok-fluent = { version = "0.4", features = ["postgres", "macros", "migrate-postgres", "factory-postgres"] }
```

Import path changes:
- `rok_orm::` → `rok_fluent::`
- `rok_orm_core::SqlValue` → `rok_fluent::SqlValue`
- `rok_orm_migrate::MigrationRunner` → `rok_fluent::migrate::MigrationRunner`
- `#[derive(rok_orm_macros::Model)]` → `#[derive(rok_fluent::Model)]`

## Resources

- [Getting Started](docs/getting-started.md)
- [Feature Flags](docs/features.md)
- [API Reference (crates.io)](https://docs.rs/rok-fluent)
- [Architecture](docs/architecture.md)
- [Changelog](docs/changelog.md)

## License

MIT
