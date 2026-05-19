# Guide: Multi-Tenancy

## Feature

```toml
rok-fluent = { version = "0.4", features = ["postgres", "tenant", "macros"] }
```

## Overview

`TenantLayer` is a Tower middleware that reads a tenant ID from each request and stores it
in a task-local. Query scopes then read the task-local to automatically filter every query
by tenant without any per-query changes to application code.

## TenantLayer Setup

```rust,no_run
use rok_fluent::core::tenant::{TenantLayer, TenantConfig};
use axum::Router;

let app = Router::new()
    .route("/posts", axum::routing::get(list_posts))
    .layer(TenantLayer::new(
        TenantConfig::default()
            .header("X-Tenant-ID")          // read from HTTP header
            // or .jwt_claim("tenant_id")    // read from JWT payload
            // or .subdomain()               // read from Host subdomain
    ));
```

## Global Tenant Scope

Register a global scope that appends `AND tenant_id = $n` to every query for a model:

```rust,no_run
use rok_fluent::orm::scopes::GlobalScope;
use rok_fluent::core::tenant;
use rok_fluent::core::query::QueryBuilder;

struct TenantScope;

impl GlobalScope<Post> for TenantScope {
    fn apply(&self, q: QueryBuilder<Post>) -> QueryBuilder<Post> {
        if let Some(tenant_id) = tenant::current_tenant_id() {
            q.where_eq("tenant_id", tenant_id.into())
        } else {
            q
        }
    }
}

// Register at startup — applies to every Post query automatically
rok_fluent::orm::scopes::register::<Post, _>(TenantScope);
```

Now every `Post::query()` call includes `WHERE tenant_id = $n` without any handler
changes.

## Bypassing Tenant Scope

Some operations (system jobs, cross-tenant reporting) need to bypass the tenant filter:

```rust,no_run
// Skip all global scopes for this query
let all_posts = Post::query().without_global_scopes().all().await?;
```

## Model Definition

Add `tenant_id` to every multi-tenant model:

```rust,no_run
#[derive(Debug, Serialize, Deserialize, Model, sqlx::FromRow)]
#[rok_orm(table = "posts", timestamps, tenant_scoped)]
pub struct Post {
    pub id: i64,
    pub tenant_id: String,
    pub title: String,
    pub body: String,
}
```

`#[rok_orm(tenant_scoped)]` is a marker attribute recognized by the global scope
auto-registration helper (if you use it) and by tooling.

## current_tenant_id()

Read the current tenant from anywhere in the call stack:

```rust,no_run
use rok_fluent::core::tenant::current_tenant_id;

async fn create_post(payload: CreatePostPayload) -> Result<Post, AppError> {
    let tenant_id = current_tenant_id()
        .ok_or(AppError::Unauthorized)?;

    Post::insert(&[
        ("tenant_id", tenant_id.into()),
        ("title",     payload.title.into()),
        ("body",      payload.body.into()),
    ])
    .await
    .map_err(AppError::Database)
}
```

## Migration Pattern

Each tenant-scoped table needs a `tenant_id` column and an index:

```sql
CREATE TABLE posts (
    id          BIGSERIAL PRIMARY KEY,
    tenant_id   VARCHAR(255) NOT NULL,
    title       VARCHAR(255) NOT NULL,
    body        TEXT,
    created_at  TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    updated_at  TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now()
);

CREATE INDEX idx_posts_tenant ON posts (tenant_id);
CREATE INDEX idx_posts_tenant_created ON posts (tenant_id, created_at DESC);
```

## Separate Schema per Tenant (alternative)

For strict data isolation, use PostgreSQL schemas instead of a `tenant_id` column.
Set the `search_path` per connection:

```rust,no_run
use sqlx::Connection;

async fn with_tenant_schema(pool: &PgPool, tenant: &str) -> anyhow::Result<()> {
    let mut conn = pool.acquire().await?;
    sqlx::query(&format!("SET search_path TO {tenant}, public"))
        .execute(&mut *conn)
        .await?;
    // queries on this connection hit the tenant's schema
    Ok(())
}
```

This approach requires managing schema creation and migrations per tenant.

## Testing Multi-Tenant Code

```rust,no_run
use rok_fluent::core::tenant;

#[sqlx::test]
async fn test_tenant_isolation(pool: sqlx::PgPool) {
    rok_fluent::orm::postgres::pool::set(pool.clone());

    // Simulate tenant A
    tenant::set_current_tenant_id("tenant_a");
    Post::insert(&[("tenant_id", "tenant_a".into()), ("title", "Post A".into())]).await.unwrap();

    // Simulate tenant B
    tenant::set_current_tenant_id("tenant_b");
    Post::insert(&[("tenant_id", "tenant_b".into()), ("title", "Post B".into())]).await.unwrap();

    // Tenant B should only see their own post
    let posts = Post::query().all().await.unwrap();
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].title, "Post B");

    // Without scope: both posts visible
    tenant::clear_current_tenant_id();
    let all = Post::query().without_global_scopes().all().await.unwrap();
    assert_eq!(all.len(), 2);
}
```
