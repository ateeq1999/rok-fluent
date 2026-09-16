# Guide: Hooks & Validation

## Feature

```toml
# Hooks — impl Hooks for YourModel { ... }
rok-fluent = { version = "0.4", features = ["active", "postgres", "macros"] }

# + validator integration
rok-fluent = { version = "0.4", features = ["active", "postgres", "macros", "validate"] }
```

`Hooks` itself has no feature gate — the trait lives in `rok_fluent::orm::hooks` and is
always compiled. The hook-aware instance methods that actually *drive* hooks
(`PgModel::insert`/`save`/`destroy`) require `active` + `postgres`, since `PgModel` is
PostgreSQL-only today. `validate` only adds `impl From<validator::ValidationErrors> for
OrmError`, so a model can plug `validator`'s own derive into a hook body.

## Why hooks exist

Real applications need to run logic *around* a write — normalizing a field before
insert, rejecting an invalid row before it reaches the database, emitting a log line or
a domain event after a successful save, cleaning up related state before a delete. Doing
this by hand means remembering to call the same normalization/validation code at every
call site that writes a row. `Hooks` moves that logic onto the model itself: implement
`impl Hooks for User { ... }` once, and every write made through the hook-aware instance
methods (`insert`/`save`/`destroy`) runs it automatically, in a fixed order, with the
option to abort the write before it touches the database.

There is no separate registration step and no blanket implementation — you write
`impl Hooks for User {}` (or override only the methods you need) directly on your model,
so it's always obvious which models have hook behavior and what that behavior is.

## The lifecycle

All eight methods default to a no-op, so only override what you need:

| Method | Called by | Return `Err` to… |
|---|---|---|
| `before_create(&mut self)` | `insert()` | abort before `INSERT` |
| `after_create(&self)` | `insert()` | — (no-return) |
| `before_update(&mut self, dirty: &[&str])` | `save()` | abort before `UPDATE` |
| `after_update(&self)` | `save()` | — (no-return) |
| `before_save(&mut self)` | `insert()` **and** `save()` | abort before either write |
| `after_save(&self)` | `insert()` **and** `save()` | — (no-return) |
| `before_delete(&self)` | `destroy()` | abort before `DELETE` |
| `after_delete(&self)` | `destroy()` | — (no-return) |

`before_save`/`after_save` are the "around both" hooks — they fire on every successful
write regardless of whether it was a create or an update, which is where
write-path-agnostic logic (email normalization, `updated_at` stamping, audit logging)
belongs. The three writer methods on `PgModel` compose them in this order:

```text
user.insert(&pool).await
  before_create → before_save → INSERT → after_create → after_save

user.save(&pool).await
  before_update → before_save → UPDATE by pk → after_update → after_save

user.destroy(&pool).await
  before_delete → DELETE by pk → after_delete
```

Note `destroy()` has no `before_save`/`after_save` step — those only wrap `insert`/`save`,
not deletes. If either `before_*` hook in a pair returns `Err(OrmError)`, the write is
aborted before touching the database and the error surfaces to the caller as
`sqlx::Error::Configuration(Box<OrmError>)`.

## Implementing `Hooks`

```rust,no_run
use rok_fluent::core::model::Model;
use rok_fluent::orm::hooks::{Hooks, OrmError, OrmResult};
use rok_fluent::orm::postgres::model::PgModel;
use rok_fluent::ModelDerive;

#[derive(Debug, Clone, sqlx::FromRow, ModelDerive)]
#[model(table = "users")]
pub struct User {
    pub id: i64,
    pub email: String,
    pub name: String,
}

impl Hooks for User {
    fn before_create(&mut self) -> OrmResult<()> {
        println!("before_create: {}", self.email);
        Ok(())
    }

    fn after_create(&self) {
        println!("after_create: id={}", self.id);
    }

    fn before_update(&mut self, dirty: &[&str]) -> OrmResult<()> {
        // This crate has no dirty-tracking, so `dirty` is always `User::columns()`.
        println!("before_update: columns considered dirty = {dirty:?}");
        Ok(())
    }

    fn after_update(&self) {
        println!("after_update: id={}", self.id);
    }

    fn before_save(&mut self) -> OrmResult<()> {
        if self.email.trim().is_empty() {
            return Err(OrmError::new("email cannot be empty"));
        }
        self.email = self.email.to_lowercase();
        Ok(())
    }

    fn after_save(&self) {
        println!("after_save: id={} email={}", self.id, self.email);
    }

    fn before_delete(&self) -> OrmResult<()> {
        println!("before_delete: id={}", self.id);
        Ok(())
    }

    fn after_delete(&self) {
        println!("after_delete: id={}", self.id);
    }
}
```

## Driving hooks: `insert`/`save`/`destroy`

