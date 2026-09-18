# Guide: Axum Integration

## Feature

```toml
rok-fluent = { version = "0.4", features = ["axum", "macros"] }
```

`axum` implies `postgres`. For MySQL or SQLite with Axum, inject the pool manually
via `Extension` instead of using `OrmLayer`.

See [`examples/07_axum_integration.rs`](../../examples/07_axum_integration.rs) for a
runnable version.

## OrmLayer

`OrmLayer` is a Tower middleware that scopes a `PgPool` as a task-local for the
duration of every request. Handlers don't need to extract or pass the pool —
pool-free `ModelQuery` terminals (`.get()`, `.first()`, `.count()`, …) find it
automatically.

```rust,no_run
use axum::{Extension, Router, routing::get};
use rok_fluent::orm::orm_layer::OrmLayer;
use sqlx::PgPool;

#[tokio::main]
async fn main() {
    let pool = PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();

    let app = Router::new()
        .route("/users", get(list_users))
        .route("/users/:id", get(get_user))
        .layer(OrmLayer::new(pool));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

## Handlers

```rust,no_run
use axum::{Json, extract::Path, http::StatusCode};
use rok_fluent::orm::postgres::model::PgModel;

async fn list_users() -> Result<Json<Vec<User>>, StatusCode> {
    User::all_query()
        .and_where("active", true)
        .order_by("name")
        .get()
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn get_user(Path(id): Path<i64>) -> Result<Json<User>, StatusCode> {
    User::find_query(id)
        .find_or_fail()
        .await
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}
```

## JSON Responses with `to_resource()`

Return API-shaped JSON that respects `#[rok_orm(hidden)]` and `#[cast(encrypted)]`:

```rust,no_run
use rok_fluent::Model;

#[derive(Model, Resource, Serialize, sqlx::FromRow)]
#[rok_orm(table = "users", hidden = "password_hash")]
pub struct User {
    pub id: i64,
    pub name: String,
    pub email: String,
    #[rok_orm(hidden)]
    pub password_hash: String,
}

async fn get_user(Path(id): Path<i64>) -> Result<Json<serde_json::Value>, StatusCode> {
    let user = User::find_query(id)
        .find_or_fail()
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(user.to_resource()))   // password_hash excluded automatically
}
```

## Pagination Endpoint

```rust,no_run
use axum::extract::Query;
use rok_fluent::orm::pagination::Page;
use serde::Deserialize;

#[derive(Deserialize)]
struct Pagination {
    page: Option<u32>,
    per_page: Option<u32>,
}

async fn list_users(
    Query(params): Query<Pagination>,
) -> Result<Json<Page<User>>, StatusCode> {
    let page = User::all_query()
        .and_where("active", true)
        .paginate(params.per_page.unwrap_or(20), params.page.unwrap_or(1))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(page))
}
```

## Transactions in Handlers

```rust,no_run
use rok_fluent::orm::postgres::{pool, transaction::Tx};

async fn create_user(
    Json(payload): Json<CreateUserPayload>,
) -> Result<Json<User>, StatusCode> {
    // `OrmLayer` scopes the pool as a task-local per request; fetch it here
    // since `Tx::begin` needs an owned `&PgPool` to start the transaction.
    let db = pool::try_current_pool().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut tx = Tx::begin(&db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let user: User = tx
        .insert_returning("users", &[
            ("name",  payload.name.into()),
            ("email", payload.email.into()),
        ])
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tx.insert::<AuditLog>("audit_log", &[
        ("action",  "user.created".into()),
        ("user_id", user.id.into()),
    ])
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(user))
}
```

## Error Handling

Define a unified error type that maps ORM errors to HTTP status codes:

```rust,no_run
use axum::response::{IntoResponse, Response};
use axum::http::StatusCode;

pub enum AppError {
    NotFound,
    Database(sqlx::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::NotFound => StatusCode::NOT_FOUND.into_response(),
            AppError::Database(e) => {
                tracing::error!("database error: {e}");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => AppError::NotFound,
            other => AppError::Database(other),
        }
    }
}
```

## Tracing Integration

```toml
rok-fluent = { version = "0.4", features = ["axum", "tracing", "macros"] }
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

```rust,no_run
tracing_subscriber::fmt()
    .with_env_filter("rok_fluent=debug,myapp=info")
    .init();
```

Every SQL query emits a `DEBUG` span with `db.statement`, `db.operation`,
`db.table`, and `db.duration_ms`.
