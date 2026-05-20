# API: ORM Runtime (`rok_fluent::orm`)

Core ORM types are always compiled. The Active Record query builder and model-centric
features require `feature = "active"`. Database-specific sub-modules require their
respective database feature (`postgres`, `sqlite`, `mysql`).

---

## `ModelQuery<M>` — fluent query builder — feature: `active`

`ModelQuery` is the entry point for Active Record style queries. Obtain one via
`YourModel::query()`.

```rust,no_run
use rok_fluent::orm::postgres::model::PgModel;

let users = User::query()
    .where_eq("active", true)
    .where_like("email", "%@example.com")
    .order_by_desc("created_at")
    .limit(25)
    .offset(0)
    .all()
    .await?;
```

### Fetch terminals

| Method | Returns | Notes |
|---|---|---|
| `.all().await?` | `Result<Vec<M>>` | all matching rows |
| `.first().await?` | `Result<Option<M>>` | first matching row |
| `.first_or_fail().await?` | `Result<M>` | `RowNotFound` if empty |
| `.first_or_default().await?` | `Result<M>` where `M: Default` | zero value if empty |
| `.first_or_else(|| ...).await?` | `Result<M>` | closure if empty |
| `.count().await?` | `Result<i64>` | `SELECT COUNT(*)` |
| `.exists().await?` | `Result<bool>` | `SELECT EXISTS(…)` |
| `.paginate(page, per).await?` | `Result<Page<M>>` | offset pagination |
| `.simple_paginate(page, per).await?` | `Result<SimplePage<M>>` | no total count |
| `.cursor_paginate(col, cursor, per).await?` | `Result<CursorPage<M>>` | stable cursors |

### Filter methods

| Method | SQL |
|---|---|
| `.where_eq("col", val)` | `WHERE col = $N` |
| `.where_ne("col", val)` | `WHERE col != $N` |
| `.where_gt("col", val)` | `WHERE col > $N` |
| `.where_gte("col", val)` | `WHERE col >= $N` |
| `.where_lt("col", val)` | `WHERE col < $N` |
| `.where_lte("col", val)` | `WHERE col <= $N` |
| `.where_like("col", "%pat%")` | `WHERE col LIKE $N` |
| `.where_in("col", vals)` | `WHERE col IN (…)` |
| `.where_null("col")` | `WHERE col IS NULL` |
| `.where_not_null("col")` | `WHERE col IS NOT NULL` |
| `.and_expr(dsl_expr)` | typed DSL `Expr` bridge — requires `query` feature |

### Order / limit

```rust,no_run
.order_by("name")          // ASC
.order_by_desc("created_at")
.limit(25)
.offset(50)
```

### DSL bridge (requires `active` + `query`)

```rust,no_run
use rok_fluent::dsl::db;

let posts = Post::query()
    .and_expr(posts::user_id.eq(42_i64).and(posts::published.eq(true)))
    .all()
    .await?;
```

---

## Pagination (`rok_fluent::orm::pagination`)

```rust,no_run
use rok_fluent::orm::pagination::{CursorPage, Page, SimplePage};

// Offset pagination
let page: Page<User> = User::query()
    .where_eq("active", true)
    .paginate(page_num, per_page)
    .await?;

// page.data        Vec<T>
// page.total       u64 — total matching rows
// page.per_page    u64
// page.current_page u64
// page.last_page   u64
// page.from        u64 — first row index on this page
// page.to          u64 — last row index on this page

// Simpler (no total count query)
let page: SimplePage<User> = User::query().simple_paginate(page_num, per_page).await?;

// Cursor pagination (stable for infinite scroll)
let page: CursorPage<User> = User::query()
    .cursor_paginate("id", cursor_value, per_page)
    .await?;
// page.data        Vec<T>
// page.next_cursor Option<String>
```

---

## Scopes (`rok_fluent::orm::scopes`)

Reusable WHERE clause fragments. Registered globally or applied inline.

```rust,no_run
use rok_fluent::orm::scopes::{GlobalScope, LocalScope};

// Global scope — applied automatically to every query for a model
struct ActiveScope;
impl GlobalScope<User> for ActiveScope {
    fn apply(&self, q: QueryBuilder<User>) -> QueryBuilder<User> {
        q.where_eq("active", true)
    }
}

// Register at startup
rok_fluent::orm::scopes::register::<User, _>(ActiveScope);

// Local scope — applied on-demand
let admins = User::query()
    .scope(|q| q.where_eq("role", "admin"))
    .all()
    .await?;

// Bypass all scopes
let all = User::query().without_global_scopes().all().await?;
```

