# rok-fluent — Architecture & Roadmap

## 1. Project Overview

**rok-fluent** is a feature-rich async ORM for Rust targeting PostgreSQL, MySQL, and SQLite via
SQLx.  It ships two composable query styles behind independent feature flags:

| Style | Flag | Inspiration | Best for |
|---|---|---|---|
| Active Record | `active` | Laravel Eloquent | CRUD-heavy apps, clean model methods |
| Typed DSL | `query` | Drizzle ORM | Complex queries, joins, type safety |

Both styles share the same `SqlValue`, `Model` trait, migration system, and factory helpers.
Users can opt into one, both, or neither and still get the core + migrations.

---

## 2. DSL API Design Decision — `users::table` vs `User::table`

### Recommendation: `users::table` primary, `User::table()` convenience alias

**The rule of thumb:** the DSL syntax should *mirror SQL syntax*.

| SQL | DSL |
|---|---|
| `FROM users` | `.from(users::table)` |
| `WHERE users.id = $1` | `.where_(users::id.eq(1_i64))` |
| `LEFT JOIN posts ON posts.user_id = users.id` | `.left_join(posts::user_id.references(users::id))` |
| `ORDER BY users.created_at DESC` | `.order_by(users::created_at.desc())` |

When you have a join between two tables, `users::id` vs `posts::user_id` is
**unambiguous at a glance**.  `User::id` vs `Post::user_id` introduces PascalCase
into what is conceptually a column identifier — reads less naturally as SQL.

**`#[derive(Table)]` generates both:**

```rust
#[derive(Debug, Table, sqlx::FromRow)]
#[table(name = "users")]
pub struct User {
    pub id:    i64,
    pub name:  String,
    pub email: String,
}

// ── What gets generated ──────────────────────────────────────────────────────

// PRIMARY: module-based (Drizzle style)
pub mod users {
    pub const table: TableMarker = TableMarker;
    pub const id:    Column<User, i64>    = Column::new("users", "id");
    pub const name:  Column<User, String> = Column::new("users", "name");
    pub const email: Column<User, String> = Column::new("users", "email");
}

// CONVENIENCE: associated items on the struct (OOP style)
impl User {
    pub fn table() -> users::TableMarker { users::table }
    pub const ID:    Column<User, i64>    = users::id;
    pub const NAME:  Column<User, String> = users::name;
    pub const EMAIL: Column<User, String> = users::email;
}
```

**Usage:**

```rust
// DSL style (SQL-natural)
db::select()
    .from(users::table)
    .where_(users::id.eq(42_i64).and(users::email.like("%@example.com")))
    .order_by(users::name.asc())
    .fetch_all::<User>(&pool).await?;

// OOP style (struct-natural)
db::select()
    .from(User::table())
    .where_(User::ID.eq(42_i64))
    .fetch_all::<User>(&pool).await?;
```

Both compile to identical SQL.  The module style is recommended in complex multi-table
queries; the struct style is nice for simple single-table lookups.

---

## 3. DSL Relationship System

### 3.1 `Loaded<T>` — the relationship carrier type

```rust
/// Marks a relationship field that may or may not be loaded from the database.
pub enum Loaded<T> {
    /// Not yet fetched — accessing the value without loading is a logic error.
    NotLoaded,
    /// Successfully loaded.
    Loaded(T),
}
```

Relationship fields on structs annotated with `#[derive(Table)]` use `Loaded<T>`.
They are skipped by the generated `Column` constants and never mapped from `sqlx::FromRow`
directly — the ORM populates them after the primary query via batch loads.

### 3.2 Relationship annotations on `#[derive(Table)]`

