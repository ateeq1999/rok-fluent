# Getting Started

rok-fluent ships two query styles.  You can use one or both:

| Style | Feature flag | Example |
|---|---|---|
| **Typed DSL** | `query` | `db::select().from(users::table).where_(users::id.eq(1_i64))` |
| **Active Record** | `active` | `User::query().where_eq("id", 1_i64).first().await?` |

---

## Install

Add rok-fluent to `Cargo.toml`. Pick the features you need (see [features.md](features.md)):

```toml
[dependencies]
# Typed DSL + Active Record + PostgreSQL
rok-fluent = { version = "0.4", features = ["active", "query", "postgres"] }

[dev-dependencies]
rok-fluent = { version = "0.4", features = ["factory-postgres", "migrate-postgres"] }
tokio = { version = "1", features = ["full"] }
```

---

## Style 1 — Typed DSL (`query` feature)

### Define a Table struct

```rust,no_run
use rok_fluent::dsl::db;

#[derive(Debug, sqlx::FromRow, rok_fluent::TableDerive)]
#[table(name = "users")]
pub struct User {
    pub id:    i64,
    pub name:  String,
    pub email: String,
}
// Generates: User::table(), User::ID, User::NAME, User::EMAIL
```

### Query

```rust,no_run
// SELECT * FROM "users" WHERE "users"."id" = $1
let user: Option<User> = db::select()
    .from(User::table())
    .where_(User::ID.eq(42_i64))
    .fetch_optional::<User>(&pool).await?;

// Compose expressions
let users: Vec<User> = db::select()
    .from(User::table())
    .where_(User::EMAIL.like("%@example.com").and(User::ID.gt(0_i64)))
    .order_by(User::NAME.asc())
    .limit(25)
    .fetch_all::<User>(&pool).await?;

// Pagination
use rok_fluent::orm::pagination::Page;
let page: Page<User> = db::select()
    .from(User::table())
    .where_(User::EMAIL.like("%@example.com"))
    .paginate::<User>(1, 25, &pool).await?;

// EXISTS check
let exists: bool = db::select()
    .from(User::table())
    .where_(User::EMAIL.eq("alice@example.com"))
    .exists(&pool).await?;
```

### Insert

```rust,no_run
// INSERT + RETURNING *
let created: User = db::insert_into(User::table())
    .values([("name", "Alice"), ("email", "alice@example.com")])
    .returning()
    .fetch_one::<User>(&pool).await?;

// Upsert (INSERT … ON CONFLICT DO UPDATE)
let user: User = db::insert_into(User::table())
    .values_typed([(User::EMAIL, "alice@example.com"), (User::NAME, "Alice")])
    .on_conflict([User::EMAIL]).do_update_values([(User::NAME, "Alice")])
    .returning()
    .fetch_one::<User>(&pool).await?;
```

### Update

```rust,no_run
db::update(User::table())
    .set_typed(User::NAME, "Bob")
    .where_(User::ID.eq(42_i64))
    .execute(&pool).await?;
```

### Delete

```rust,no_run
db::delete_from(User::table())
    .where_(User::ID.eq(42_i64))
    .execute(&pool).await?;
```

See `examples/02_crud.rs` for a runnable version of insert/update/delete/upsert
via the DSL, and `examples/09_window_functions_cte.rs` for window functions
and CTEs.

---

## Style 2 — Active Record (`active` + `postgres` features)

### Define a Model

```rust,no_run
use rok_fluent::Model;

#[derive(Debug, sqlx::FromRow, Model)]
#[model(table = "users", timestamps, soft_delete)]
pub struct User {
    pub id:            i64,
    pub name:          String,
    pub email:         String,
    #[model(skip)]
    pub password_hash: String,
    pub active:        bool,
}
```

`#[derive(Model)]` generates:
- `User::table_name()` → `"users"`
- `User::primary_key()` → `"id"`
- `User::columns()` → `["id", "name", "email", "active"]`

### Connect

```rust,no_run
use sqlx::PgPool;
use rok_fluent::orm::postgres::pool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pool = PgPool::connect("postgres://localhost/mydb").await?;

    // Pool-free ORM queries resolve their pool from this task-local scope;
    // `OrmLayer` does this automatically per request in Axum apps.
    pool::with_pool(pool, async {
        // … your queries here …
    })
    .await;

    Ok(())
}
```

See `examples/01_quickstart.rs` for a runnable connect + query walkthrough
(SQLite, zero setup).

### Query

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

See `examples/08_search_filter_sort.rs` for search/filter/sort composition,
and `examples/03_relations_eager_loading.rs` for eager loading relations.

### Insert

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

### Update

```rust,no_run
User::update_where(
    &[("active", false.into())],
    &[("email", "alice@example.com".into())],
)
.await?;
```

### Delete

```rust,no_run
// Soft delete (sets deleted_at)
User::soft_delete_where(&[("id", 42_i64.into())]).await?;

// Hard delete
User::delete_where(&[("id", 42_i64.into())]).await?;
```

### Pagination

```rust,no_run
use rok_fluent::orm::pagination::Page;

let page: Page<User> = User::query()
    .where_eq("active", true)
    .paginate(1, 25)         // page 1, 25 per page
    .await?;

println!("{} total, {} pages", page.meta.total, page.meta.last_page);
for user in page.data { /* … */ }
```

See `examples/03_relations_eager_loading.rs` for `CrudService::paginate_with`
in action.

### Transactions

```rust,no_run
use rok_fluent::orm::postgres::transaction::Tx;

let result = Tx::run(|tx| async move {
    User::insert_in_tx(&tx, &[("name", "Bob".into()), ("email", "bob@example.com".into())]).await?;
    Account::insert_in_tx(&tx, &[("user_id", bob_id.into())]).await?;
    Ok(())
})
.await?;
```

See `examples/04_transactions_locking.rs` for a runnable version with
savepoints and advisory locking.

---

## Next Steps

- [Feature flags reference](features.md)
- [Writing migrations](guides/migrations.md) — see also `examples/05_migrations.rs`
- [Testing with factories](guides/testing.md) — see also `examples/06_factories_faker.rs`
- [Axum integration](guides/axum.md) — see also `examples/07_axum_integration.rs`
- Multi-tenancy — see `docs/guides/multi-tenancy.md` and `examples/10_multi_tenancy.rs`
