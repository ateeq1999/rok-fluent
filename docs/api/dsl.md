# DSL API (`feature = "query"`)

The typed query DSL lets you write SQL-like queries in Rust with full compile-time column
type checking.  Enable it with `features = ["query", "postgres"]` (or `sqlite`/`mysql`).

---

## Quick start

```toml
[dependencies]
rok-fluent = { version = "0.4", features = ["query", "postgres"] }
```

```rust
use rok_fluent::dsl::db;

#[derive(Debug, sqlx::FromRow, rok_fluent::TableDerive)]
#[table(name = "users")]
pub struct User {
    pub id:    i64,
    pub name:  String,
    pub email: String,
}
// Generates: User::table(), User::ID, User::NAME, User::EMAIL

// SELECT * FROM "users" WHERE "users"."id" = $1
let user: Option<User> = db::select()
    .from(User::table())
    .where_(User::ID.eq(42_i64))
    .fetch_optional::<User>(&pool)
    .await?;
```

---

## `#[derive(Table)]`

Attach to any `sqlx::FromRow` struct. The macro generates:

1. **Associated items on the struct** (primary OOP API):
   - `fn table() -> PostTable` — pass to `.from()`, `.insert_into()`, etc.
   - One `pub const FIELD_NAME: Column<Struct, FieldType>` per non-skipped field.

2. **`pub mod <table_name>` companion module** (secondary SQL-mirroring alias):
   - `pub const table: PostTable` — same as `Post::table()`
   - `pub const id: Column<Post, i64>` — same as `Post::ID`

### Struct-level attributes

| Attribute | Description |
|---|---|
| `#[table(name = "blog_posts")]` | Override the table name (default: struct name snake_case + "s") |
| `#[table(rename_all = "camelCase")]` | Auto-rename all constant names (Phase 34) |

### Field-level attributes

| Attribute | Description |
|---|---|
| `#[table(skip)]` | Exclude this field from column generation |
| `#[table(column = "col_name")]` | Override the SQL column name |
| `#[table(searchable)]` | Mark for `SearchService` full-text queries (Phase 34) |

### Example

```rust
#[derive(Debug, sqlx::FromRow, rok_fluent::TableDerive)]
#[table(name = "blog_posts")]
pub struct Post {
    pub id:         i64,
    pub title:      String,
    #[table(column = "body_text")]
    pub body:       String,
    #[table(skip)]
    pub cache_key:  String,  // not a DB column
}

// Generated OOP constants (primary):
// impl Post {
//     pub fn table() -> PostTable { PostTable }
//     pub const ID:    Column<Post, i64>    = Column::new("blog_posts", "id");
//     pub const TITLE: Column<Post, String> = Column::new("blog_posts", "title");
//     pub const BODY:  Column<Post, String> = Column::new("blog_posts", "body_text");
//     // cache_key is skipped
// }

// Generated module alias (secondary):
// pub mod blog_posts {
//     pub use super::PostTable as TableMarker;
//     pub const table: PostTable         = PostTable;
//     pub const id:    Column<Post, i64> = Post::ID;
//     pub const title: ...               = Post::TITLE;
//     pub const body:  ...               = Post::BODY;
// }
```

---

## Entry points (`db::`)

```rust
use rok_fluent::dsl::db;

db::select()                      // → SelectBuilder
db::insert_into(User::table())    // → InsertBuilder
db::update(User::table())         // → UpdateBuilder
db::delete_from(User::table())    // → DeleteBuilder
```

---

## `SelectBuilder`

```rust
db::select()
    .from(User::table())
    .columns([User::ID, User::NAME])      // restrict columns (default: SELECT *)
    .distinct()                           // SELECT DISTINCT
    .where_(expr)                         // WHERE — multiple calls are AND-ed
    .or_where(expr)                       // OR WHERE
    .order_by(User::NAME.asc())
    .order_by(User::CREATED_AT.desc())
    .limit(20)
    .offset(40)
    // Joins (Phase 22)
    .inner_join(Post::table(), Post::USER_ID.references(User::ID))
    .left_join(Post::table(), Post::USER_ID.references(User::ID))
    // Aggregation (Phase 24)
    .group_by([User::ROLE])
    .having(User::ID.count().gt(1_i64))
    // Locking
    .lock(Lock::ForUpdate)
    // Relationship loading (Phase 25)
    .with(User::POSTS)
    .with_paginated(User::POSTS, 1, 10)
```

### Terminals

| Method | SQL | Returns |
|---|---|---|
| `.fetch_all::<T>(&pool)` | `SELECT …` | `Result<Vec<T>>` |
| `.fetch_one::<T>(&pool)` | `SELECT … LIMIT 1` | `Result<T>` (RowNotFound if empty) |
| `.fetch_optional::<T>(&pool)` | `SELECT … LIMIT 1` | `Result<Option<T>>` |
| `.exists(&pool)` | `SELECT EXISTS (…)` | `Result<bool>` |
| `.count(&pool)` | `SELECT COUNT(*) …` | `Result<i64>` |
| `.paginate::<T>(page, per, &pool)` | offset pagination | `Result<Page<T>>` |
| `.simple_paginate::<T>(page, per, &pool)` | no total count | `Result<SimplePage<T>>` |
| `.cursor_paginate::<T>(col, cursor, per, &pool)` | cursor pagination | `Result<CursorPage<T>>` |

