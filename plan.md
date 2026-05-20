# rok-fluent — Architecture & Roadmap

## 1. Project Overview

**rok-fluent** is a feature-rich async ORM for Rust targeting PostgreSQL, MySQL, and SQLite via
SQLx.  It ships two composable query styles behind independent feature flags:

| Style | Flag | Inspiration | Best for |
|---|---|---|---|
| Active Record | `active` | Laravel Eloquent | CRUD-heavy apps, clean model methods |
| Typed DSL | `query` | ActiveRecord + Drizzle | Complex queries, joins, type safety, composable logic |

Both styles share the same `SqlValue`, `Model` trait, pagination types, migration system,
factory helpers, and CRUD service infrastructure.

Users can opt into one, both, or neither and still get core + migrations.

---

## 2. DSL API Design — `User::table()` OOP Style (Primary)

### Decision: OOP struct-associated style is the primary API

The DSL is designed around struct-associated constants and methods so the type system is always
front-and-center. You always know which model a column belongs to without needing to look at the
surrounding module namespace.

```rust
#[derive(Debug, sqlx::FromRow, Table)]
#[table(name = "users")]
pub struct User {
    pub id:    i64,
    pub name:  String,
    pub email: String,
}
```

**What `#[derive(Table)]` generates:**

```rust
// ── PRIMARY: struct-associated (OOP style) ───────────────────────────────────

impl User {
    /// Table marker for use in `.from()`, `.insert_into()`, etc.
    pub fn table() -> UserTable { UserTable }

    /// Typed column constants — SCREAMING_SNAKE_CASE per Rust convention.
    pub const ID:    ::rok_fluent::dsl::Column<User, i64>    = Column::new("users", "id");
    pub const NAME:  ::rok_fluent::dsl::Column<User, String> = Column::new("users", "name");
    pub const EMAIL: ::rok_fluent::dsl::Column<User, String> = Column::new("users", "email");
}

#[derive(Debug, Clone, Copy)]
pub struct UserTable;
impl ::rok_fluent::dsl::Table for UserTable {
    fn table_name() -> &'static str { "users" }
}

// ── SECONDARY: module alias (for SQL-mirroring multi-table queries) ──────────

pub mod users {
    pub use super::{UserTable as TableMarker};
    pub const table: UserTable = UserTable;
    pub const id:    ::rok_fluent::dsl::Column<super::User, i64>    = super::User::ID;
    pub const name:  ::rok_fluent::dsl::Column<super::User, String> = super::User::NAME;
    pub const email: ::rok_fluent::dsl::Column<super::User, String> = super::User::EMAIL;
}
```

**Usage — OOP style (primary):**

```rust
// Single-table
let user: Option<User> = db::select()
    .from(User::table())
    .where_(User::ID.eq(42_i64))
    .fetch_optional::<User>(&pool).await?;

// Multi-table join — type still clear from the column struct prefix
db::select()
    .from(User::table())
    .left_join(Post::table(), Post::USER_ID.references(User::ID))
    .where_(User::ID.gt(0_i64).and(Post::PUBLISHED.eq(true)))
    .fetch_all::<UserWithPost>(&pool).await?;

// Order, limit, offset
db::select()
    .from(User::table())
    .where_(User::EMAIL.like("%@example.com"))
    .order_by(User::NAME.asc())
    .limit(25)
    .offset(50)
    .fetch_all::<User>(&pool).await?;
```

**Usage — module alias (secondary, still supported):**

```rust
db::select().from(users::table).where_(users::id.eq(42_i64))
```

Both compile to identical SQL. The OOP style is the canonical API going forward.

---

## 3. Full DSL Feature Surface

### 3.1 Entry points (`db::`)

```rust
use rok_fluent::dsl::db;

db::select()                      // → SelectBuilder
db::insert_into(User::table())    // → InsertBuilder
db::update(User::table())         // → UpdateBuilder
db::delete_from(User::table())    // → DeleteBuilder
```

### 3.2 `SelectBuilder` — complete API

```rust
// ── Source ────────────────────────────────────────────────────────────────────
.from(User::table())
.from_subquery(sub_select, "alias")        // FROM (SELECT …) AS alias

// ── Projection ────────────────────────────────────────────────────────────────
.columns([User::ID, User::NAME])           // SELECT col, col (default: SELECT *)
.column_raw("COALESCE(name, email)")       // raw SQL column expression
.distinct()                                // SELECT DISTINCT
.distinct_on([User::EMAIL])                // SELECT DISTINCT ON (col, …) — PostgreSQL

// ── Filtering ─────────────────────────────────────────────────────────────────
.where_(expr)                              // WHERE — multiple calls are AND-ed
.where_raw("score > 100")                  // raw SQL predicate
.or_where(expr)                            // OR WHERE

// ── Joins ─────────────────────────────────────────────────────────────────────
.inner_join(Post::table(), Post::USER_ID.references(User::ID))
.left_join(Post::table(), Post::USER_ID.references(User::ID))
.right_join(Post::table(), Post::USER_ID.references(User::ID))
.cross_join(Tag::table())

// ── Sorting ───────────────────────────────────────────────────────────────────
.order_by(User::NAME.asc())
.order_by(User::CREATED_AT.desc())
.order_by_raw("LOWER(name) ASC NULLS LAST")

// ── Pagination ────────────────────────────────────────────────────────────────
.limit(25)
.offset(50)

// ── Grouping ──────────────────────────────────────────────────────────────────
.group_by([User::ROLE])
.having(User::ID.count().gt(1_i64))

// ── Locking ───────────────────────────────────────────────────────────────────
.lock(Lock::ForUpdate)
.lock(Lock::ForShare)
.lock(Lock::SkipLocked)

// ── CTEs ──────────────────────────────────────────────────────────────────────
.with_cte("active_users", db::select().from(User::table()).where_(User::ACTIVE.eq(true)))
.with_recursive_cte("tree", base_query, recursive_query)

// ── Set operations ────────────────────────────────────────────────────────────
.union(other_select)
.union_all(other_select)
.intersect(other_select)
.except(other_select)

// ── Relationship loading ──────────────────────────────────────────────────────
.with(User::POSTS)                         // batch load has_many
.with(User::PROFILE)                       // batch load has_one
.with_paginated(User::POSTS, page, per)    // paginated has_many
.with_cursor(User::POSTS, cursor, per)     // cursor has_many

// ── Terminals (PostgreSQL) ────────────────────────────────────────────────────
.fetch_all::<User>(&pool).await?           // Vec<User>
.fetch_one::<User>(&pool).await?           // User  (RowNotFound if empty)
.fetch_optional::<User>(&pool).await?      // Option<User>
.exists(&pool).await?                      // bool
.count(&pool).await?                       // i64
.paginate::<User>(page, per, &pool).await?           // Page<User>
.simple_paginate::<User>(page, per, &pool).await?    // SimplePage<User>
.cursor_paginate::<User>(col, cursor, per, &pool).await? // CursorPage<User>

// ── SQL rendering (testing / logging) ────────────────────────────────────────
.to_sql_pg()     // → (String, Vec<SqlValue>)  — $1 $2 placeholders
.to_sql_qmark()  // → (String, Vec<SqlValue>)  — ? placeholders
```

