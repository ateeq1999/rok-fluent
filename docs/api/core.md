# API: Core (`rok_fluent::core`)

Always available — no feature flag required.

## `Model` trait

```rust,no_run
pub trait Model: Sized {
    fn table_name() -> &'static str;
    fn primary_key() -> &'static str;
    fn primary_keys() -> &'static [&'static str];
    fn columns() -> &'static [&'static str];
    fn pk_value(&self) -> SqlValue;
    fn pk_values(&self) -> Vec<SqlValue> { vec![self.pk_value()] }
    fn soft_delete_column() -> Option<&'static str> { None }
    fn timestamp_columns() -> Option<(&'static str, &'static str)> { None }

    // Provided
    fn query() -> QueryBuilder<Self>;
}
```

Normally implemented via `#[derive(Model)]`. See [features.md — macros](../features.md#macros).

### Derive attributes

Both `#[rok_orm(...)]` and `#[model(...)]` are accepted — they are identical aliases.
Use whichever reads more naturally to you.

**Struct-level (`#[model(...)]` or `#[rok_orm(...)]`)**

| Attribute | Default | Effect |
|---|---|---|
| `table = "name"` | `struct_name + "s"` in snake_case | Override table name |
| `primary_key = "col"` | `"id"` | Override PK column |
| `primary_keys = "a,b"` | — | Composite PK |
| `soft_delete` | off | Adds `deleted_at` soft-delete |
| `timestamps` | off | Adds `created_at` / `updated_at` |
| `fillable = "a,b"` | all | Mass-assign allowlist |
| `guarded = "a,b"` | none | Mass-assign blocklist |
| `typed_queries` | off | Generates `COL_*` constants |
| `hidden = "a,b"` | — | Passed through to `#[derive(Resource)]` |
| `computed = "a,b"` | — | Passed through to `#[derive(Resource)]` |

**Field-level**

| Attribute | Alias | Effect |
|---|---|---|
| `#[rok_orm(primary_key)]` | `#[model(pk)]` | Mark field as PK |
| `#[rok_orm(skip)]` | `#[model(skip)]` | Exclude from `columns()` |
| `#[rok_orm(column = "col")]` | `#[model(column = "col")]` | Override column name |
| `#[rok_orm(hidden)]` | — | Exclude from `to_resource()` |
| `#[rok_orm(index)]` | — | Register index hint |
| `#[rok_orm(unique_index)]` | — | Register unique index hint |
| `#[cast(json)]` | — | Serialize/deserialize as JSON |
| `#[cast(encrypted)]` | — | Redact in `to_resource()` as `"[ENCRYPTED]"` |
| `#[cast(enum)]` | — | Use enum string cast |
| `#[cast(csv)]` | — | Use CSV cast |
| `#[cast(timestamp)]` | — | Serialize as Unix i64 in `to_resource()` |

---

## `QueryBuilder<T>`

Fluent SQL query builder. All methods return `Self` and are `#[must_use]`.

```rust,no_run
User::query()
    .select(&["id", "name"])
    .where_eq("active", true)
    .where_gt("age", 18_i64)
    .where_like("email", "%@example.com")
    .where_in("role", vec!["admin".into(), "editor".into()])
    .where_not_null("verified_at")
    .or_where_eq("superuser", true)
    .order_by("name")
    .order_by_desc("created_at")
    .limit(20)
    .offset(40)
    .distinct()
    .to_sql();          // → (String, Vec<SqlValue>)
```

### Condition methods

| Method | SQL |
|---|---|
| `.where_eq(col, val)` | `WHERE col = $n` |
| `.where_ne(col, val)` | `WHERE col != $n` |
| `.where_gt(col, val)` | `WHERE col > $n` |
| `.where_gte(col, val)` | `WHERE col >= $n` |
| `.where_lt(col, val)` | `WHERE col < $n` |
| `.where_lte(col, val)` | `WHERE col <= $n` |
| `.where_like(col, pat)` | `WHERE col LIKE $n` |
| `.where_not_like(col, pat)` | `WHERE col NOT LIKE $n` |
| `.where_null(col)` | `WHERE col IS NULL` |
| `.where_not_null(col)` | `WHERE col IS NOT NULL` |
| `.where_in(col, vals)` | `WHERE col IN ($n, …)` |
| `.where_not_in(col, vals)` | `WHERE col NOT IN ($n, …)` |
| `.where_between(col, lo, hi)` | `WHERE col BETWEEN $n AND $m` |
| `.where_json_contains(col, json)` | `WHERE col @> $n` (PostgreSQL) |
| `.where_raw(sql)` | literal SQL fragment |
| `.or_where_eq(col, val)` | `OR col = $n` |
| `.or_where_ne(col, val)` | `OR col != $n` |

### Clauses

| Method | Effect |
|---|---|
| `.select(&[cols])` | override SELECT columns |
| `.order_by(col)` | ORDER BY col ASC |
| `.order_by_desc(col)` | ORDER BY col DESC |
| `.limit(n)` | LIMIT n |
| `.offset(n)` | OFFSET n |
| `.distinct()` | SELECT DISTINCT |
| `.join(Join { … })` | add JOIN clause |
| `.lock(LockClause::ForUpdate)` | SELECT … FOR UPDATE |
| `.with_cte(name, sub_query)` | prepend WITH clause |
| `.union(other_query)` | UNION |
| `.window(WindowDef { … })` | add WINDOW clause |
| `.to_sql()` | compile to `(sql_string, params)` |

---

## `SqlValue`

Type-erased SQL parameter. Implements `From` for all common types. Used by both the
Active Record `QueryBuilder<T>` and the typed DSL `Column<T, V>`.

```rust,no_run
let v: SqlValue = "hello".into();          // Text
let v: SqlValue = 42_i64.into();           // Integer
let v: SqlValue = 3.14_f64.into();         // Float
let v: SqlValue = true.into();             // Bool
let v: SqlValue = uuid::Uuid::new_v4().into();               // Uuid
let v: SqlValue = serde_json::json!({"key": "value"}).into(); // Json
let v: SqlValue = SqlValue::Null;
```

### Variant table

| Rust type | `SqlValue` variant | PostgreSQL binding | SQLite / MySQL binding |
|---|---|---|---|
| `&str`, `String` | `Text(String)` | `text` | `TEXT` |
| `i8`, `i16`, `i32`, `i64`, `u32`, `u64` | `Integer(i64)` | `int8` | `INTEGER` |
| `f32`, `f64` | `Float(f64)` | `float8` | `REAL` |
| `bool` | `Bool(bool)` | `bool` | `INTEGER 0/1` |
| `serde_json::Value` | `Json(Value)` | `jsonb` | serialized `TEXT` |
| `uuid::Uuid` | `Uuid(Uuid)` | `uuid` | `CHAR(36)` TEXT |
| `None` / explicit | `Null` | `NULL` | `NULL` |

### `Option<T>` coercion

`Option<T>` where `T: Into<SqlValue>` maps `Some(v)` → inner variant and `None` → `Null`:

```rust,no_run
let v: SqlValue = Some(42_i64).into();  // Integer(42)
let v: SqlValue = None::<i64>.into();   // Null
```

---

## `Condition`, `JoinOp`, `OrderDir`

Supporting types for building conditions programmatically.

```rust,no_run
use rok_fluent::core::condition::{Condition, JoinOp, OrderDir};

let cond = Condition::eq("status", "active".into());
let cond = Condition::or(vec![
    Condition::eq("role", "admin".into()),
    Condition::eq("role", "editor".into()),
]);
```

---

## `Dialect`

Controls SQL parameter placeholder style.

| Variant | Placeholder | Used by |
|---|---|---|
| `Dialect::Postgres` | `$1, $2, …` | PostgreSQL |
| `Dialect::MySql` | `?, ?, …` | MySQL |
| `Dialect::Sqlite` | `?, ?, …` | SQLite |

---

## `schema_cache`

Runtime column metadata. Populated by `MigrationRunner` at startup.

```rust,no_run
use rok_fluent::core::schema_cache;

let schema = schema_cache::get_schema("users");  // → Option<&TableSchema>
schema_cache::set_schema("users", table_schema);
schema_cache::invalidate("users");
schema_cache::clear();
```

`ColumnMeta` fields: `name: String`, `data_type: String`, `nullable: bool`.