### SQL rendering (for testing / logging)

```rust
let (sql, params) = builder.to_sql_pg();    // $1, $2, … placeholders
let (sql, params) = builder.to_sql_qmark(); // ? placeholders (SQLite/MySQL)
```

---

## `InsertBuilder`

```rust
// Basic insert
db::insert_into(User::table())
    .values([("name", "Alice"), ("email", "alice@example.com")])
    .execute(&pool).await?;    // Result<u64>

// Typed values
db::insert_into(User::table())
    .values_typed([(User::NAME, "Bob"), (User::EMAIL, "bob@example.com")])
    .execute(&pool).await?;

// RETURNING *
let user: User = db::insert_into(User::table())
    .values([("name", "Carol"), ("email", "carol@example.com")])
    .returning()
    .fetch_one::<User>(&pool).await?;

// ON CONFLICT (upsert) — Phase 31
db::insert_into(User::table())
    .values_typed([(User::EMAIL, "a@b.com"), (User::NAME, "Alice")])
    .on_conflict(User::EMAIL).do_update([(User::NAME, "Alice")])
    .returning()
    .fetch_one::<User>(&pool).await?;

// ON CONFLICT DO NOTHING
db::insert_into(User::table())
    .values_typed([(User::EMAIL, "a@b.com")])
    .on_conflict(User::EMAIL).do_nothing()
    .execute(&pool).await?;
```

---

## `UpdateBuilder`

```rust
db::update(User::table())
    .set("name", "Carol")
    .set_typed(User::NAME, "Carol")         // typed alternative
    .set_raw("score = score + 1")           // raw SQL expression
    .where_(User::ID.eq(42_i64))
    .execute(&pool).await?;    // Result<u64>

// RETURNING (Phase 31)
let user: User = db::update(User::table())
    .set_typed(User::NAME, "Bob")
    .where_(User::ID.eq(42_i64))
    .returning()
    .fetch_one::<User>(&pool).await?;
```

---

## `DeleteBuilder`

```rust
db::delete_from(User::table())
    .where_(User::ID.eq(42_i64))
    .execute(&pool).await?;    // Result<u64>

// RETURNING (Phase 31)
db::delete_from(User::table())
    .where_(User::ACTIVE.eq(false))
    .returning_cols([User::ID, User::EMAIL])
    .fetch_all::<DeletedUser>(&pool).await?;
```

---

## `Column<T, V>` methods

All constants generated by `#[derive(Table)]` are `Column<StructType, FieldType>`.

### Comparison operators

| Method | SQL |
|---|---|
| `.eq(val)` | `col = $N` |
| `.ne(val)` | `col != $N` |
| `.gt(val)` | `col > $N` |
| `.gte(val)` | `col >= $N` |
| `.lt(val)` | `col < $N` |
| `.lte(val)` | `col <= $N` |
| `.between(lo, hi)` | `col BETWEEN $N AND $M` |
| `.like(pattern)` | `col LIKE $N` |
| `.not_like(pattern)` | `col NOT LIKE $N` |
| `.ilike(pattern)` | `col ILIKE $N` (PostgreSQL) |
| `.in_(vals)` | `col IN ($1, $2, …)` |
| `.not_in(vals)` | `col NOT IN ($1, $2, …)` |
| `.in_subquery(sel)` | `col IN (SELECT …)` |
| `.is_null()` | `col IS NULL` |
| `.is_not_null()` | `col IS NOT NULL` |
| `.eq_any(vals)` | `col = ANY($N)` (PostgreSQL) |
| `.references(other)` | `col = other_col` (JOIN ON) |

### Ordering

| Method | Returns |
|---|---|
| `.asc()` | `OrderExpr` — pass to `.order_by()` |
| `.desc()` | `OrderExpr` |
| `.asc_nulls_last()` | `OrderExpr` |
| `.desc_nulls_last()` | `OrderExpr` |

### Aggregates (Phase 24) — returns `AggExpr`

| Method | SQL |
|---|---|
| `.count()` | `COUNT(col)` |
| `.count_distinct()` | `COUNT(DISTINCT col)` |
| `.sum()` | `SUM(col)` |
| `.avg()` | `AVG(col)` |
| `.min()` | `MIN(col)` |
| `.max()` | `MAX(col)` |
| `.array_agg()` | `ARRAY_AGG(col)` |

### String / math / date helpers (Phase 29)