---

## Hooks (`rok_fluent::orm::hooks`)

Model lifecycle callbacks. Implement the `Observer` trait and register it.

```rust,no_run
use rok_fluent::orm::hooks::{Observer, Event};

struct AuditObserver;

impl Observer<User> for AuditObserver {
    fn on_creating(&self, data: &[(&str, SqlValue)]) { /* … */ }
    fn on_created(&self, id: &SqlValue) { /* … */ }
    fn on_updating(&self, id: &SqlValue, data: &[(&str, SqlValue)]) { /* … */ }
    fn on_deleting(&self, id: &SqlValue) { /* … */ }
}

rok_fluent::orm::hooks::observe::<User, _>(AuditObserver);
```

---

## Eager Loading (`rok_fluent::orm::eager`)

Batch-load relationships to prevent N+1 queries.

```rust,no_run
use rok_fluent::orm::eager::EagerLoad;

// Load users and their posts in 2 queries instead of N+1
let users = User::query()
    .eager_load::<Post, _>(|user_ids| {
        Post::query().where_in("user_id", user_ids)
    })
    .all()
    .await?;
```

---

## Collections (`rok_fluent::orm::collection`)

In-memory model collections with filtering and grouping.

```rust,no_run
use rok_fluent::orm::collection::Collection;

let col = Collection::from(users);

let admins = col.filter(|u| u.role == "admin");
let by_role = col.group_by(|u| u.role.clone());
let sorted = col.sort_by(|a, b| a.name.cmp(&b.name));
let first = col.first();
let found = col.find(|u| u.id == 42);
```

---

## Casts (`rok_fluent::orm::casts`)

Field-level type transformation between Rust values and SQL storage.

```rust,no_run
use rok_fluent::orm::casts;

// Serialize a Rust value to its SQL storage form
let stored = casts::to_sql_json(&my_struct)?;
let stored = casts::to_sql_csv(&["a", "b", "c"])?;

// Deserialize from SQL storage
let value: MyStruct = casts::from_sql_json(&row_bytes)?;
let items: Vec<String> = casts::from_sql_csv(&row_str)?;
```

Declared on fields via `#[cast(json)]`, `#[cast(csv)]`, `#[cast(encrypted)]`,
`#[cast(enum)]`, `#[cast(timestamp)]`.

---

## Resources (`rok_fluent::orm::resource`)

JSON API response shaping. Used by `#[derive(Resource)]`.

```rust,no_run
pub type ResourceValue = serde_json::Value;

pub fn build_resource(entries: Vec<(&'static str, serde_json::Value)>) -> ResourceValue;
pub fn field_to_json<T: serde::Serialize>(value: &T) -> serde_json::Value;
```

With `#[derive(Resource)]`:

```rust,no_run
let json = user.to_resource();                               // excludes hidden fields
let json = user.to_resource_with_auth(|scope| auth.has(scope)); // conditional fields
```

---

## Morph (`rok_fluent::orm::morph`)

Polymorphic relationships — one table references multiple model types.

```rust,no_run
use rok_fluent::orm::morph::MorphMap;

// Register model → table mappings
MorphMap::register::<Post>("posts");
MorphMap::register::<Video>("videos");

// Resolve the morphable type at query time
let target_table = MorphMap::resolve("posts")?;
```

---

## Through (`rok_fluent::orm::through`)

Load nested has-many-through relationships in one query.

```rust,no_run
use rok_fluent::orm::through::through_query;

// Users → Teams → Projects (two hops)
let projects = through_query::<User, Team, Project>(
    user_id,
    "user_id",   // Team FK → User
    "team_id",   // Project FK → Team
)
.await?;
```

---

## OrmLayer (`rok_fluent::orm::orm_layer`) — feature: `axum`

Tower middleware that injects the pool into Axum request extensions.

```rust,no_run
use rok_fluent::orm::orm_layer::OrmLayer;

let app = Router::new()
    .route("/users", get(list_users))
    .layer(OrmLayer::new(pool));

// In handlers:
async fn list_users(Extension(pool): Extension<PgPool>) -> impl IntoResponse { /* … */ }
```