### 3.3 `InsertBuilder` — complete API

```rust
db::insert_into(User::table())
    .values([("name", "Alice"), ("email", "alice@example.com")])
    .values_typed([(User::NAME, "Alice"), (User::EMAIL, "alice@example.com")])  // typed
    .on_conflict(User::EMAIL).do_nothing()                       // INSERT … ON CONFLICT DO NOTHING
    .on_conflict(User::EMAIL).do_update([("name", "Alice")])     // ON CONFLICT DO UPDATE SET …
    .on_conflict_typed(User::EMAIL).do_update_typed([(User::NAME, "Alice")])
    .returning()                                                 // RETURNING *
    .returning_cols([User::ID, User::NAME])                      // RETURNING id, name
    .execute(&pool).await?                                       // Result<u64>
    .fetch_one::<User>(&pool).await?                             // Result<User>  (needs .returning())
    .fetch_all::<User>(&pool).await?                             // Result<Vec<User>>
```

### 3.4 `UpdateBuilder` — complete API

```rust
db::update(User::table())
    .set("name", "Carol")
    .set_typed(User::NAME, "Carol")                              // typed
    .set_raw("score = score + 1")                               // raw SQL expression
    .where_(User::ID.eq(42_i64))
    .returning()
    .returning_cols([User::ID, User::UPDATED_AT])
    .execute(&pool).await?                                       // Result<u64>
    .fetch_one::<User>(&pool).await?                             // Result<User>
    .fetch_all::<User>(&pool).await?                             // Result<Vec<User>>
```

### 3.5 `DeleteBuilder` — complete API

```rust
db::delete_from(User::table())
    .where_(User::ID.eq(42_i64))
    .returning()
    .returning_cols([User::ID])
    .execute(&pool).await?                                       // Result<u64>
    .fetch_one::<User>(&pool).await?
```

---

## 4. `Column<T, V>` — full operator surface

### 4.1 Comparison

| Method | SQL |
|---|---|
| `.eq(val)` | `col = $N` |
| `.ne(val)` | `col != $N` |
| `.gt(val)` | `col > $N` |
| `.gte(val)` | `col >= $N` |
| `.lt(val)` | `col < $N` |
| `.lte(val)` | `col <= $N` |
| `.between(lo, hi)` | `col BETWEEN $N AND $M` |
| `.not_between(lo, hi)` | `col NOT BETWEEN $N AND $M` |
| `.like(pat)` | `col LIKE $N` |
| `.not_like(pat)` | `col NOT LIKE $N` |
| `.ilike(pat)` | `col ILIKE $N` (PostgreSQL case-insensitive) |
| `.in_(vals)` | `col IN ($1, $2, …)` |
| `.not_in(vals)` | `col NOT IN ($1, $2, …)` |
| `.in_subquery(sel)` | `col IN (SELECT …)` |
| `.not_in_subquery(sel)` | `col NOT IN (SELECT …)` |
| `.is_null()` | `col IS NULL` |
| `.is_not_null()` | `col IS NOT NULL` |
| `.eq_any(vals)` | `col = ANY($N)` — PostgreSQL array |
| `.references(other_col)` | `self = other` — column-to-column equality for JOINs |
| `.eq_col(other_col)` | same as `.references()` — `WHERE` context alias |

### 4.2 Ordering

| Method | Returns |
|---|---|
| `.asc()` | `OrderExpr` |
| `.desc()` | `OrderExpr` |
| `.asc_nulls_first()` | `OrderExpr` |
| `.asc_nulls_last()` | `OrderExpr` |
| `.desc_nulls_first()` | `OrderExpr` |
| `.desc_nulls_last()` | `OrderExpr` |

### 4.3 Aggregates — returns `AggExpr` for use in `.columns()` or `.having()`

| Method | SQL |
|---|---|
| `.count()` | `COUNT(col)` |
| `.count_distinct()` | `COUNT(DISTINCT col)` |
| `.sum()` | `SUM(col)` |
| `.avg()` | `AVG(col)` |
| `.min()` | `MIN(col)` |
| `.max()` | `MAX(col)` |
| `.array_agg()` | `ARRAY_AGG(col)` — PostgreSQL |
| `.string_agg(delim)` | `STRING_AGG(col, delim)` — PostgreSQL |
| `.json_agg()` | `JSON_AGG(col)` — PostgreSQL |

