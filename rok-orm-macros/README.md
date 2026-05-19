# rok-orm-macros

> Proc-macros for automatic model mapping, scope derivation, and relationship registration in rok-orm.

Part of the [Rok Framework](https://rok.rs) — a full-stack Rust web framework built on Axum 0.8 and SQLx 0.8.

[![crates.io](https://img.shields.io/crates/v/rok-orm-macros)](https://crates.io/crates/rok-orm-macros) [![docs.rs](https://img.shields.io/docsrs/rok-orm-macros)](https://docs.rs/rok-orm-macros) [![MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

## Features

- `#[derive(Model)]` generates a complete `Model` trait implementation from a plain Rust struct
- Table name inference (snake_case plural) or explicit override via `#[model(table = "...")]`
- Per-field column name override with `#[model(column = "db_col_name")]`
- Composite primary key support via `#[model(primary_keys = "col1,col2")]`
- Soft-delete enablement via `#[model(soft_delete)]` using a `deleted_at` column
- `#[model(skip)]` to exclude non-column struct fields from SQL generation
- `#[derive(Scope)]` for reusable, named query constraint types
- Relationship registration via `#[has_many]`, `#[belongs_to]`, and `#[has_one]` helper attributes

## Installation

```toml
[dependencies]
rok-orm-macros = "0.2"
```

> This crate is re-exported by `rok-orm`. You typically do not need to add it directly —
> add `rok-orm` and the macros are available automatically.

## Quick Start

```rust
use rok_orm::Model;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, sqlx::FromRow, Model)]
#[model(table = "blog_posts", soft_delete)]
pub struct Post {
    pub id: i64,

    #[model(column = "author_id")]
    pub user_id: i64,

    pub title: String,
    pub body: String,
    pub published: bool,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

// The macro generates:
//   impl Model for Post { fn table_name() -> &'static str { "blog_posts" } ... }
//   Post::find(id), Post::all(), Post::query(), post.update(...), post.delete()
//   Post::query().only_trashed(), Post::query().with_trashed()
```

## Core API

### Model derive attributes

```rust
#[derive(Model)]
#[model(
    table = "project_members",
    primary_keys = "project_id,user_id",
    soft_delete,
)]
pub struct ProjectMember {
    #[model(primary_key)]
    pub project_id: i64,

    #[model(primary_key)]
    pub user_id: i64,

    #[model(column = "member_role")]
    pub role: String,

    #[model(skip)]                  // not stored in the DB
    pub display_label: Option<String>,
}

// Composite PK lookup — generates WHERE project_id = $1 AND user_id = $2
let member = ProjectMember::find_composite(vec![1_i64.into(), 2_i64.into()]).await?;
```

### #[derive(Scope)]

```rust
use rok_orm_macros::Scope;
use rok_orm_core::{Scope as ScopeTrait, QueryBuilder};

#[derive(Scope)]
#[scope(model = "Post")]
pub struct RecentScope {
    pub days: i64,
}

impl ScopeTrait<Post> for RecentScope {
    fn apply(&self, qb: QueryBuilder<Post>) -> QueryBuilder<Post> {
        qb.where_gte("created_at", chrono::Utc::now() - chrono::Duration::days(self.days))
    }
}

let recent = Post::query()
    .apply_scope(RecentScope { days: 7 })
    .order_by_desc("created_at")
    .get()
    .await?;
```

### Relationship helpers

```rust
use rok_orm_macros::Model;

#[derive(Model)]
#[model(table = "users")]
pub struct User {
    pub id: i64,
    pub email: String,
    pub name: String,
}

impl User {
    #[has_many(model = "Post", foreign_key = "user_id")]
    pub async fn posts(&self) -> Vec<Post> { unreachable!() }

    #[has_one(model = "Profile", foreign_key = "user_id")]
    pub async fn profile(&self) -> Option<Profile> { unreachable!() }
}

// Generated: user.posts().await?, user.profile().await?
```

## Integration

`rok-orm-macros` is a proc-macro crate consumed by `rok-orm` and re-exported through it. The generated `Model` implementation calls into `rok-orm-core` trait definitions and relies on the task-local SQLx pool provided by `rok-orm`'s `OrmLayer`. In application code, you only need:

```toml
rok-orm = { version = "0.2", features = ["postgres", "axum"] }
```

```rust
use rok_orm::Model;  // re-exports the derive macro from rok-orm-macros
```

## License

MIT
