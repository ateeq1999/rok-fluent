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

See [`examples/01_quickstart.rs`](../../examples/01_quickstart.rs) for a runnable
Active Record connect + query walkthrough.

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
| `.and_expr(dsl_expr)` | typed DSL `Expr` bridge — requires `active` + `query` features |
| `.or_expr(dsl_expr)` | OR variant of the above |

### Order / limit

```rust,no_run
.order_by("name")          // ASC
.order_by_desc("created_at")
.limit(25)
.offset(50)
```

### DSL bridge (requires `active` + `query`)

Inject typed DSL [`Expr`](crate::dsl::Expr) conditions into an AR chain, or convert
the whole chain to a [`SelectBuilder`] for DSL-only operations:

```rust,ignore
use rok_fluent::dsl::db;

// Inject a typed DSL condition into an AR chain
let posts = Post::all_query()
    .and_expr(posts::user_id.eq(42_i64).and(posts::published.eq(true)))
    .get()
    .await?;

// Convert an AR chain to a DSL SelectBuilder
let sel = Post::all_query()
    .and_where("published", true)
    .into_dsl()
    .inner_join(users::table, posts::user_id.eq(users::id))
    .select([users::name, posts::title]);
let rows = sel.fetch_all::<PostWithAuthor>(&pool).await?;
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

See [`examples/03_relations_eager_loading.rs`](../../examples/03_relations_eager_loading.rs)
for `CrudService::paginate_with` in action.

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

## Hooks (`rok_fluent::orm::hooks`) — feature: `active` + `postgres` for the instance methods

Model lifecycle callbacks. Implement the `Hooks` trait directly on your model — there is
no separate observer/registration step.

```rust,no_run
use rok_fluent::orm::hooks::{Hooks, OrmResult, OrmError};

impl Hooks for User {
    fn before_save(&mut self) -> OrmResult<()> {
        if self.email.is_empty() {
            return Err(OrmError::new("email cannot be empty"));
        }
        self.email = self.email.to_lowercase();
        Ok(())
    }
    fn after_save(&self) {
        tracing::info!(user_id = self.id, "user saved");
    }
}
```

All eight methods (`before_create`/`after_create`/`before_update`/`after_update`/
`before_save`/`after_save`/`before_delete`/`after_delete`) default to a no-op, so only
override what you need.

### Hook-aware instance writes (`PgModel::insert`/`save`/`destroy`)

Once a model implements both `Hooks` and `ModelValues` (the latter generated
automatically by `#[derive(Model)]`), `PgModel` provides hook-aware instance methods
that wrap the existing static CRUD methods:

```rust,no_run
use rok_fluent::orm::postgres::model::PgModel;

let mut user = User { id: 0, email: "Alice@Example.com".into() };
user.insert(&pool).await?;   // before_create → before_save → INSERT → after_create → after_save

user.email = "alice@example.com".into();
user.save(&pool).await?;     // before_update → before_save → UPDATE by pk → after_update → after_save

user.destroy(&pool).await?;  // before_delete → DELETE by pk → after_delete
```

`before_update`'s `_dirty: &[&str]` argument is always `Self::columns()` — this crate
does not track which fields actually changed. A hook error (`Err(OrmError)`) aborts the
write before touching the database and surfaces as
`sqlx::Error::Configuration(Box<OrmError>)`.

See [`examples/11_hooks.rs`](../../examples/11_hooks.rs).

### Validation (`feature = "validate"`)