`AggExpr` implements the same comparison operators as `Column<T,V>` for use in `HAVING`.

### 4.4 JSON operators (PostgreSQL `jsonb` columns)

| Method | SQL |
|---|---|
| `.json_get(key)` | `col -> 'key'` |
| `.json_get_text(key)` | `col ->> 'key'` |
| `.json_path(path)` | `col #>> '{a,b}'` |
| `.json_contains(val)` | `col @> $N` |
| `.json_contained_by(val)` | `col <@ $N` |
| `.json_has_key(key)` | `col ? 'key'` |
| `.json_has_all(keys)` | `col ?& ARRAY[…]` |
| `.json_has_any(keys)` | `col ?\| ARRAY[…]` |

### 4.5 String functions — returns `FnExpr` usable as a column expression

| Method | SQL |
|---|---|
| `.lower()` | `LOWER(col)` |
| `.upper()` | `UPPER(col)` |
| `.trim()` | `TRIM(col)` |
| `.length()` | `LENGTH(col)` |
| `.concat(other)` | `col \|\| other` |
| `.starts_with(prefix)` | `col LIKE '$N%'` |
| `.ends_with(suffix)` | `col LIKE '%$N'` |

### 4.6 Math functions

| Method | SQL |
|---|---|
| `.abs()` | `ABS(col)` |
| `.round(n)` | `ROUND(col, n)` |
| `.ceil()` | `CEIL(col)` |
| `.floor()` | `FLOOR(col)` |
| `.mod_(n)` | `MOD(col, n)` |
| `.power(n)` | `POWER(col, n)` |

### 4.7 Date/time functions

| Method | SQL |
|---|---|
| `.date_trunc(unit)` | `DATE_TRUNC('unit', col)` |
| `.extract(part)` | `EXTRACT(part FROM col)` |
| `.age()` | `AGE(col)` — PostgreSQL |
| `.to_char(fmt)` | `TO_CHAR(col, 'fmt')` |

### 4.8 Type casting

```rust
User::SCORE.cast::<f64>()      // col::float8
User::ID.cast::<String>()      // col::text
```

### 4.9 Window functions

```rust
User::SCORE.rank().over(Window::new()
    .partition_by(User::DEPT)
    .order_by(User::SCORE.desc()))
// → RANK() OVER (PARTITION BY dept ORDER BY score DESC)

User::SALARY.row_number().over(Window::new().partition_by(User::DEPT))
User::SALARY.dense_rank().over(...)
User::SALARY.lag(1).over(...)
User::SALARY.lead(1).over(...)
User::SALARY.first_value().over(...)
User::SALARY.last_value().over(...)
User::SALARY.nth_value(3).over(...)
```

---

## 5. `Expr` — composable boolean tree

```rust
use rok_fluent::dsl::Expr;

// Combine
let expr = User::EMAIL.like("%@example.com")
    .and(User::ID.gt(0_i64))
    .or(User::NAME.is_not_null());

// Negate
let not_deleted = !Post::DELETED_AT.is_not_null();

// Nest freely
let complex = (User::ID.gt(10_i64).and(User::ID.lt(100_i64)))
    .or(User::EMAIL.eq("admin@example.com"));

// EXISTS subquery
let has_posts = Expr::exists(
    db::select().from(Post::table()).where_(Post::USER_ID.references(User::ID))
);

// CASE WHEN
let status = Expr::case()
    .when(User::ACTIVE.eq(true), "active")
    .when(User::BANNED.eq(true), "banned")
    .otherwise("inactive");

// Raw SQL escape hatch
let raw = Expr::raw("score > salary * 0.1");
```

---

## 6. Pagination types

All pagination types are in `rok_fluent::orm::pagination` — shared by both DSL and Active Record.

```rust
// Offset pagination
pub struct Page<T> {
    pub data:         Vec<T>,
    pub total:        u64,
    pub per_page:     u64,
    pub current_page: u64,
    pub last_page:    u64,
    pub from:         u64,   // first row index (1-based) on this page
    pub to:           u64,   // last row index (1-based) on this page
}

// Simplified offset (no total count query)
pub struct SimplePage<T> {
    pub data:         Vec<T>,
    pub per_page:     u64,
    pub current_page: u64,
    pub has_more:     bool,
}

// Cursor pagination (stable, efficient for infinite scroll)
pub struct CursorPage<T> {
    pub data:        Vec<T>,
    pub next_cursor: Option<String>,   // opaque base64-encoded cursor
    pub prev_cursor: Option<String>,
    pub per_page:    u64,
}
```

DSL pagination on `SelectBuilder`:

```rust
// Offset
let page: Page<User> = db::select()
    .from(User::table())
    .where_(User::ACTIVE.eq(true))
    .order_by(User::CREATED_AT.desc())
    .paginate::<User>(1, 25, &pool).await?;

// Cursor
let page: CursorPage<User> = db::select()
    .from(User::table())
    .where_(User::ACTIVE.eq(true))
    .cursor_paginate::<User>(User::ID, cursor_str, 25, &pool).await?;
```

Active Record pagination (unchanged):

```rust
let page: Page<User> = User::query()
    .where_eq("active", true)
    .paginate(1, 25).await?;

let page: CursorPage<User> = User::query()
    .cursor_paginate("id", cursor_str, 25).await?;
```

---

## 7. Relationship System

### 7.1 `Loaded<T>`