```rust
#[derive(Debug, Table, sqlx::FromRow)]
#[table(name = "users")]
pub struct User {
    pub id:    i64,
    pub name:  String,

    // ── Relationships (skipped by #[table(rel)] — not DB columns) ───────────

    /// User has one Profile  (FK: profiles.user_id → users.id)
    #[table(has_one = Profile, fk = "user_id")]
    pub profile: Loaded<Option<Profile>>,

    /// User has many Posts  (FK: posts.user_id → users.id)
    #[table(has_many = Post, fk = "user_id")]
    pub posts: Loaded<Vec<Post>>,

    /// User has many Tags through Posts  (User→Post→PostTag→Tag)
    #[table(has_many_through = Tag, through = Post, local_fk = "user_id", foreign_fk = "post_id")]
    pub tags: Loaded<Vec<Tag>>,

    /// User belongs to Organization  (FK: users.org_id → organizations.id)
    #[table(belongs_to = Organization, fk = "org_id")]
    pub organization: Loaded<Option<Organization>>,

    /// User has many Roles through RoleUser pivot  (M:M)
    #[table(many_to_many = Role, pivot = "role_user", local_fk = "user_id", foreign_fk = "role_id")]
    pub roles: Loaded<Vec<Role>>,

    /// Polymorphic: User has many Images as imageable
    #[table(morph_many = Image, as_type = "imageable")]
    pub images: Loaded<Vec<Image>>,
}
```

### 3.3 All relationship types

| Type | Annotation | FK location | SQL pattern |
|---|---|---|---|
| **Has One** | `has_one = T, fk = "..."` | related table | `LEFT JOIN … ON t.fk = self.pk` |
| **Has Many** | `has_many = T, fk = "..."` | related table | batch `IN` or `LEFT JOIN` |
| **Belongs To** | `belongs_to = T, fk = "..."` | this table | `LEFT JOIN … ON self.fk = t.pk` |
| **Many To Many** | `many_to_many = T, pivot = "…"` | pivot table | `JOIN pivot JOIN t` |
| **Has One Through** | `has_one_through = T, through = U` | intermediate | double join |
| **Has Many Through** | `has_many_through = T, through = U` | intermediate | double join |
| **Belongs To Through** | `belongs_to_through = T, through = U` | intermediate | double join |
| **Morph One** | `morph_one = T, as_type = "…"` | related table | `WHERE type='X' AND id=self.pk` |
| **Morph Many** | `morph_many = T, as_type = "…"` | related table | batch `IN` with type filter |
| **Morph To** | `morph_to, type_col = "…", id_col = "…"` | this table | dynamic lookup |
| **Morph To Many** | `morph_to_many = T, pivot = "…"` | poly pivot | `JOIN pivot WHERE type='X'` |

### 3.4 Loading relationships in queries

```rust
// ── Option A: .with() on SelectBuilder (type-safe, batch load) ───────────────
let users: Vec<User> = db::select()
    .from(users::table)
    .with(users::posts)     // typed — compiler knows posts is a has_many
    .with(users::profile)   // typed — compiler knows profile is a has_one
    .with(users::roles)     // typed — many_to_many
    .fetch_all(&pool)
    .await?;

// ── Option B: JOIN builder (SQL join, returns tuples) ────────────────────────
let rows: Vec<(User, Option<Post>)> = db::select()
    .from(users::table)
    .left_join(posts::table, posts::user_id.references(users::id))
    .fetch_all(&pool)
    .await?;

// ── Option C: After-fetch include (fluent chain on Vec) ──────────────────────
let users: Vec<User> = db::select().from(users::table).fetch_all(&pool).await?;
let users = users
    .include(users::posts, &pool).await?
    .include(users::roles, &pool).await?;
```

### 3.5 JOIN builder API (new in Phase 22)

```rust
// SelectBuilder gains:
.inner_join(table, on_expr)    // INNER JOIN
.left_join(table, on_expr)     // LEFT JOIN
.right_join(table, on_expr)    // RIGHT JOIN (PG/MySQL)
.cross_join(table)             // CROSS JOIN

// Column gets a new operator:
users::id.references(posts::user_id)  // → JoinExpr (used as ON clause)
users::id.eq_col(posts::user_id)      // → Expr (equality between two columns)
```

---