`impl From<validator::ValidationErrors> for OrmError` lets a model that also
`#[derive(validator::Validate)]` plug the [`validator`](https://docs.rs/validator)
crate's own `#[validate(email, length(...), ...)]` field attributes straight into a
hook with `?`:

```rust,ignore
impl Hooks for User {
    fn before_save(&mut self) -> OrmResult<()> {
        self.validate().map_err(OrmError::from)
    }
}
```

See [features.md](../features.md#validate) and
[`examples/12_validation.rs`](../../examples/12_validation.rs).

---

## Repository / DI override (`rok_fluent::orm::postgres::repository`) — feature: `active` + `postgres`

Swap the implementation behind `PgModel`'s default CRUD methods for a given model
without touching any call site. `Repository<M>` has a default body for every method
that simply calls the corresponding static `PgModel` method — an override only
implements what it wants to change — and is dispatched dynamically via
`Arc<dyn Repository<M>>` (`#[async_trait]`, since RPITIT methods are not
`dyn`-compatible).

```rust,no_run
use rok_fluent::orm::postgres::repository::{register, Repository};
use rok_fluent::orm::postgres::model::PgModel;
use rok_fluent::core::condition::SqlValue;
use sqlx::PgPool;

struct FastUserRepo;

#[async_trait::async_trait]
impl Repository<User> for FastUserRepo {
    async fn find_by_pk(&self, pool: &PgPool, id: SqlValue) -> Result<Option<User>, sqlx::Error> {
        // custom caching / read-replica routing / test double, then delegate:
        User::find_by_pk(pool, id).await
    }
}

// Register once at startup — every existing `User::find_by_pk(...)` call site
// transparently delegates to it afterward.
register::<User, _>(FastUserRepo);
```

`find_by_pk`/`create`/`update_by_pk`/`delete_by_pk`/`all` on `PgModel` each check the
per-model registry first (one `TypeId` hashmap lookup — same cost `orm::scopes`
already pays) and fall through to the existing `executor::*` path when nothing is
registered, so this is fully additive: existing callers who never call `register()`
see zero behavior change.

See [`examples/13_repository_di.rs`](../../examples/13_repository_di.rs).

---

## Query Result Cache (`rok_fluent::orm::cache`) — feature: `cache` (terminals also need `postgres` + `query`)

Opt-in, per-query result cache. Nothing is cached implicitly — callers opt in per
query via a `_cached` terminal and a TTL; every other terminal's behavior is
unchanged. A process-wide `DashMap<String, CacheEntry>` (mirroring the `NAMED_POOLS`
registry in `orm::postgres::pool`) holds entries keyed by table-prefixed rendered
SQL + `SqlValue::to_sql_literal()`-joined params (`SqlValue`/`SelectBuilder` have no
`Hash`/`Eq` impl, so the key is derived from the rendered query output). Expired
entries are evicted lazily on the next `get` for that key — there is no background
sweep task.

```rust,ignore
use std::time::Duration;
use std::sync::Arc;

let posts: Arc<Vec<Post>> = db::select()
    .from(Post::table())
    .fetch_all_cached::<Post>(&pool, Duration::from_secs(30))
    .await?;

let post: Option<Arc<Post>> = db::select()
    .from(Post::table())
    .where_(Post::ID.eq(1_i64))
    .fetch_optional_cached::<Post>(&pool, Duration::from_secs(30))
    .await?;
```

`fetch_all_cached`/`fetch_optional_cached` return `Arc<Vec<T>>`/`Option<Arc<T>>`
rather than `Vec<T>`/`Option<T>` so a cache hit can hand every concurrent caller a
clone of the same `Arc` without requiring `T: Clone`. The plain `fetch_all`/
`fetch_optional`/`count`/etc. terminals are completely untouched by this feature.

**Invalidation.** `InsertBuilder::execute`/`UpdateBuilder::execute`/
`DeleteBuilder::execute` (feature `postgres` + `cache`) and the Active Record write
path (`orm::postgres::executor::insert`/`update`/`delete`) call
`cache::invalidate_table(table)` after every successful write, so a write through
any path this crate controls busts the cache for the tables it touched. For writes
made outside those paths (e.g. raw SQL), call the same functions directly:

```rust,ignore
rok_fluent::orm::cache::invalidate_table("posts");
rok_fluent::orm::cache::clear(); // evict everything
```

Generic primitives, usable directly for arbitrary keyed values beyond the
`SelectBuilder` terminals above:

```rust,ignore
use rok_fluent::orm::cache;
use std::time::Duration;
use std::sync::Arc;

cache::put("my-key".to_string(), Arc::new(42_i64), Duration::from_secs(60));
let v: Option<Arc<i64>> = cache::get::<i64>("my-key");
```

When the `metrics` feature is also enabled, `cache::get` increments
`rok_fluent_cache_hit_total` / `rok_fluent_cache_miss_total`.

**Out of scope this phase:** sqlite/mysql cache read-through — the DSL's async
terminals (`SelectBuilder::fetch_all`, etc.) only exist for PostgreSQL today, so the
cache terminals built on top of them are PostgreSQL-only too.

See [`examples/14_query_cache.rs`](../../examples/14_query_cache.rs).

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

See [`examples/03_relations_eager_loading.rs`](../../examples/03_relations_eager_loading.rs)
for a runnable version covering `with_has_many`, `group_has_many`, and
`CrudService::all_with`/`paginate_with`.

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

## Service Layer (`rok_fluent::services`) — features: `active` + `postgres`

Pre-built service types that sit above Active Record and own a `PgPool` or accept one per call.

### `CrudService<M>`

Pool-owning CRUD wrapper. All common read/write/paginate/search operations in one struct.

```rust,ignore
let svc = CrudService::<User>::new(pool.clone());

// Read
let all   = svc.all().await?;
let user  = svc.find(42_i64).await?;           // Option<User>
let user  = svc.find_or_fail(42_i64).await?;   // RowNotFound if missing
let n     = svc.count().await?;
let exists = svc.exists(42_i64).await?;

// Write
let user  = svc.create(&[("name", "Alice".into()), ("email", "a@example.com".into())]).await?;
svc.update(42_i64, &[("name", "Bob".into())]).await?;
svc.delete(42_i64).await?;
svc.soft_delete(42_i64).await?;
svc.restore(42_i64).await?;

// Pagination
let page  = svc.paginate(1, 25).await?;
let page  = svc.simple_paginate(1, 25).await?;
let page  = svc.cursor_paginate("id", None, 25).await?;

// Bulk
svc.bulk_create(&[vec![("name", "X".into())], vec![("name", "Y".into())]]).await?;
svc.delete_where(&[("active", false.into())]).await?;
let user  = svc.upsert_by("email", &[("email", "a@example.com".into())]).await?;

// Search
let hits  = svc.search("alice", &["name", "email"]).await?;
let page  = svc.search_paginated("alice", &["name", "email"], 1, 25).await?;
```

See [`examples/03_relations_eager_loading.rs`](../../examples/03_relations_eager_loading.rs)
for `CrudService::all_with`/`paginate_with` in action.

### `BatchService<M>`

Stateless multi-row operations — pass the pool each call.

```rust,ignore
// Bulk insert (multi-row INSERT — up to ~65k params)
BatchService::<User>::bulk_insert(&rows, &pool).await?;
BatchService::<User>::bulk_insert_chunked(&rows, 500, &pool).await?;

// COPY FROM STDIN — 10–50× faster than INSERT for large batches
BatchService::<User>::copy_insert(&rows, &pool).await?;

// Upsert by unique key
BatchService::<User>::bulk_upsert_by("email", &rows, &pool).await?;

// Update a set of IDs
BatchService::<User>::bulk_update(&[("active", false.into())], &[1_i64, 2, 3], &pool).await?;

// Delete where
BatchService::<User>::delete_where(&[("role", "guest".into())], &pool).await?;
```

### `FilterBuilder<M>`

Composable, reusable WHERE clause sets. Build filters separately from execution.

```rust,ignore
let mut fb = FilterBuilder::<User>::new();
fb.eq("active", true);
fb.like("email", "%@example.com");

let query = fb.apply(User::query());
let users: Vec<User> = query.all().await?;
```

See [`examples/08_search_filter_sort.rs`](../../examples/08_search_filter_sort.rs) for
a runnable version.

### `SortBuilder<M>`

Whitelist-validated user-driven sorting — safe for accepting sort parameters from HTTP requests.

```rust,ignore
let sb = SortBuilder::<User>::new(&["name", "created_at", "email"]);
let query = sb.apply(User::query(), "created_at", "desc");
let users = query.all().await?;
// Silently falls back to no-op for unknown columns.
```

See [`examples/08_search_filter_sort.rs`](../../examples/08_search_filter_sort.rs) for
a runnable version.

### `SoftDeleteService<M>`

Scoped operations for models with a `deleted_at` column.

```rust,ignore
let active  = SoftDeleteService::<Post>::all_active(&pool).await?;
let deleted = SoftDeleteService::<Post>::all_deleted(&pool).await?;
let all     = SoftDeleteService::<Post>::with_trashed(&pool).await?;

SoftDeleteService::<Post>::soft_delete(42_i64, &pool).await?;
SoftDeleteService::<Post>::restore(42_i64, &pool).await?;
SoftDeleteService::<Post>::force_delete(42_i64, &pool).await?;

// Hard-delete every row with a non-NULL deleted_at
let purged = SoftDeleteService::<Post>::purge_deleted(&pool).await?;
```

> Methods gracefully no-op (empty Vec / 0 rows) when the model has no `deleted_at` column.

### `SearchService<M>`

ILIKE and full-text search across declared columns.

```rust,ignore
// ILIKE '%alice%' on name OR email
let hits = SearchService::<User>::search("alice", &["name", "email"], &pool).await?;

// With offset pagination + COUNT(*)
let page = SearchService::<User>::search_paginated("alice", &["name", "email"], 1, 25, &pool).await?;

// Simple pagination (no COUNT)
let page = SearchService::<User>::search_simple_paginated("alice", &["name", "email"], 1, 25, &pool).await?;

// PostgreSQL full-text search (GIN index recommended)
// Renders: WHERE to_tsvector('english', name || ' ' || bio) @@ plainto_tsquery('english', $1)
let hits = SearchService::<User>::fts("alice", &["name", "bio"], &pool).await?;
```

See [`examples/08_search_filter_sort.rs`](../../examples/08_search_filter_sort.rs) for
a runnable version.

### `AuditService<M>`

Timestamp helpers and audit log queries for models with `created_at` / `updated_at`.

```rust,ignore
// Set updated_at = NOW() for the row with pk = 42
AuditService::<Post>::touch(42_i64, &pool).await?;

// Query the audit_log table (returns [] gracefully if the table doesn't exist)
let entries: Vec<AuditEntry> = AuditService::<Post>::history(42_i64, &pool).await?;
for e in &entries {
    println!("{} {} at {}", e.operation, e.record_id, e.changed_at);
}
```

`AuditEntry` fields: `table_name`, `record_id`, `operation` (`INSERT`/`UPDATE`/`DELETE`),
`old_data: Option<serde_json::Value>`, `new_data: Option<serde_json::Value>`,
`changed_at: chrono::DateTime<Utc>`.

Requires an `audit_log` table with columns:
`table_name TEXT, record_id TEXT, operation TEXT, old_data JSONB, new_data JSONB, changed_at TIMESTAMPTZ`.

### `TransactionService` — feature: `active` + `postgres`

Manual transaction management with savepoints and CRUD inside the transaction.

```rust,ignore
let tx = TransactionService::begin(&pool).await?;

// Savepoints
tx.savepoint("sp1").await?;
tx.rollback_to("sp1").await?;
tx.release("sp1").await?;

// CRUD inside the transaction (pool-free)
let user = tx.create::<User>(&[("name", "Alice".into())]).await?;
let user = tx.update_by_pk::<User>(42_i64, &[("name", "Bob".into())]).await?;
tx.delete_by_pk::<User>(42_i64).await?;

// Raw queries
let rows: Vec<User> = tx.fetch_all("SELECT * FROM users WHERE active = $1", &[true.into()]).await?;

tx.commit().await?;
// or tx.rollback().await?;
```

See [`examples/04_transactions_locking.rs`](../../examples/04_transactions_locking.rs)
for a runnable version.

### `LockService` — feature: `active` + `postgres`

PostgreSQL advisory and row-level locking.

```rust,ignore
// Advisory lock — blocking
LockService::acquire("my_lock", &pool).await?;
LockService::release("my_lock", &pool).await?;

// Advisory lock — non-blocking
let acquired = LockService::try_acquire("my_lock", &pool).await?;
if acquired { /* locked */ }

// Advisory lock — timeout
LockService::acquire_timeout("my_lock", Duration::from_secs(5), &pool).await?;

// Transaction-scoped advisory lock (auto-released on commit/rollback)
LockService::acquire_xact("my_tx_lock", &pool).await?;

// Row-level locking via SelectBuilder (feature: query)
db::select()
    .from(User::table())
    .where_(User::ID.eq(42_i64))
    .lock(Lock::ForUpdate)
    .lock_conflict(LockConflict::SkipLocked)
    .fetch_one::<User>(&pool).await?;
```

See [`examples/04_transactions_locking.rs`](../../examples/04_transactions_locking.rs)
for a runnable version of `LockService` advisory locking.

### `SchemaInspector` — feature: `postgres`

Query `information_schema` for column, index, and foreign-key metadata.

```rust,ignore
use rok_fluent::services::SchemaInspector;

let cols = SchemaInspector::columns("users", &pool).await?;
for col in &cols {
    println!("{} {} (pk={}, nullable={})", col.name, col.data_type, col.is_pk, col.nullable);
}

let indexes = SchemaInspector::indexes("users", &pool).await?;
let fks = SchemaInspector::foreign_keys("posts", &pool).await?;
```

`ColumnInfo` fields: `name`, `data_type`, `max_length`, `nullable`, `default`, `is_pk`.
`IndexInfo` fields: `name`, `columns`, `unique`, `primary`.
`ForeignKeyInfo` fields: `constraint_name`, `columns`, `foreign_table`, `foreign_columns`.

---

## Typed DSL Window Functions (`rok_fluent::dsl::window`) — feature: `query`

Window functions produce typed `WinExpr` values for use in SELECT projections via
[`SelectBuilder::win_col()`](crate::dsl::select::SelectBuilder::win_col).

### Standalone window functions

| Function | SQL |
|---|---|
| `rank()` | `RANK() OVER (…)` |
| `row_number()` | `ROW_NUMBER() OVER (…)` |
| `dense_rank()` | `DENSE_RANK() OVER (…)` |
| `ntile(n)` | `NTILE(n) OVER (…)` |

### Column-based window functions

| Method | SQL |
|---|---|
| `col.lag(offset)` | `LAG("col", offset) OVER (…)` |
| `col.lag_with_default(offset, default)` | `LAG("col", offset, default) OVER (…)` |
| `col.lead(offset)` | `LEAD("col", offset) OVER (…)` |
| `col.lead_with_default(offset, default)` | `LEAD("col", offset, default) OVER (…)` |
| `col.first_value()` | `FIRST_VALUE("col") OVER (…)` |
| `col.last_value()` | `LAST_VALUE("col") OVER (…)` |

### `Window` builder

```rust,ignore
use rok_fluent::dsl::{Window, rank, row_number};

let w = Window::new()
    .partition_by(Employee::DEPT)
    .order_by(Employee::SALARY.desc());

let query = db::select()
    .from(Employee::table())
    .columns([Employee::NAME, Employee::SALARY])
    .win_col(rank().over(&w).alias("dept_rank"))
    .win_col(row_number().over(Window::new().order_by(Employee::ID.asc())).alias("rn"));
```

See [`examples/09_window_functions_cte.rs`](../../examples/09_window_functions_cte.rs)
for a runnable version.

---

## `TypedJson<T>` column wrapper (`rok_fluent::orm::casts::TypedJson`) — feature: `postgres`

Deserializes a `jsonb` column into a typed Rust struct using native PostgreSQL jsonb encoding.

```rust,ignore
use rok_fluent::TypedJson;

#[derive(Debug, serde::Deserialize)]
struct Metadata { key: String, value: i64 }

#[derive(Debug, sqlx::FromRow, rok_fluent::Table)]
struct Product {
    id: i64,
    name: String,
    meta: TypedJson<Metadata>,
}
```

`TypedJson<T>` implements `sqlx::Type<Postgres>`, `Decode`, and `Encode` by delegating
to `sqlx::types::Json<T>`. It also implements `From<TypedJson<T>> for SqlValue` for use
in query builders.

---

## OrmLayer (`rok_fluent::orm::orm_layer`) — feature: `axum`

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

See [`examples/07_axum_integration.rs`](../../examples/07_axum_integration.rs) for a
runnable version.
