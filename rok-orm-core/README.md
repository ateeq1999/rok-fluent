# rok-orm-core

> Core traits and primitive types shared across the rok-orm ecosystem.

Part of the [Rok Framework](https://rok.rs) — a full-stack Rust web framework built on Axum 0.8 and SQLx 0.8.

[![crates.io](https://img.shields.io/crates/v/rok-orm-core)](https://crates.io/crates/rok-orm-core) [![docs.rs](https://img.shields.io/docsrs/rok-orm-core)](https://docs.rs/rok-orm-core) [![MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

## Features

- `Model` trait defining the full CRUD surface for database models
- `QueryBuilder<T>` with fluent API for conditions, joins, ordering, and pagination
- `PaginatedResult<T>` and `CursorPage<T>` for structured page responses
- `Scope` trait for reusable, composable query constraints
- `SoftDelete` trait with `deleted_at` column convention
- `SqlValue` enum for type-erased parameter binding across all dialects
- Runtime schema cache (`schema_cache` module) for column metadata introspection
- Dialect abstraction (`Dialect` enum) for PostgreSQL, MySQL, and SQLite SQL generation

## Installation

```toml
[dependencies]
rok-orm-core = "0.2"
```

> In most cases you should depend on `rok-orm` directly rather than `rok-orm-core`.
> This crate is intended for authors of ORM extensions and proc-macro crates.

## Quick Start

```rust
use rok_orm_core::{Model, QueryBuilder, SqlValue, Dialect};

// Implement Model manually (or use #[derive(Model)] from rok-orm-macros)
impl Model for Article {
    fn table_name() -> &'static str { "articles" }
    fn primary_key() -> &'static str { "id" }
    fn dialect() -> Dialect { Dialect::Postgres }
}

// Build a query programmatically
let (sql, params) = QueryBuilder::<Article>::new("articles")
    .filter("published", true)
    .order_by_desc("published_at")
    .limit(10)
    .to_sql();

// sql  => "SELECT * FROM articles WHERE published = $1 ORDER BY published_at DESC LIMIT 10"
// params => vec![SqlValue::Bool(true)]
```

## Core API

### Model trait

```rust
pub trait Model: Sized + Send + Sync {
    fn table_name() -> &'static str;
    fn primary_key() -> &'static str { "id" }
    fn primary_keys() -> Vec<&'static str> { vec![Self::primary_key()] }
    fn dialect() -> Dialect { Dialect::Postgres }
    fn soft_delete_column() -> Option<&'static str> { None }

    // Provided by the blanket impl when Model is implemented
    fn query() -> QueryBuilder<Self>;
    fn find_composite(keys: Vec<SqlValue>) -> QueryBuilder<Self>;
}
```

### QueryBuilder

```rust
let qb = QueryBuilder::<Post>::new("posts")
    .select(&["id", "title", "body"])
    .filter("user_id", 42_i64)
    .where_in("status", &["draft", "published"])
    .where_null("deleted_at")
    .order_by_asc("title")
    .limit(25)
    .offset(50);

let (sql, params) = qb.to_sql();
```

### PaginatedResult and Scope

```rust
// PaginatedResult carries page metadata alongside rows
pub struct PaginatedResult<T> {
    pub data: Vec<T>,
    pub total: i64,
    pub page: u32,
    pub per_page: u32,
    pub last_page: u32,
}

// Scope — reusable query constraint
pub struct PublishedScope;

impl Scope<Post> for PublishedScope {
    fn apply(&self, qb: QueryBuilder<Post>) -> QueryBuilder<Post> {
        qb.filter("published", true).where_not_null("published_at")
    }
}

let posts = Post::query()
    .apply_scope(PublishedScope)
    .order_by_desc("published_at")
    .get()
    .await?;
```

### Schema Cache

```rust
use rok_orm_core::schema_cache::{set_schema, get_schema, TableSchema, ColumnMeta};

let schema = TableSchema {
    table: "users".into(),
    columns: vec![
        ColumnMeta { name: "id".into(), data_type: "bigint".into(), nullable: false, is_pk: true },
        ColumnMeta { name: "email".into(), data_type: "text".into(), nullable: false, is_pk: false },
    ],
    primary_keys: vec!["id".into()],
};

set_schema(schema);

if let Some(cached) = get_schema("users") {
    println!("PKs: {:?}", cached.primary_keys);
}
```

## Integration

`rok-orm-core` is the foundation of the rok-orm ecosystem. `rok-orm-macros` generates `Model` implementations that satisfy the traits defined here. `rok-orm` re-exports everything from this crate and adds the executor layer (SQLx query execution, `OrmLayer`).

If you are writing a crate that needs to accept any Rok model generically, depend only on `rok-orm-core` to avoid pulling in the full SQLx executor:

```rust
// In a generic utility crate
use rok_orm_core::{Model, Scope};

pub fn apply_tenant_scope<M: Model>(qb: QueryBuilder<M>, tenant_id: i64) -> QueryBuilder<M> {
    qb.filter("tenant_id", tenant_id)
}
```

## License

MIT