## 4. Directory Layout (current)

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
├── rok-fluent-macros/       # proc-macro crate (never imported directly by users)
│   ├── Cargo.toml
│   └── src/lib.rs           # Model, Table, Resource, Seed derives + query! macro
├── src/
│   ├── lib.rs               # public API + feature-gated re-exports
│   ├── core/
│   │   ├── mod.rs
│   │   ├── condition.rs     # SqlValue, Condition, JoinOp, OrderDir
│   │   ├── model.rs         # Model trait
│   │   ├── query.rs         # QueryBuilder<T> (Active Record query builder)
│   │   ├── replica.rs       # read-replica routing
│   │   ├── schema_cache.rs  # column introspection cache
│   │   ├── tenant.rs        # task-local tenant ID
│   │   └── sqlx/
│   │       ├── mod.rs
│   │       ├── pg.rs        # PostgreSQL bind helpers
│   │       ├── sqlite.rs    # SQLite bind helpers
│   │       └── mysql.rs     # MySQL bind helpers
│   ├── dsl/                 # Typed DSL (feature = "query")
│   │   ├── mod.rs
│   │   ├── column.rs        # Column<T,V> + OrderExpr
│   │   ├── db.rs            # select(), insert_into(), update(), delete_from()
│   │   ├── delete.rs
│   │   ├── expr.rs          # Expr tree, $N / ? rendering
│   │   ├── insert.rs
│   │   ├── select.rs        # + JOIN builder (Phase 22)
│   │   ├── table.rs         # Table trait
│   │   └── update.rs
│   ├── orm/                 # Active Record style
│   │   ├── mod.rs
│   │   ├── casts.rs
│   │   ├── collection.rs
│   │   ├── eager.rs         # with_has_many, with_has_one, with_belongs_to
│   │   ├── hooks.rs
│   │   ├── model_query.rs   # ModelQuery<M> (feature = "active")
│   │   ├── morph.rs         # polymorphic relationships
│   │   ├── n1.rs            # N+1 detection
│   │   ├── orm_layer.rs     # Axum middleware
│   │   ├── pagination.rs
│   │   ├── resource.rs
│   │   ├── scopes.rs
│   │   ├── through.rs       # has-many-through
│   │   ├── postgres/
│   │   │   ├── executor.rs
│   │   │   ├── model.rs     # PgModel CRUD
│   │   │   ├── pivot_query.rs
│   │   │   ├── pool.rs
│   │   │   ├── query_log.rs
│   │   │   └── transaction.rs
│   │   ├── mysql/
│   │   └── sqlite/
│   ├── factory/             # feature = "factory"
│   │   ├── mod.rs
│   │   └── faker.rs
│   └── migrate/             # feature = "migrate"
│       ├── mod.rs
│       ├── migration.rs
│       ├── runner.rs
│       ├── schema.rs
│       ├── source.rs
│       └── table.rs
└── docs/
```

---

## 5. Feature Flag Taxonomy

```toml
[features]
default = ["macros"]

# ── Proc-macro derives ────────────────────────────────────────────────────────
macros = ["dep:rok-fluent-macros"]

# ── Query styles (orthogonal — enable one or both) ───────────────────────────
active = []        # Active Record / Eloquent: ModelQuery, PgModel, MorphTo*, Through
query  = []        # Typed DSL: db::select().from(users::table).where_(...)

# ── Database backends ─────────────────────────────────────────────────────────
postgres = ["dep:sqlx", "sqlx/postgres", "dep:tokio", "tokio/rt",
            "dep:futures-core", "dep:futures", "dep:dashmap"]
sqlite   = ["dep:sqlx", "sqlx/sqlite",  "dep:tokio", "tokio/rt"]
mysql    = ["dep:sqlx", "sqlx/mysql",   "dep:tokio", "tokio/rt"]

# ── Web / tower integration ───────────────────────────────────────────────────
axum    = ["dep:axum", "dep:tower", "postgres"]

# ── Observability ─────────────────────────────────────────────────────────────
tracing = ["dep:tracing"]
metrics = ["dep:metrics"]

# ── Multi-tenancy ─────────────────────────────────────────────────────────────
tenant  = ["dep:tower", "dep:http", "dep:tokio", "tokio/rt"]

# ── Read-replica routing ──────────────────────────────────────────────────────
replica = []

# ── Test utilities ────────────────────────────────────────────────────────────
factory          = []
factory-postgres = ["factory", "postgres"]

# ── Migrations ────────────────────────────────────────────────────────────────
migrate          = ["dep:async-trait", "dep:anyhow"]
migrate-postgres = ["migrate", "postgres"]
migrate-sqlite   = ["migrate", "sqlite"]
migrate-mysql    = ["migrate", "mysql"]