```rust
pub enum Loaded<T> {
    NotLoaded,
    Loaded(T),
}

impl<T> Loaded<T> {
    pub fn get(&self) -> Option<&T>;
    pub fn unwrap(self) -> T;
    pub fn is_loaded(&self) -> bool;
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Loaded<U>;
}
```

### 7.2 Relationship annotations

```rust
#[derive(Debug, sqlx::FromRow, Table)]
#[table(name = "users")]
pub struct User {
    pub id:   i64,
    pub name: String,

    // 1:1 — FK on related table
    #[table(has_one = Profile, fk = "user_id")]
    pub profile: Loaded<Option<Profile>>,

    // 1:M — FK on related table
    #[table(has_many = Post, fk = "user_id")]
    pub posts: Loaded<Vec<Post>>,

    // M:1 — FK on this table
    #[table(belongs_to = Organization, fk = "org_id")]
    pub organization: Loaded<Option<Organization>>,

    // M:M — through pivot table
    #[table(many_to_many = Role, pivot = "role_user", local_fk = "user_id", foreign_fk = "role_id")]
    pub roles: Loaded<Vec<Role>>,

    // Has many through (two hops)
    #[table(has_many_through = Tag, through = Post, local_fk = "user_id", foreign_fk = "post_id")]
    pub tags: Loaded<Vec<Tag>>,

    // Has one through
    #[table(has_one_through = Country, through = Organization, local_fk = "org_id", foreign_fk = "country_id")]
    pub country: Loaded<Option<Country>>,

    // Belongs to through
    #[table(belongs_to_through = Organization, through = Team, local_fk = "team_id", foreign_fk = "org_id")]
    pub org_via_team: Loaded<Option<Organization>>,

    // Polymorphic 1:1 (this model owns imageable_id + imageable_type)
    #[table(morph_one = Image, as_type = "imageable")]
    pub cover: Loaded<Option<Image>>,

    // Polymorphic 1:M
    #[table(morph_many = Comment, as_type = "commentable")]
    pub comments: Loaded<Vec<Comment>>,

    // Inverse polymorphic (fields: commentable_type, commentable_id)
    #[table(morph_to, type_col = "commentable_type", id_col = "commentable_id")]
    pub commentable: Loaded<MorphTarget>,

    // Polymorphic M:M
    #[table(morph_to_many = Tag, pivot = "taggables", as_type = "taggable",
            id_col = "taggable_id", type_col = "taggable_type")]
    pub tags_poly: Loaded<Vec<Tag>>,
}
```

### 7.3 All 11 relationship types

| Type | Annotation | FK location | SQL pattern |
|---|---|---|---|
| **Has One** | `has_one = T, fk = "…"` | related table | `WHERE t.fk = self.pk LIMIT 1` |
| **Has Many** | `has_many = T, fk = "…"` | related table | `WHERE t.fk IN (…)` |
| **Belongs To** | `belongs_to = T, fk = "…"` | this table | `WHERE t.pk = self.fk` |
| **Many To Many** | `many_to_many = T, pivot = "…"` | pivot table | `JOIN pivot JOIN t` |
| **Has One Through** | `has_one_through = T, through = U` | intermediate model | double join |
| **Has Many Through** | `has_many_through = T, through = U` | intermediate model | double join |
| **Belongs To Through** | `belongs_to_through = T, through = U` | intermediate | double join |
| **Morph One** | `morph_one = T, as_type = "…"` | related table | `WHERE type='X' AND id=pk` |
| **Morph Many** | `morph_many = T, as_type = "…"` | related table | batch IN + type filter |
| **Morph To** | `morph_to, type_col, id_col` | this table | dynamic dispatch |
| **Morph To Many** | `morph_to_many = T, pivot = "…"` | poly pivot | JOIN with type filter |

### 7.4 Loading relationships — three approaches

```rust
// ── Approach A: .with() on SelectBuilder — batch load, no N+1 ─────────────────
let users: Vec<User> = db::select()
    .from(User::table())
    .with(User::POSTS)                // SELECT * FROM posts WHERE user_id IN (…)
    .with(User::PROFILE)
    .with(User::ROLES)
    .fetch_all(&pool).await?;

// ── Approach B: paginated relationship loading ───────────────────────────────
let users: Vec<User> = db::select()
    .from(User::table())
    .with_paginated(User::POSTS, 1, 10)    // loads first 10 posts per user
    .with_cursor(User::COMMENTS, cursor, 20)
    .fetch_all(&pool).await?;

// each user.posts → Loaded::Loaded([first 10 posts])

// ── Approach C: JOIN builder ──────────────────────────────────────────────────
let rows: Vec<(User, Option<Post>)> = db::select()
    .from(User::table())
    .left_join(Post::table(), Post::USER_ID.references(User::ID))
    .fetch_all(&pool).await?;

// ── Approach D: after-fetch include (lazy) ────────────────────────────────────
let mut users = db::select().from(User::table()).fetch_all::<User>(&pool).await?;
users.include(User::POSTS, &pool).await?;
users.include_paginated(User::COMMENTS, 1, 20, &pool).await?;
```

---

## 8. Built-in CRUD Service

`CrudService<M>` is a generic zero-boilerplate service layer for any model that implements
`PgModel` (or `SqliteModel` / `MySqlModel`). It wraps the most common CRUD patterns — find,
list, create, update, delete, paginate, search — in a single re-usable struct.

Feature gate: `active` + database feature.

