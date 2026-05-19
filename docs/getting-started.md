# Getting Started

## Install

Add rok-fluent to `Cargo.toml`. Pick the features you need (see [features.md](features.md)):

```toml
[dependencies]
rok-fluent = { version = "0.4", features = ["postgres", "macros"] }

[dev-dependencies]
rok-fluent = { version = "0.4", features = ["factory-postgres", "migrate-postgres"] }
tokio = { version = "1", features = ["full"] }
```

## Define a Model

```rust,no_run
use rok_fluent::Model;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Model, sqlx::FromRow)]
#[rok_orm(table = "users", timestamps, soft_delete)]
pub struct User {
    pub id: i64,
    pub name: String,
    pub email: String,
    #[rok_orm(hidden)]
    pub password_hash: String,
    pub active: bool,
    // created_at and updated_at added by timestamps
    // deleted_at added by soft_delete
}
```

`#[derive(Model)]` generates:
- `User::table_name()` → `"users"`
- `User::primary_key()` → `"id"`
- `User::columns()` → `["id", "name", "email", "password_hash", "active"]`

## Connect

```rust,no_run
use sqlx::PgPool;
use rok_fluent::orm::postgres;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pool = PgPool::connect("postgres://localhost/mydb").await?;
    postgres::pool::set(pool);           // store in task-local
    Ok(())
}
```

## Query

```rust,no_run
use rok_fluent::{Model, query};
use rok_fluent::orm::postgres::model::PgModel;

// Fluent builder
let users = User::query()
    .where_eq("active", true)
    .order_by_desc("created_at")
    .limit(20)
    .all()
    .await?;

// Shorthand macro
let q = query!(User,
    where_eq "active" true,
    order_by_desc "created_at",
    limit 20,
);
let users = q.all().await?;

// Find by primary key
let user = User::find(1_i64).await?;

// Find or 404
let user = User::find_or_fail(1_i64).await?;
```

## Insert

```rust,no_run
use rok_fluent::orm::postgres::model::PgModel;

let id = User::insert(&[
    ("name",          "Alice".into()),
    ("email",         "alice@example.com".into()),
    ("password_hash", hash.into()),
    ("active",        true.into()),
])
.await?;
```

## Update

```rust,no_run
User::update_where(
    &[("active", false.into())],
    &[("email", "alice@example.com".into())],
)
.await?;
```

## Delete

```rust,no_run
// Soft delete (sets deleted_at)
User::soft_delete_where(&[("id", 42_i64.into())]).await?;

// Hard delete
User::delete_where(&[("id", 42_i64.into())]).await?;
```

## Pagination

```rust,no_run
use rok_fluent::orm::pagination::Page;

let page: Page<User> = User::query()
    .where_eq("active", true)
    .paginate(1, 25)         // page 1, 25 per page
    .await?;

println!("{} total, {} pages", page.total, page.last_page);
for user in page.data { /* … */ }
```

## Transactions

```rust,no_run
use rok_fluent::orm::postgres::transaction::Tx;

let result = Tx::run(|tx| async move {
    User::insert_in_tx(&tx, &[("name", "Bob".into()), ("email", "bob@example.com".into())]).await?;
    Account::insert_in_tx(&tx, &[("user_id", bob_id.into())]).await?;
    Ok(())
})
.await?;
```

## Next Steps

- [Feature flags reference](features.md)
- [Writing migrations](guides/migrations.md)
- [Testing with factories](guides/testing.md)
- [Axum integration](guides/axum.md)