# ── Convenience bundle ────────────────────────────────────────────────────────
full = [
    "macros", "active", "query",
    "postgres", "axum", "tracing", "metrics",
    "tenant", "replica", "factory-postgres", "migrate-postgres",
]
```

---

## 6. Completed Phases

| Phase | Description | Status |
|---|---|---|
| 0–12 | Consolidate 5 crates → 1 | ✅ |
| 15 | `active` + `query` feature gates | ✅ |
| 16 | `#[model(...)]` attribute alias | ✅ |
| 17a–d | `query` DSL scaffold + `#[derive(Table)]` | ✅ |
| 18 | `first_or_default`, `first_or_else`, `pool::ping` | ✅ |
| 19 | `SqlValue::Json`, `SqlValue::Uuid` | ✅ |
| — | crates.io publish (v0.4.0) + CI/CD | ✅ |

---

## 7. Roadmap

### Phase 21 — Dual API: `users::table` + `User::table()` (v0.4.1)

**Goal:** `#[derive(Table)]` generates both the module-based DSL API *and* associated
constants/methods on the struct for users who prefer the OOP style.

**Changes to `rok-fluent-macros`:**

```rust
// In addition to the existing `pub mod users { ... }`, also generate:
impl User {
    /// Table marker — identical to `users::table`.
    pub fn table() -> users::TableMarker { users::table }
    /// Typed column — identical to `users::id`.
    pub const ID:    ::rok_fluent::dsl::Column<Self, i64>    = users::id;
    pub const NAME:  ::rok_fluent::dsl::Column<Self, String> = users::name;
    pub const EMAIL: ::rok_fluent::dsl::Column<Self, String> = users::email;
}
```

- Associated constant names are `SCREAMING_SNAKE_CASE` (Rust convention for consts).
- `table()` method returns the exact same singleton as `users::table`.
- Only generated when `feature = "query"` is active.

---

### Phase 22 — DSL JOIN Builder (v0.4.1)

**Goal:** typed JOIN support in `SelectBuilder`.

**New `Expr` variants:**

```rust
// In src/dsl/expr.rs:
pub enum Expr {
    // … existing …
    /// Two columns referencing each other: `"t1"."col" = "t2"."col"`
    ColEq(String, String),
}
```

**New `Column` method:**

```rust
impl<T, V> Column<T, V> {
    /// `self.col = other.col` for use in JOIN ON expressions.
    pub fn references<T2, V2>(self, other: Column<T2, V2>) -> Expr {
        Expr::ColEq(self.qualified(), other.qualified())
    }
    /// Same as `references` — alias for `WHERE` column-equality expressions.
    pub fn eq_col<T2, V2>(self, other: Column<T2, V2>) -> Expr {
        Expr::ColEq(self.qualified(), other.qualified())
    }
}
```

**New `SelectBuilder` methods:**

```rust
impl SelectBuilder {
    pub fn inner_join(mut self, table: impl Table, on: Expr) -> Self { … }
    pub fn left_join(mut self, table: impl Table, on: Expr) -> Self { … }
    pub fn right_join(mut self, table: impl Table, on: Expr) -> Self { … }
    pub fn cross_join(mut self, table: impl Table) -> Self { … }
}
```

**Rendered SQL:**

```rust
db::select()
    .from(users::table)
    .left_join(posts::table, posts::user_id.references(users::id))
    .where_(users::id.gt(0_i64))
    .to_sql_pg()
// → SELECT * FROM "users" LEFT JOIN "posts" ON "posts"."user_id" = "users"."id"
//   WHERE "users"."id" > $1
```

---

### Phase 23 — DSL Relationship Annotations on `#[derive(Table)]` (v0.4.2)

**Goal:** declare relationships in the struct; the macro generates batch-load methods and
`.with()` compatibility; zero runtime overhead when not used.

**New `Loaded<T>` type in `src/dsl/loaded.rs`:**

```rust
pub enum Loaded<T> {
    NotLoaded,
    Loaded(T),
}

impl<T> Loaded<T> {
    pub fn get(&self) -> Option<&T> { … }
    pub fn unwrap(self) -> T { … }
    pub fn is_loaded(&self) -> bool { … }
}
```

**`#[table(has_many = T, fk = "...")]` etc. on fields:**