```rust
use rok_fluent::services::CrudService;

// Create a service for User (zero-cost wrapper)
let svc = CrudService::<User>::new(&pool);

// ── Read ──────────────────────────────────────────────────────────────────────
let user: Option<User>  = svc.find(1_i64).await?;
let user: User          = svc.find_or_fail(1_i64).await?;
let users: Vec<User>    = svc.all().await?;
let page: Page<User>    = svc.paginate(1, 25).await?;
let page: CursorPage<User> = svc.cursor_paginate("id", cursor, 25).await?;
let n: i64              = svc.count().await?;
let exists: bool        = svc.exists(1_i64).await?;

// ── Scoped read ───────────────────────────────────────────────────────────────
let users = svc.query()                     // returns ModelQuery<User>
    .where_eq("active", true)
    .order_by_desc("created_at")
    .all().await?;

// ── Write ─────────────────────────────────────────────────────────────────────
let user: User  = svc.create(&[("name", "Alice".into()), ("email", "a@b.com".into())]).await?;
let n: u64      = svc.update(1_i64, &[("name", "Bob".into())]).await?;
let n: u64      = svc.delete(1_i64).await?;
let n: u64      = svc.soft_delete(1_i64).await?;   // sets deleted_at
let n: u64      = svc.restore(1_i64).await?;        // clears deleted_at

// ── Bulk ──────────────────────────────────────────────────────────────────────
let count: u64  = svc.bulk_create(&[row1, row2, row3]).await?;
let n: u64      = svc.delete_where(&[("active", false.into())]).await?;

// ── Upsert ────────────────────────────────────────────────────────────────────
let user: User = svc.upsert_by("email", &[("email", "a@b.com".into()), ("name", "Alice".into())]).await?;

// ── Search ────────────────────────────────────────────────────────────────────
// Full-text search across the columns declared with #[table(searchable)]
let users: Vec<User> = svc.search("alice").await?;
let page: Page<User> = svc.search_paginated("alice", 1, 25).await?;

// ── Relationship loading ──────────────────────────────────────────────────────
let users: Vec<User> = svc.all_with([User::POSTS, User::PROFILE]).await?;
let page: Page<User> = svc.paginate_with(1, 25, [User::POSTS]).await?;
```

### 8.1 Custom service via extension trait

```rust
// Extend CrudService with domain-specific methods:
trait UserService {
    async fn find_by_email(&self, email: &str) -> Result<Option<User>>;
    async fn activate(&self, id: i64) -> Result<User>;
}

impl UserService for CrudService<User> {
    async fn find_by_email(&self, email: &str) -> Result<Option<User>> {
        self.query().where_eq("email", email).first().await
    }
    async fn activate(&self, id: i64) -> Result<User> {
        self.update(id, &[("active", true.into()), ("activated_at", Utc::now().into())]).await?;
        self.find_or_fail(id).await
    }
}
```

---

## 9. Query Helper Services

### 9.1 `FilterBuilder<M>` — reusable, composable filter sets

```rust
use rok_fluent::services::FilterBuilder;

let filter = FilterBuilder::<User>::new()
    .eq("active", true)
    .like("email", "%@example.com")
    .gt("created_at", cutoff)
    .in_("role", vec!["admin".into(), "editor".into()])
    .build();   // → ModelQuery<User>

let users = filter.order_by_desc("created_at").paginate(1, 25).await?;
```

### 9.2 `SortBuilder<M>` — declarative, user-driven sorting

```rust
use rok_fluent::services::SortBuilder;

// Accept sort params from query string (?sort=name&dir=asc)
let sort = SortBuilder::<User>::new()
    .allow(["name", "email", "created_at"])     // whitelist
    .default(User::CREATED_AT.desc())
    .from_params(sort_field, sort_dir)?;        // validates against allowlist

let users = User::query().apply_sort(&sort).paginate(1, 25).await?;
```

### 9.3 `SearchService<M>` — full-text search

```rust
use rok_fluent::services::SearchService;

// Mark columns searchable in the struct:
#[derive(Debug, sqlx::FromRow, Table)]
pub struct User {
    pub id:    i64,
    #[table(searchable)]
    pub name:  String,
    #[table(searchable)]
    pub email: String,
    pub active: bool,
}

let results = SearchService::<User>::search("alice")
    .where_eq("active", true)
    .paginate(1, 25, &pool).await?;

// PostgreSQL: uses tsvector / GIN index when available
// MySQL / SQLite: falls back to LIKE OR … LIKE
```

### 9.4 `AuditService<M>` — automatic created_at / updated_at

Enabled automatically when the model has `#[model(timestamps)]`.

```rust
// On create: sets created_at = NOW(), updated_at = NOW()
// On update: sets updated_at = NOW()
// Manual override available:
AuditService::<Post>::touch(42_i64, &pool).await?;   // update updated_at = NOW()
AuditService::<Post>::history(42_i64, &pool).await?; // read audit_log if present
```

### 9.5 `SoftDeleteService<M>` — scoped queries for soft-deleted rows

Enabled automatically when the model has `#[model(soft_delete)]`.

```rust
use rok_fluent::services::SoftDeleteService;

let active  = SoftDeleteService::<Post>::all_active(&pool).await?;   // WHERE deleted_at IS NULL
let deleted = SoftDeleteService::<Post>::all_deleted(&pool).await?;  // WHERE deleted_at IS NOT NULL
let with_trashed = SoftDeleteService::<Post>::with_trashed(&pool).await?; // all rows

SoftDeleteService::<Post>::soft_delete(42_i64, &pool).await?;
SoftDeleteService::<Post>::restore(42_i64, &pool).await?;
SoftDeleteService::<Post>::force_delete(42_i64, &pool).await?;      // hard delete
SoftDeleteService::<Post>::purge_deleted(&pool).await?;             // delete all with deleted_at set
```

### 9.6 `BatchService<M>` — efficient multi-row operations