Once a model implements both `Hooks` and `ModelValues` (the latter generated
automatically by `#[derive(Model)]` — `to_values(&self) -> Vec<(&'static str, SqlValue)>`),
`PgModel` provides hook-aware instance methods that wrap the existing static CRUD
methods:

```rust,no_run
# use rok_fluent::orm::postgres::model::PgModel;
# async fn example(mut user: User, pool: sqlx::PgPool) -> Result<(), sqlx::Error> {
# #[derive(Debug, Clone, sqlx::FromRow, rok_fluent::ModelDerive)]
# #[model(table = "users")]
# struct User { id: i64, email: String, name: String }
# impl rok_fluent::orm::hooks::Hooks for User {}
user.insert(&pool).await?;   // before_create → before_save → INSERT → after_create → after_save

user.email = "alice@example.com".into();
user.save(&pool).await?;     // before_update → before_save → UPDATE by pk → after_update → after_save

user.destroy(&pool).await?;  // before_delete → DELETE by pk → after_delete
# Ok(())
# }
```

`insert`/`save` require `Self: Hooks + ModelValues + Send`; `destroy` additionally
requires `Self: Sync` because `after_delete` needs `&self` after the `.await`, and a
`&Self` held across an await point is only `Send` when `Self: Sync`. The existing static
`PgModel::create`/`update_by_pk`/`delete_by_pk` methods are completely unaffected — they
never run hooks — so this is purely additive.

## Known limitation: `before_update`'s dirty-columns list

`before_update`'s `_dirty: &[&str]` argument is always `Self::columns()` — this crate
does not track which fields actually changed. If you need to distinguish "changed"
columns from "unchanged" columns inside `before_update`, you currently have to do that
comparison yourself (e.g. by holding onto the previously-loaded row and diffing field by
field) — the hook does not do it for you.

## Combining hooks with validation

rok-fluent does not parse validation attributes itself. The `validate` feature only adds
`impl From<validator::ValidationErrors> for OrmError`, so a model that separately
`#[derive(validator::Validate)]` (from the [`validator`](https://docs.rs/validator)
crate, `0.21`, `derive` feature) can call `self.validate()` and convert the result with
`?` straight inside `before_save`:

```rust,no_run
use rok_fluent::core::model::Model;
use rok_fluent::orm::hooks::{Hooks, OrmError, OrmResult};
use rok_fluent::orm::postgres::model::PgModel;
use rok_fluent::ModelDerive;
use validator::Validate;

#[derive(Debug, Clone, sqlx::FromRow, ModelDerive, Validate)]
#[model(table = "users")]
pub struct User {
    pub id: i64,
    #[validate(email(message = "email must be a valid address"))]
    pub email: String,
    #[validate(length(min = 1, max = 100, message = "name must be 1-100 characters"))]
    pub name: String,
}

impl Hooks for User {
    fn before_save(&mut self) -> OrmResult<()> {
        self.validate().map_err(OrmError::from)
    }
}
```

Because `before_save` fires on both `insert()` and `save()`, this one hook validates
every write path — a bad `email` or an empty `name` is rejected before either an
`INSERT` or an `UPDATE` reaches the database:

```rust,no_run
# use rok_fluent::orm::postgres::model::PgModel;
# use rok_fluent::core::model::Model;
# async fn example(pool: sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
# #[derive(Debug, Clone, sqlx::FromRow, rok_fluent::ModelDerive, validator::Validate)]
# #[model(table = "users")]
# struct User { id: i64, #[validate(email)] email: String, #[validate(length(min = 1))] name: String }
# impl rok_fluent::orm::hooks::Hooks for User {
#     fn before_save(&mut self) -> rok_fluent::orm::hooks::OrmResult<()> {
#         self.validate().map_err(rok_fluent::orm::hooks::OrmError::from)
#     }
# }
let mut bad_email = User { id: 0, email: "not-an-email".into(), name: "Grace Hopper".into() };
match bad_email.insert(&pool).await {
    Ok(_) => println!("unexpected: invalid email insert succeeded"),
    Err(e) => println!("rejected by validator::Validate as expected: {e}"),
}
# Ok(())
# }
```

Combine this with the dirty-columns limitation above: because there's no
dirty-tracking, `before_save`-driven validation always re-validates every field on
every write, not just the fields that changed — for most models this is the desired
behavior anyway (a stored row should never be allowed to become invalid), but it's worth
knowing if validation ever grows expensive.

## See also

- [`examples/11_hooks.rs`](../../examples/11_hooks.rs) — full runnable walkthrough of
  `insert`/`save`/`destroy` and every hook firing in order.
- [`examples/12_validation.rs`](../../examples/12_validation.rs) — the combined
  `Hooks` + `validator::Validate` example this guide's code is drawn from.
- [ORM API docs — Hooks](../api/orm.md#hooks-rok_fluentormhooks--feature-active--postgres-for-the-instance-methods)
- [Features — `validate`](../features.md#validate)
