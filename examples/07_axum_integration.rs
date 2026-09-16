//! [`OrmLayer`] — Tower middleware that scopes a `PgPool` to every Axum
//! request, so handlers can call pool-free `ModelQuery` terminals
//! (`.get()`, `.first()`, `.count()`, …) without threading the pool through
//! extractors.
//!
//! Requires a live PostgreSQL database. Starts an HTTP server on
//! `127.0.0.1:3000` — stop it with Ctrl+C.
//!
//! ```sh
//! DATABASE_URL=postgres://user:pass@localhost/rok_fluent_db \
//!   cargo run --example 07_axum_integration --features postgres,axum,active
//! ```
//!
//! Then, in another terminal:
//!
//! ```sh
//! curl http://127.0.0.1:3000/users
//! ```

use axum::{routing::get, Json, Router};
use rok_fluent::core::model::Model;
use rok_fluent::orm::orm_layer::OrmLayer;
use rok_fluent::orm::postgres::model::PgModel;
use rok_fluent::ModelDerive;

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, ModelDerive)]
#[model(table = "users")]
pub struct User {
    pub id: i64,
    pub name: String,
    pub email: String,
}

/// `GET /users` — no pool argument: [`OrmLayer`] scopes the pool for the
/// duration of this request via a task-local, and `all_query().get()` picks
/// it up automatically.
async fn list_users() -> Json<Vec<User>> {
    let users = User::all_query().get().await.unwrap_or_default();
    Json(users)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/rok_fluent_db".into());
    let pool = sqlx::PgPool::connect(&database_url).await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS users (
            id    BIGSERIAL PRIMARY KEY,
            name  TEXT NOT NULL,
            email TEXT NOT NULL UNIQUE
        )",
    )
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com') ON CONFLICT (email) DO NOTHING")
        .execute(&pool)
        .await?;

    let app = Router::new()
        .route("/users", get(list_users))
        .layer(OrmLayer::new(pool));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    println!("Listening on http://127.0.0.1:3000 — try `curl http://127.0.0.1:3000/users`");
    axum::serve(listener, app).await?;

    Ok(())
}
