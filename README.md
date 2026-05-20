# rok-fluent

[![crates.io](https://img.shields.io/crates/v/rok-fluent.svg)](https://crates.io/crates/rok-fluent)
[![docs.rs](https://docs.rs/rok-fluent/badge.svg)](https://docs.rs/rok-fluent)
[![CI](https://github.com/ateeq1999/rok-fluent/actions/workflows/ci.yml/badge.svg)](https://github.com/ateeq1999/rok-fluent/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Async ORM for Rust built on [SQLx](https://github.com/launchbadge/sqlx), supporting **PostgreSQL**, **MySQL**, and **SQLite**.

Two query styles ship side by side — pick one or use both:

| Style | Feature flag | Inspiration |
|---|---|---|
| Active Record | `active` | Laravel Eloquent |
| Typed query DSL | `query` | Drizzle ORM |

---

## Installation

```toml
[dependencies]
rok-fluent = { version = "0.4", features = ["postgres"] }
```

Enable both query styles:

```toml
rok-fluent = { version = "0.4", features = ["active", "query", "postgres"] }
```

Or pull everything in:

```toml
rok-fluent = { version = "0.4", features = ["full"] }
```

---

## Feature flags

| Flag | Description |
|---|---|
| `macros` *(default)* | `#[derive(Model)]`, `#[derive(Table)]`, `#[derive(Resource)]`, `#[derive(Seed)]` |
| `active` | Active Record style — `ModelQuery`, `PgModel`, `MorphTo*`, `ThroughQuery` |
| `query` | Typed DSL — `db::select().from(users::table).where_(users::id.eq(1))` |
| `postgres` | PostgreSQL via sqlx |
| `sqlite` | SQLite via sqlx |
| `mysql` | MySQL via sqlx |
| `axum` | Tower middleware for Axum |
| `tracing` | OpenTelemetry-compatible span instrumentation |
| `metrics` | Prometheus-style query counters |
| `tenant` | Task-local multi-tenancy |
| `replica` | Read-replica routing |
| `migrate` | Schema migration runner |
| `factory` | Test factory helpers + `Faker` data generator |
| `full` | All of the above |

---

## Typed DSL style (`query` feature)

```rust
use rok_fluent::dsl::db;

#[derive(Debug, sqlx::FromRow, Table)]
#[table(name = "users")]
pub struct User {
    pub id:    i64,
    pub name:  String,
    pub email: String,
}

// SELECT * FROM "users" WHERE "users"."id" = $1 LIMIT 1
let user: Option<User> = db::select()
    .from(users::table)
    .where_(users::id.eq(42_i64))
    .fetch_optional::<User>(&pool)
    .await?;

// INSERT INTO "users" ("name", "email") VALUES ($1, $2) RETURNING *
let created: User = db::insert_into(users::table)
    .values([("name", "Alice"), ("email", "alice@example.com")])
    .returning()
    .fetch_one::<User>(&pool)
    .await?;

// UPDATE "users" SET "name" = $1 WHERE "users"."id" = $2
db::update(users::table)
    .set("name", "Bob")
    .where_(users::id.eq(42_i64))
    .execute(&pool)
    .await?;

// DELETE FROM "users" WHERE "users"."id" = $1
db::delete_from(users::table)
    .where_(users::id.eq(42_i64))
    .execute(&pool)
    .await?;
```

### Composable `WHERE` expressions

```rust
let expr = users::email.like("%@example.com")
    .and(users::id.gt(10_i64))
    .or(!users::name.is_null());

let rows: Vec<User> = db::select()
    .from(users::table)
    .where_(expr)
    .order_by(users::id.desc())
    .limit(20)
    .fetch_all::<User>(&pool)
    .await?;
```

---

## Active Record style (`active` + `postgres` features)

```rust
use rok_fluent::Model;

#[derive(Debug, sqlx::FromRow, Model)]
#[model(table = "posts", timestamps)]
pub struct Post {
    pub id:      i64,
    pub title:   String,
    pub user_id: i64,
}

// Find by primary key
let post = Post::query().find(1_i64).await?;

// Scoped query
let recent: Vec<Post> = Post::query()
    .where_eq("user_id", 42_i64)
    .order_by_desc("id")
    .limit(10)
    .get()
    .await?;

// Count
let total: i64 = Post::query().count().await?;

// Eager loading (avoids N+1)
let posts_with_author = with_belongs_to(
    recent,
    "id",
    |p: &Post| p.user_id,
    |u: &User| u.id,
).await?;
```

---

## Migrations (`migrate` feature)

```rust
use rok_fluent::migrate::{Migration, MigrationRunner, SchemaExecutor};
use async_trait::async_trait;

pub struct CreateUsersTable;

#[async_trait]
impl Migration for CreateUsersTable {
    fn name(&self) -> &str { "2026_05_18_000001_create_users_table" }

    async fn up(&self, schema: &SchemaExecutor) -> anyhow::Result<()> {
        schema.create("users", |t| {
            t.id();
            t.string("email").not_null().unique();
            t.string("name").not_null();
            t.timestamps();
        }).await
    }

    async fn down(&self, schema: &SchemaExecutor) -> anyhow::Result<()> {
        schema.drop_table_if_exists("users").await
    }
}

MigrationRunner::new(&pool)
    .migration(CreateUsersTable)
    .run()
    .await?;
```

---

## `SqlValue` types

| Variant | Rust type | PostgreSQL | SQLite / MySQL |
|---|---|---|---|
| `Text(String)` | `String`, `&str` | `TEXT` | `TEXT` |
| `Integer(i64)` | `i8`–`i64`, `u32`, `u64` | `BIGINT` | `INTEGER` |
| `Float(f64)` | `f32`, `f64` | `FLOAT8` | `REAL` |
| `Bool(bool)` | `bool` | `BOOLEAN` | `BOOLEAN` |
| `Json(Value)` | `serde_json::Value` | `JSONB` | text blob |
| `Uuid(Uuid)` | `uuid::Uuid` | native `UUID` | `CHAR(36)` |
| `Null` | `Option<T>` | `NULL` | `NULL` |

---

## Health check

```rust
if rok_fluent::orm::postgres::pool::ping(&pool).await {
    println!("database is reachable");
}
```

---

## License

MIT — see [LICENSE](LICENSE).
