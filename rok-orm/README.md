# rok-orm

> Eloquent-inspired async ORM for PostgreSQL, MySQL, and SQLite built on SQLx 0.8.

Part of the [Rok Framework](https://rok.rs) — a full-stack Rust web framework built on Axum 0.8 and SQLx 0.8.

[![crates.io](https://img.shields.io/crates/v/rok-orm)](https://crates.io/crates/rok-orm) [![docs.rs](https://img.shields.io/docsrs/rok-orm)](https://docs.rs/rok-orm) [![MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

## Features

- Derive-macro model mapping with automatic CRUD via `#[derive(Model)]`
- Fluent query builder: `filter()`, `order_by()`, `limit()`, `paginate()`, `first_or_404()`
- Relationships: `belongs_to`, `has_many`, `has_one`, `belongs_to_many`, `morph_many`
- Soft deletes with `with_trashed` and `only_trashed` query scopes
- Common Table Expressions (CTEs) and recursive CTEs
- Window functions (`RANK`, `ROW_NUMBER`, `LAG`, `LEAD`) with named windows
- Full-text search with `and_where_fts()` and `order_by_rank()`
- Composite primary keys, model observers, and query logging

## Installation

```toml
[dependencies]
rok-orm = { version = "0.2", features = ["postgres", "axum"] }
```

## Quick Start

```rust
use rok_orm::Model;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, sqlx::FromRow, Model)]
#[model(table = "users")]
pub struct User {
    pub id: i64,
    pub email: String,
    pub name: String,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

// In an Axum handler — pool is injected via OrmLayer (no parameter needed)
async fn list_users() -> impl IntoResponse {
    let users = User::filter("active", true)
        .order_by_desc("created_at")
        .limit(20)
        .get()
        .await?;

    let page = User::query()
        .paginate(1, 25)
        .await?;

    Json(users)
}

async fn get_user(Path(id): Path<i64>) -> impl IntoResponse {
    let user = User::find_or_404(id).await?;
    Json(user)
}
```

## Core API

### Model trait

```rust
// Static finders
let user  = User::find(42).await?;           // Option<User>
let user  = User::find_or_404(42).await?;    // User or 404 response
let all   = User::all().await?;
let first = User::query().filter("active", true).first().await?;

// Mutations
let id = User::insert(&[("email", "alice@example.com"), ("name", "Alice")]).await?;
user.update(&[("name", "Alice Smith")]).await?;
user.delete().await?;
```

### QueryBuilder

```rust
let results = User::query()
    .filter("role", "editor")
    .or_filter("role", "admin")
    .where_in("status", &["active", "pending"])
    .where_between("created_at", &start, &end)
    .order_by_asc("name")
    .limit(50)
    .offset(100)
    .get()
    .await?;
```

### Relationships

```rust
// has_many
let posts = user.has_many::<Post>("user_id").await?;

// belongs_to
let author = post.belongs_to::<User>("user_id").await?;

// belongs_to_many (many-to-many via junction table)
let tags = post.belongs_to_many::<Tag>("post_tags", "post_id", "tag_id").await?;

// Polymorphic
let comments = post.morph_many::<Comment>("commentable").await?;
```

### CTEs and Window Functions

```rust
// CTE
let top_posts = QueryBuilder::<Post>::new("posts")
    .select(&["id", "title", "view_count"])
    .order_by_desc("view_count")
    .limit(10);

let (sql, params) = QueryBuilder::<()>::new("posts")
    .with_cte("top_posts", &["id", "title", "view_count"], top_posts)
    .inner_join("top_posts", "posts.id = top_posts.id")
    .to_sql();

// Window function
let (sql, _) = QueryBuilder::<()>::new("sales")
    .select(&["rep", "amount"])
    .select_with_window("amount", "RANK", "rep_win")
    .window("rep_win", &["region"], &[("amount", OrderDir::Desc)])
    .to_sql();
```

## Feature Flags

| Flag | Description | Default |
|------|-------------|---------|
| `postgres` | PostgreSQL driver via SQLx | No |
| `sqlite` | SQLite driver via SQLx | No |
| `mysql` | MySQL driver via SQLx | No |
| `axum` | Tower middleware `OrmLayer` for task-local pool injection | No |

## Integration

`rok-orm` integrates with the Rok framework via `OrmLayer`, a Tower middleware that stores the SQLx pool in a task-local variable. This means handlers do not need to accept a `State<Pool>` parameter — the pool is automatically available to any `Model` method.

```rust
use rok_orm::OrmLayer;
use sqlx::PgPool;

let pool = PgPool::connect(&database_url).await?;

let app = Router::new()
    .route("/users", get(list_users))
    .layer(OrmLayer::new(pool));
```

Model observers integrate with `rok-mail` and `rok-queue` for side-effect hooks:

```rust
struct UserObserver;

impl ModelHooks<User> for UserObserver {
    async fn after_create(&self, user: &User) {
        Queue::dispatch(SendWelcomeEmail { user_id: user.id }).await.ok();
    }
    async fn before_delete(&self, user: &User) {
        Storage::delete_prefix(&format!("avatars/{}", user.id)).await.ok();
    }
}
```

## License

MIT