```rust
use rok_fluent::services::BatchService;

// Bulk insert with chunking (avoids PG parameter limit)
BatchService::<User>::bulk_insert(&rows, &pool).await?;
BatchService::<User>::bulk_insert_chunked(&rows, 500, &pool).await?;  // chunk of 500

// Bulk update
BatchService::<User>::bulk_update(
    &[("active", false.into())],
    &[1_i64, 2_i64, 3_i64],   // IDs
    &pool,
).await?;

// Bulk upsert
BatchService::<User>::bulk_upsert_by("email", &rows, &pool).await?;
```

---

## 10. Directory Layout (current)

```
rok-fluent/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── LICENSE
├── .github/
│   └── workflows/
│       ├── ci.yml           # test on every push/PR
│       └── publish.yml      # auto-publish when version bumps
├── rok-fluent-macros/
│   ├── Cargo.toml
│   └── src/lib.rs           # Model, Table, Resource, Seed derives
└── src/
    ├── lib.rs
    ├── core/
    │   ├── condition.rs     # SqlValue, Condition
    │   ├── model.rs         # Model trait
    │   ├── query.rs         # QueryBuilder<T>
    │   ├── replica.rs
    │   ├── schema_cache.rs
    │   ├── tenant.rs
    │   └── sqlx/{pg,sqlite,mysql}.rs
    ├── dsl/                 # feature = "query"
    │   ├── mod.rs
    │   ├── column.rs        # Column<T,V>, AggExpr, FnExpr, WindowExpr
    │   ├── db.rs            # entry points
    │   ├── delete.rs
    │   ├── expr.rs          # Expr tree + CASE WHEN + Expr::raw
    │   ├── insert.rs        # + on_conflict builder
    │   ├── loaded.rs        # Loaded<T>
    │   ├── select.rs        # + JOIN, GROUP BY, HAVING, CTEs, set ops, pagination
    │   ├── table.rs         # Table trait
    │   ├── update.rs        # + RETURNING
    │   └── window.rs        # Window, WindowDef
    ├── orm/
    │   ├── model_query.rs   # ModelQuery<M>           [active]
    │   ├── morph.rs
    │   ├── eager.rs
    │   ├── through.rs
    │   ├── scopes.rs        # [active]
    │   ├── hooks.rs
    │   ├── pagination.rs    # Page, SimplePage, CursorPage
    │   ├── collection.rs
    │   ├── casts.rs
    │   ├── n1.rs
    │   ├── resource.rs
    │   ├── orm_layer.rs     # [axum]
    │   ├── postgres/{executor,model,pool,transaction,pivot_query,query_log}.rs
    │   ├── mysql/{executor,model}.rs
    │   └── sqlite/{executor,model}.rs
    ├── services/            # feature = "active" (NEW)
    │   ├── mod.rs
    │   ├── crud.rs          # CrudService<M>
    │   ├── filter.rs        # FilterBuilder<M>
    │   ├── sort.rs          # SortBuilder<M>
    │   ├── search.rs        # SearchService<M>
    │   ├── audit.rs         # AuditService<M>
    │   ├── soft_delete.rs   # SoftDeleteService<M>
    │   └── batch.rs         # BatchService<M>
    ├── factory/
    └── migrate/
```

---

## 11. Feature Flag Taxonomy

```toml
[features]
default = ["macros"]

macros   = ["dep:rok-fluent-macros"]
active   = []        # Active Record style + Services
query    = []        # Typed DSL

postgres = ["dep:sqlx", "sqlx/postgres", "dep:tokio", "tokio/rt",
            "dep:futures-core", "dep:futures", "dep:dashmap"]
sqlite   = ["dep:sqlx", "sqlx/sqlite",  "dep:tokio", "tokio/rt"]
mysql    = ["dep:sqlx", "sqlx/mysql",   "dep:tokio", "tokio/rt"]

axum     = ["dep:axum", "dep:tower", "postgres"]
tracing  = ["dep:tracing"]
metrics  = ["dep:metrics"]
tenant   = ["dep:tower", "dep:http", "dep:tokio", "tokio/rt"]
replica  = []

factory          = []
factory-postgres = ["factory", "postgres"]

migrate          = ["dep:async-trait", "dep:anyhow"]
migrate-postgres = ["migrate", "postgres"]
migrate-sqlite   = ["migrate", "sqlite"]
migrate-mysql    = ["migrate", "mysql"]

full = [
    "macros", "active", "query",
    "postgres", "axum", "tracing", "metrics",
    "tenant", "replica", "factory-postgres", "migrate-postgres",
]
```

---

## 12. Completed Phases

| Phase | Description | Status |
|---|---|---|
| 0–12 | Consolidate 5 crates → 1 | ✅ |
| 15 | `active` + `query` feature gates | ✅ |
| 16 | `#[model(...)]` attribute alias | ✅ |
| 17a–d | DSL scaffold + `#[derive(Table)]` | ✅ |
| 18 | `first_or_default`, `first_or_else`, `pool::ping` | ✅ |
| 19 | `SqlValue::Json`, `SqlValue::Uuid` | ✅ |
| — | crates.io v0.4.0 publish + CI/CD | ✅ |

---

## 13. Roadmap

### Phase 21 — OOP Primary API: `User::table()` + `User::ID` (v0.4.1)

Generate struct-associated `table()` method and `SCREAMING_SNAKE_CASE` column constants on
every `#[derive(Table)]` struct. Module-style alias (`pub mod users { … }`) is also emitted
as a secondary convenience.

**macro changes in `rok-fluent-macros`:**

```rust
impl User {
    pub fn table() -> UserTable { UserTable }
    pub const ID:    Column<User, i64>    = Column::new("users", "id");
    pub const NAME:  Column<User, String> = Column::new("users", "name");
    pub const EMAIL: Column<User, String> = Column::new("users", "email");
}
```