- Relationship fields are excluded from `Column` constant generation.
- They are excluded from `sqlx::FromRow` mapping (not DB columns).
- The macro generates a typed `Relationship` descriptor for each annotated field.
- `SelectBuilder::with(column)` accepts a `Relationship` descriptor.

**Batch-load integration:**

```rust
// .with() on SelectBuilder performs a secondary SELECT … WHERE fk IN (…)
let users: Vec<User> = db::select()
    .from(users::table)
    .with(users::posts)     // generates: SELECT * FROM posts WHERE user_id IN (…)
    .with(users::profile)   // generates: SELECT * FROM profiles WHERE user_id IN (…)
    .fetch_all(&pool)
    .await?;
// users[0].posts → Loaded::Loaded(vec![…])
// users[0].profile → Loaded::Loaded(Some(profile))
```

**Supported annotations:**

```rust
// 1:1  — FK on related table
#[table(has_one = Profile, fk = "user_id")]
pub profile: Loaded<Option<Profile>>,

// 1:M  — FK on related table
#[table(has_many = Post, fk = "user_id")]
pub posts: Loaded<Vec<Post>>,

// M:1  — FK on this table
#[table(belongs_to = Organization, fk = "org_id")]
pub organization: Loaded<Option<Organization>>,

// M:M  — through pivot table
#[table(many_to_many = Role, pivot = "role_user", local_fk = "user_id", foreign_fk = "role_id")]
pub roles: Loaded<Vec<Role>>,
```

---

### Phase 24 — Through Relationships (v0.4.2)

**Goal:** `has_one_through`, `has_many_through`, `belongs_to_through` — navigate across
two foreign keys via an intermediate model.

```rust
// User has many Tags through Posts
#[table(has_many_through = Tag,
        through = Post,
        local_fk  = "user_id",      // posts.user_id = users.id
        foreign_fk = "post_id")]    // post_tags.post_id = posts.id
pub tags: Loaded<Vec<Tag>>,

// Post has one Country through User's organization
#[table(has_one_through = Country,
        through = User,
        local_fk = "user_id",
        foreign_fk = "org_id")]
pub country: Loaded<Option<Country>>,

// Post belongs to Organization through User
#[table(belongs_to_through = Organization,
        through = User,
        local_fk = "user_id",
        foreign_fk = "org_id")]
pub organization: Loaded<Option<Organization>>,
```

**Generated SQL (has_many_through):**

```sql
SELECT tags.* FROM tags
INNER JOIN post_tags ON post_tags.tag_id = tags.id
WHERE post_tags.post_id IN ($1, $2, …)
```

---

### Phase 25 — Polymorphic Relationships (v0.4.3)

**Goal:** `morph_one`, `morph_many`, `morph_to`, `morph_to_many` — one table serves as the
target of relationships from multiple other tables.

**Convention:** polymorphic columns are `<name>_type` (stores the model name) and `<name>_id`.

```rust
// On the polymorphic target (Comment, Image, etc.):
#[table(morph_to, type_col = "commentable_type", id_col = "commentable_id")]
pub commentable: Loaded<MorphTarget>,   // MorphTarget = enum { Post(Post), Video(Video) }

// On the owning models (Post, Video):
#[table(morph_many = Comment, as_type = "commentable")]
pub comments: Loaded<Vec<Comment>>,

#[table(morph_one = Image, as_type = "imageable")]
pub cover_image: Loaded<Option<Image>>,

// Polymorphic M:M (Post/Video ↔ Tag through taggables pivot):
#[table(morph_to_many = Tag, pivot = "taggables", as_type = "taggable",
        id_col = "taggable_id", type_col = "taggable_type")]
pub tags: Loaded<Vec<Tag>>,
```

**Generated WHERE for morph_many:**

```sql
SELECT * FROM comments
WHERE commentable_type = 'Post' AND commentable_id IN ($1, $2, …)
```

---

### Phase 26 — Active Record ↔ DSL Bridge (v0.4.3)

**Goal:** Active Record `ModelQuery` can use `Expr` from the DSL; DSL `SelectBuilder` can
use `Model` constraints for type-checked table names.