```rust
User::NAME.lower()                    // LOWER(name)
User::SCORE.abs()                     // ABS(score)
User::CREATED_AT.date_trunc("month")  // DATE_TRUNC('month', created_at)
User::ID.cast::<String>()             // id::text
```

### Column-to-column equality (for JOINs — Phase 22)

```rust
Post::USER_ID.references(User::ID)   // "posts"."user_id" = "users"."id"
Post::USER_ID.eq_col(User::ID)       // same — for WHERE context
```

---

## `Expr` — composable boolean expressions

```rust
use rok_fluent::dsl::Expr;

// .and() / .or() / !
let expr = User::EMAIL.like("%@example.com")
    .and(User::ID.gt(0_i64))
    .or(User::NAME.is_not_null());

let not_deleted = !Post::DELETED_AT.is_not_null();

// Nested
let complex = (User::ID.gt(10_i64).and(User::ID.lt(100_i64)))
    .or(User::EMAIL.eq("admin@example.com"));

// EXISTS subquery (Phase 30)
let has_posts = Expr::exists(
    db::select().from(Post::table()).where_(Post::USER_ID.references(User::ID))
);

// CASE WHEN (Phase 29)
let label = Expr::case()
    .when(User::ACTIVE.eq(true), "active")
    .when(User::BANNED.eq(true), "banned")
    .otherwise("inactive");

// Raw SQL escape hatch (Phase 34)
let raw = Expr::raw("score > salary * 0.1");
```

Rendered SQL example:

```sql
(("users"."id" > $1 AND "users"."id" < $2) OR "users"."email" = $3)
```

---

## Pagination (Phase 23)

```rust
use rok_fluent::orm::pagination::{Page, CursorPage};

// Offset pagination
let page: Page<User> = db::select()
    .from(User::table())
    .where_(User::ACTIVE.eq(true))
    .order_by(User::CREATED_AT.desc())
    .paginate::<User>(1, 25, &pool).await?;

println!("{} total, page {}/{}", page.total, page.current_page, page.last_page);

// Cursor pagination (stable for infinite scroll)
let page: CursorPage<User> = db::select()
    .from(User::table())
    .cursor_paginate::<User>(User::ID, cursor_str, 25, &pool).await?;
```

---

## Aggregation (Phase 24)

```rust
// COUNT, SUM, GROUP BY, HAVING
let stats = db::select()
    .from(Order::table())
    .columns([Order::USER_ID, Order::TOTAL.sum().alias("total_spend")])
    .group_by([Order::USER_ID])
    .having(Order::TOTAL.sum().gte(1000_i64))
    .fetch_all::<UserStats>(&pool).await?;

// Scalar aggregate
let n: i64 = db::select().from(User::table()).count(&pool).await?;
```

---

## Relationship loading (Phase 25)

```rust
#[derive(Debug, sqlx::FromRow, rok_fluent::TableDerive)]
#[table(name = "users")]
pub struct User {
    pub id:   i64,
    pub name: String,

    #[table(has_many = Post, fk = "user_id")]
    pub posts: Loaded<Vec<Post>>,

    #[table(belongs_to = Organization, fk = "org_id")]
    pub organization: Loaded<Option<Organization>>,
}

// Batch load (no N+1)
let users: Vec<User> = db::select()
    .from(User::table())
    .with(User::POSTS)
    .with(User::ORGANIZATION)
    .fetch_all(&pool).await?;

// Paginated relationship loading (Phase 26)
let users: Vec<User> = db::select()
    .from(User::table())
    .with_paginated(User::POSTS, 1, 10)    // first 10 posts per user
    .fetch_all(&pool).await?;

for user in &users {
    for post in user.posts.get().unwrap() {
        println!("{}: {}", user.name, post.title);
    }
}
```

See [plan.md](../../plan.md) §7 for all 11 relationship types.

---

## Combining DSL with Active Record (requires `active` + `query`)

```rust
// Typed Expr inside Active Record:
let posts = Post::query()
    .and_expr(Post::USER_ID.eq(42_i64).and(Post::PUBLISHED.eq(true)))
    .all().await?;

// Convert ModelQuery → SelectBuilder (Phase 33):
let posts: Vec<Post> = Post::query()
    .where_eq("user_id", 42_i64)
    .into_dsl()
    .order_by(Post::CREATED_AT.desc())
    .fetch_all(&pool).await?;
```

---

## `SqlValue` types

`Column<T, V>` accepts any type implementing `Into<SqlValue>`.

| Rust type | `SqlValue` variant | PostgreSQL binding |
|---|---|---|
| `&str`, `String` | `Text(String)` | `text` |
| integer types | `Integer(i64)` | `int8` |
| float types | `Float(f64)` | `float8` |
| `bool` | `Bool(bool)` | `bool` |
| `serde_json::Value` | `Json(Value)` | `jsonb` |
| `uuid::Uuid` | `Uuid(Uuid)` | `uuid` |
| `Option<T>` | `Null` / inner | `NULL` / inner |
