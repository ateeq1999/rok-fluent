//! Repository / DI override — swap the implementation behind `PgModel`'s
//! default CRUD methods for a given model without touching any call site.
//!
//! Requires a live PostgreSQL database:
//!
//! ```sh
//! DATABASE_URL=postgres://user:pass@localhost/rok_fluent_db \
//!   cargo run --example 13_repository_di --features postgres,active
//! ```
//!
//! # Why PostgreSQL-only
//!
//! `orm::postgres::repository::{Repository, register}` only exists for the
//! PostgreSQL `PgModel` trait today — like the rest of the Active Record layer
//! covered in `examples/01_quickstart.rs`'s module doc.
//!
//! # The pattern
//!
//! `Repository<M>` has a default body for every method that simply calls the
//! corresponding static `PgModel` method, so an override only needs to
//! implement what it wants to change. Once registered with
//! `orm::postgres::repository::register::<M, _>(repo)`, `PgModel`'s own
//! `find_by_pk`/`create`/`update_by_pk`/`delete_by_pk`/`all` methods check the
//! registry first and delegate transparently — every existing
//! `User::find_by_pk(...)` call site fires the override without any change.

use rok_fluent::core::condition::SqlValue;
use rok_fluent::core::model::Model;
use rok_fluent::orm::postgres::model::PgModel;
use rok_fluent::orm::postgres::repository::{self, Repository};
use rok_fluent::ModelDerive;
use sqlx::PgPool;

#[derive(Debug, Clone, sqlx::FromRow, ModelDerive)]
#[model(table = "users")]
pub struct User {
    pub id: i64,
    pub email: String,
    pub name: String,
}

/// A repository override that marks every row it fetches, so the example can
/// observe that the override fired instead of the default `executor::*` path.
struct FastUserRepo;

#[async_trait::async_trait]
impl Repository<User> for FastUserRepo {
    async fn find_by_pk(&self, pool: &PgPool, id: SqlValue) -> Result<Option<User>, sqlx::Error> {
        println!("FastUserRepo::find_by_pk override fired for id={id:?}");
        let mut found = User::find_by_pk(pool, id).await?;
        if let Some(user) = found.as_mut() {
            user.name = format!("[via FastUserRepo] {}", user.name);
        }
        Ok(found)
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

    User::create(
        &pool,
        &[
            ("email", "ada@example.com".into()),
            ("name", "Ada Lovelace".into()),
        ],
    )
    .await?;

    // ── Before registration: the default `executor::*` path is used ───────────────
    let before = User::find_by_pk(&pool, 1_i64)
        .await?
        .ok_or("row not found before registering a repository")?;
    println!("Before registration: {before:?}");
    assert_eq!(before.name, "Ada Lovelace");

    // ── Register the override at startup ────────────────────────────────────────────
    repository::register::<User, _>(FastUserRepo);

    // ── After registration: the same `User::find_by_pk` call now delegates to
    // `FastUserRepo::find_by_pk` transparently ─────────────────────────────────────
    let after = User::find_by_pk(&pool, 1_i64)
        .await?
        .ok_or("row not found after registering a repository")?;
    println!("After registration:  {after:?}");
    assert_eq!(after.name, "[via FastUserRepo] Ada Lovelace");

    Ok(())
}