---

### Phase 22 — DSL JOIN Builder (v0.4.1)

```rust
impl SelectBuilder {
    pub fn inner_join(self, table: impl Table, on: Expr) -> Self;
    pub fn left_join(self, table: impl Table, on: Expr) -> Self;
    pub fn right_join(self, table: impl Table, on: Expr) -> Self;
    pub fn cross_join(self, table: impl Table) -> Self;
}

impl<T, V> Column<T, V> {
    pub fn references<T2, V2>(self, other: Column<T2, V2>) -> Expr;  // ON clause
    pub fn eq_col<T2, V2>(self, other: Column<T2, V2>) -> Expr;      // WHERE alias
}
```

---

### Phase 23 — DSL Pagination on SelectBuilder (v0.4.2)

Add `.paginate()`, `.simple_paginate()`, `.cursor_paginate()` terminals directly on
`SelectBuilder`. Uses the shared `Page<T>`, `SimplePage<T>`, `CursorPage<T>` types from
`rok_fluent::orm::pagination`.

Implementation:
- Offset: wraps the builder in a count query + a data query, returns `Page<T>`
- Cursor: encodes the cursor as `base64(json({col: last_value}))`, adds `WHERE col > cursor`

---

### Phase 24 — DSL Aggregators (v0.4.2)

- `Column::count()`, `.sum()`, `.avg()`, `.min()`, `.max()`, `.array_agg()`, etc. → `AggExpr`
- `SelectBuilder::group_by([col])` → adds `GROUP BY`
- `SelectBuilder::having(agg_expr)` → adds `HAVING`
- `AggExpr` implements comparison ops for use in `HAVING`

```rust
let stats = db::select()
    .from(Order::table())
    .columns([Order::USER_ID, Order::TOTAL.sum().alias("total_spend")])
    .group_by([Order::USER_ID])
    .having(Order::TOTAL.sum().gte(1000_i64))
    .fetch_all::<UserStats>(&pool).await?;
```

---

### Phase 25 — Relationship Annotations + Batch Loading (v0.4.3)

- `#[table(has_one, has_many, belongs_to, many_to_many)]` on `#[derive(Table)]` fields
- `Loaded<T>` type in `src/dsl/loaded.rs`
- `SelectBuilder::with(AssocField)` — batch load via secondary IN query
- Relationship fields excluded from `Column` constants and `sqlx::FromRow`

---

### Phase 26 — Paginated Relationship Loading (v0.4.3)

```rust
// Load only the first N related records per parent
db::select()
    .from(User::table())
    .with_paginated(User::POSTS, 1, 10)       // page 1, 10 per user
    .with_cursor(User::COMMENTS, cursor, 20)  // cursor-based
    .fetch_all(&pool).await?;
```

Each `User` in the result gets `posts: Loaded::Loaded([…first 10…])`.
The paginated load issues `SELECT * FROM posts WHERE user_id IN (…) ORDER BY id LIMIT 10` per page.

---

### Phase 27 — Through Relationships (v0.4.3)

`has_one_through`, `has_many_through`, `belongs_to_through` — navigate two foreign keys via an
intermediate model.

```rust
#[table(has_many_through = Tag, through = Post,
        local_fk = "user_id", foreign_fk = "post_id")]
pub tags: Loaded<Vec<Tag>>,
```

Generated SQL:

```sql
SELECT tags.* FROM tags
INNER JOIN post_tags ON post_tags.tag_id = tags.id
WHERE post_tags.post_id IN ($1, $2, …)
```

---

### Phase 28 — Polymorphic Relationships (v0.4.4)

`morph_one`, `morph_many`, `morph_to`, `morph_to_many`.

```rust
#[table(morph_many = Comment, as_type = "commentable")]
pub comments: Loaded<Vec<Comment>>,
```

Generated WHERE:

```sql
WHERE commentable_type = 'Post' AND commentable_id IN ($1, $2, …)
```

---

### Phase 29 — Advanced `Expr`: CASE, JSON, Arrays, String, Math, Date (v0.4.4)

- `Expr::case().when(cond, val).otherwise(val)` — typed CASE WHEN
- JSON operators on `Column<T, serde_json::Value>` (`->`, `->>`, `@>`, `?`, etc.)
- `Column::cast::<V2>()` — SQL type cast
- String functions: `.lower()`, `.upper()`, `.trim()`, `.length()`, `.concat()`
- Math functions: `.abs()`, `.round(n)`, `.ceil()`, `.floor()`
- Date functions: `.date_trunc(unit)`, `.extract(part)`, `.to_char(fmt)`
- Window functions: `.rank()`, `.row_number()`, `.lag(n)`, `.lead(n)`, `.over(Window)`

---

### Phase 30 — Subqueries, CTEs, Set Operations (v0.4.5)

```rust
// Subquery in FROM
db::select()
    .from_subquery(
        db::select().from(User::table()).where_(User::ACTIVE.eq(true)),
        "active_users"
    )

// Correlated subquery in WHERE
.where_(Expr::exists(
    db::select().from(Post::table()).where_(Post::USER_ID.references(User::ID))
))

// IN subquery
.where_(User::ID.in_subquery(
    db::select().from(Order::table()).columns([Order::USER_ID])
))

// CTEs
db::select()
    .with_cte("top_users",
        db::select().from(User::table()).order_by(User::SCORE.desc()).limit(100)
    )
    .from_cte("top_users")

// Set operations
db::select().from(User::table()).where_(User::ROLE.eq("admin"))
    .union(db::select().from(User::table()).where_(User::ROLE.eq("superadmin")))
```

---

### Phase 31 — Upsert + RETURNING on all builders (v0.4.5)