```rust
// Use typed DSL Expr inside Active Record:
let posts = Post::query()
    .and_expr(posts::user_id.eq(42_i64).and(posts::published.eq(true)))
    .get().await?;

// Convert ModelQuery to SelectBuilder for DSL terminals:
let posts: Vec<Post> = Post::query()
    .where_eq("user_id", 42_i64)
    .into_dsl()          // → SelectBuilder
    .order_by(posts::created_at.desc())
    .fetch_all(&pool).await?;
```

---

### Phase 27 — Developer Experience (v0.4.4)

**Goal:** make debugging and query inspection first-class.

- **`QueryInspector`** — `.inspect()` on any builder prints the SQL + bound parameters to stderr (gated behind a debug flag or always in debug builds)
- **`EXPLAIN` terminal** — `.explain(&pool) -> String` runs `EXPLAIN ANALYZE`
- **`query_log` module** — structured log of every SQL statement with timing, row count, table name
- **Better compile errors** — custom `compile_error!` in proc-macros that point to the offending attribute with a fix hint
- **`#[table(rename_all = "camelCase")]`** — auto-rename all column constants to a naming convention
- **`SelectBuilder::raw_where(sql, params)`** — escape hatch for complex SQL without breaking the builder chain
- **`Expr::raw(sql)`** — raw SQL fragment inside a typed expression tree

---

### Phase 28 — Performance & Scalability (v0.4.5)

- **Prepared statement cache** — cache compiled queries by SQL fingerprint, skip the parse phase on repeated calls
- **`bulk_insert_chunked`** — split large inserts into batches (PostgreSQL `COPY FROM STDIN` backend for very large loads)
- **`upsert`** — `INSERT … ON CONFLICT DO UPDATE` typed builder
- **`returning_many`** — `INSERT … RETURNING *` for bulk insert returning rows
- **`SqlValue::Array(Vec<SqlValue>)`** — bind a typed array for `= ANY($1)` pattern (PostgreSQL)
- **Connection pool warming** — `pool::warm(n)` pre-opens `n` connections at startup

---

### Phase 29 — `rok db` CLI (v0.5.0)

**Goal:** `rok` command-line tool for common ORM tasks.

```
rok db migrate            # run pending migrations
rok db rollback           # roll back one migration
rok db status             # show migration status table
rok db make <name>        # scaffold a timestamped migration file
rok db seed               # run Seed derives
rok db schema dump        # introspect live DB → schema struct
rok db schema diff        # compare live DB vs migration history
```

Implementation: `[[bin]]` target in a separate optional crate `rok-fluent-cli`, or a
`[[bin]]` section in the main `Cargo.toml` behind a `cli` feature that brings in `clap`.

---

### Phase 30 — Ecosystem & Publish Quality (ongoing)

- **`rok-orm` tombstone** (Phase 13) — publish `rok-orm@0.3.99` pointing to `rok-fluent`
- **`docs.rs` feature matrix** — `[package.metadata.docs.rs] all-features = true`
- **MSRV policy** — declare minimum supported Rust version (target: stable - 2)
- **Fuzz testing** — `cargo-fuzz` targets for SQL rendering correctness
- **Benchmarks** — `criterion` suite measuring query build + bind time vs raw sqlx

---

## 8. Design Decisions Log

| Decision | Rationale |
|---|---|
| Single crate, proc-macro satellite | Mirrors tokio/serde pattern; users never import macros crate directly |
| `#[non_exhaustive]` on `SqlValue` | Lets us add variants (Json, Uuid, Array…) without breaking downstream matches |
| `Expr` tree instead of string SQL | Enables query reuse, composition, and `$N` / `?` placeholder switching |
| `Loaded<T>` instead of `Option<T>` | `Option<T>` is ambiguous (null vs not loaded); `Loaded<T>` is always explicit |
| Batch IN-load over JOIN for has-many | Avoids data multiplication (duplicate parent rows per child); preserves struct ownership |
| Both `users::table` and `User::table()` | SQL-natural primary style; OOP alias for discoverability |
| `active` and `query` as separate flags | Each style is independently useful; avoids bloat for DSL-only or Active Record-only users |
| `SqlValue::Json(serde_json::Value)` | More type-safe than storing JSON as `Text`; enables `JSONB` binding on PG |
| Auto-publish on version bump | CI checks crates.io; publishes only when version is genuinely new — no accidental re-publishes |
