//! Validation — integrating the `validator` crate's `#[derive(Validate)]` field
//! attributes with [`Hooks::before_save`], driven through the hook-aware instance
//! methods `PgModel::insert`/`save`.
//!
//! Requires a live PostgreSQL database:
//!
//! ```sh
//! DATABASE_URL=postgres://user:pass@localhost/rok_fluent_db \
//!   cargo run --example 12_validation --features postgres,active,validate
//! ```
//!
//! # Why PostgreSQL-only
//!
//! Same reason as `examples/11_hooks.rs`: `PgModel::insert`/`save` — the hook-aware
//! writes that call `before_save` — only exist for PostgreSQL today.
//!
//! # The integration pattern
//!
//! rok-fluent does not parse `#[validate(...)]` attributes itself. It only supplies
//! `impl From<validator::ValidationErrors> for OrmError` (behind the `validate`
//! feature) so a model that also `#[derive(validator::Validate)]` can plug straight
//! into `Hooks::before_save` with `?`:
//!
//! ```rust,ignore
//! impl Hooks for User {
//!     fn before_save(&mut self) -> OrmResult<()> {
//!         self.validate().map_err(OrmError::from)
//!     }
//! }
//! ```

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/rok_fluent_db".into());
    let pool = sqlx::PgPool::connect(&database_url).await?;

    sqlx::query("DROP TABLE IF EXISTS users")
        .execute(&pool)
        .await?;
    sqlx::query(
        "CREATE TABLE users (
            id    BIGSERIAL PRIMARY KEY,
            email TEXT NOT NULL,
            name  TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await?;

    // ── A valid model passes `before_save` and inserts normally ────────────────────
    let mut user = User {
        id: 0,
        email: "ada@example.com".into(),
        name: "Ada Lovelace".into(),
    };
    user.insert(&pool).await?;

    let inserted: User = User::find_by_pk(&pool, 1_i64)
        .await?
        .ok_or("row not found after insert")?;
    println!("Stored row: {inserted:?}");

    // ── An invalid email is rejected by `before_save` before touching the DB ───────
    let mut bad_email = User {
        id: 0,
        email: "not-an-email".into(),
        name: "Grace Hopper".into(),
    };
    match bad_email.insert(&pool).await {
        Ok(_) => println!("\nunexpected: invalid email insert succeeded"),
        Err(e) => println!("\nRejected by validator::Validate as expected: {e}"),
    }

    // ── An empty name is also rejected — this time on an `update` via `save()` ─────
    let mut to_update = inserted;
    to_update.name = "".into();
    match to_update.save(&pool).await {
        Ok(_) => println!("\nunexpected: empty-name save succeeded"),
        Err(e) => println!("\nRejected by validator::Validate as expected: {e}"),
    }

    let unchanged: User = User::find_by_pk(&pool, to_update.id)
        .await?
        .ok_or("row not found after rejected save")?;
    println!("\nRow left unchanged by rejected save: {unchanged:?}");

    Ok(())
}