```rust
// InsertBuilder
db::insert_into(User::table())
    .values_typed([(User::EMAIL, "a@b.com"), (User::NAME, "Alice")])
    .on_conflict(User::EMAIL).do_update([(User::NAME, "Alice")])
    .returning_cols([User::ID, User::CREATED_AT])
    .fetch_one::<User>(&pool).await?;

// UpdateBuilder RETURNING
db::update(User::table())
    .set_typed(User::NAME, "Bob")
    .where_(User::ID.eq(1_i64))
    .returning()
    .fetch_one::<User>(&pool).await?;

// DeleteBuilder RETURNING
db::delete_from(User::table())
    .where_(User::ID.eq(1_i64))
    .returning_cols([User::ID, User::EMAIL])
    .fetch_all::<DeletedUser>(&pool).await?;
```

---

### Phase 32 — Built-in CRUD Service (v0.5.0)

Implement `CrudService<M>` in `src/services/crud.rs` (see Section 8 above).
Requires `active` + one database feature.

Additional helpers:
- `FilterBuilder<M>` — composable filter sets
- `SortBuilder<M>` — validated, whitelisted user-driven sorting
- `SearchService<M>` — full-text + LIKE search across `#[table(searchable)]` columns
- `AuditService<M>` — `touch()`, `history()` for timestamped models
- `SoftDeleteService<M>` — scoped queries for soft-deleted rows
- `BatchService<M>` — `bulk_insert_chunked`, `bulk_update`, `bulk_upsert_by`

---

### Phase 33 — Active Record ↔ DSL Bridge (v0.5.0)

```rust
// Typed DSL Expr inside Active Record:
Post::query()
    .and_expr(Post::USER_ID.eq(42_i64).and(Post::PUBLISHED.eq(true)))
    .all().await?;

// Convert ModelQuery → SelectBuilder:
Post::query()
    .where_eq("user_id", 42_i64)
    .into_dsl()
    .order_by(Post::CREATED_AT.desc())
    .fetch_all(&pool).await?;
```

---

### Phase 34 — Developer Experience (v0.5.1)

- **`QueryInspector`** — `.inspect()` on any builder prints SQL + params to stderr
- **`.explain(&pool) -> String`** — runs `EXPLAIN ANALYZE` and returns the plan
- **`.explain_json(&pool) -> serde_json::Value`** — structured EXPLAIN output
- **`#[table(rename_all = "camelCase")]`** — auto-rename all generated constants
- **`SelectBuilder::raw_where(sql, params)`** — raw SQL escape hatch in builder chain
- **`Expr::raw(sql)`** — raw SQL inside the typed expression tree
- **Better proc-macro errors** — `compile_error!` pointing to the offending attribute
- **`#[table(searchable)]`** — marks columns for `SearchService` + generates `search_sql()`

---

### Phase 35 — Performance & Scalability (v0.5.2)

- **Prepared statement cache** — cache compiled queries by SQL fingerprint
- **`bulk_insert_chunked`** — split large INSERTs; use `COPY FROM STDIN` on PostgreSQL
- **`SqlValue::Array(Vec<SqlValue>)`** — bind typed arrays for `= ANY($1)` (PostgreSQL)
- **Connection pool warming** — `pool::warm(n)` pre-opens connections at startup
- **Query result streaming** — `SelectBuilder::stream(&pool) -> impl Stream<Item = Result<T>>`
  for large result sets without buffering the whole Vec in memory

---

### Phase 36 — `rok db` CLI (v0.6.0)

```
rok db migrate
rok db rollback
rok db status
rok db make <name>
rok db seed
rok db schema dump
rok db schema diff
```

Optional `[[bin]]` section behind a `cli` feature (brings in `clap`).

---

### Phase 37 — Ecosystem & Publish Quality (ongoing)

- **`rok-orm` tombstone** — `rok-orm@0.3.99` pointing to `rok-fluent`
- **`docs.rs` feature matrix** — `all-features = true` in `[package.metadata.docs.rs]`
- **MSRV policy** — declare and test against stable - 2
- **Fuzz testing** — `cargo-fuzz` targets for SQL rendering correctness
- **Benchmarks** — `criterion` suite: query build time, bind time, vs raw sqlx

---

## 14. Design Decisions Log

| Decision | Rationale |
|---|---|
| OOP style primary (`User::table()`) | Struct prefix always identifies the owning model; cleaner in IDE autocomplete |
| Module alias secondary (`users::table`) | Still useful for SQL-mirroring ergonomics; trivially generated alongside OOP style |
| Single crate, proc-macro satellite | Mirrors tokio/serde; users never import macros crate directly |
| `#[non_exhaustive]` on `SqlValue` | Lets us add variants (Json, Uuid, Array…) without breaking downstream matches |
| `Expr` tree instead of string SQL | Enables query reuse, composition, and `$N` / `?` placeholder switching |
| `Loaded<T>` instead of `Option<T>` | `Option<T>` is ambiguous (null vs not loaded); `Loaded<T>` is always explicit |
| Batch IN-load over JOIN for has-many | Avoids data multiplication from JOIN fan-out; preserves struct ownership |
| `active` and `query` as separate flags | Each style is independently useful; avoids bloat for DSL-only users |
| `CrudService<M>` in `services/` | Generic zero-boilerplate service layer; users extend it via a trait |
| `SortBuilder` whitelist approach | Prevents SQL injection from user-controlled sort params |
| Paginated `.with_paginated()` | Allows safe rendering of large has-many collections without OOM risk |
| `SqlValue::Json(serde_json::Value)` | Type-safe JSON; enables `JSONB` binding on PostgreSQL |
| Auto-publish on version bump | CI checks crates.io; publishes only when version is genuinely new |
