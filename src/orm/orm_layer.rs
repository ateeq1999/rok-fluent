//! [`OrmLayer`] — Tower middleware that scopes a [`PgPool`] to every request.
//!
//! Add this layer to your Axum router so that pool-free [`ModelQuery`] terminals
//! (`.get()`, `.first()`, `.count()`) can find the pool without it being passed
//! explicitly.
//!
//! # Example
//!
//! ```rust,no_run
//! # use rok_fluent::orm::orm_layer::OrmLayer;
//! # use axum::Router;
//! # async fn example(pool: sqlx::PgPool) {
//! let app = Router::new()
//!     .layer(OrmLayer::new(pool.clone()));
//! # }
//! ```
//!
//! [`PgPool`]: sqlx::PgPool
//! [`ModelQuery`]: crate::orm::model_query::ModelQuery

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use sqlx::PgPool;
use tower::{Layer, Service};

use crate::orm::postgres::pool;

// ── OrmLayer ──────────────────────────────────────────────────────────────────

/// Tower [`Layer`] that injects a [`PgPool`] into the task-local scope for
/// every request, enabling pool-free ORM queries.
///
/// [`PgPool`]: sqlx::PgPool
#[derive(Clone)]
pub struct OrmLayer {
    pool: PgPool,
}

impl OrmLayer {
    /// Create an `OrmLayer` wrapping the given pool.
    pub fn new(pool: PgPool) -> Self {
        crate::orm::n1::auto_configure();
        Self { pool }
    }
}

impl<S> Layer<S> for OrmLayer {
    type Service = OrmService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        OrmService {
            inner,
            pool: self.pool.clone(),
        }
    }
}

// ── OrmService ────────────────────────────────────────────────────────────────

/// Tower [`Service`] produced by [`OrmLayer`].
#[derive(Clone)]
pub struct OrmService<S> {
    inner: S,
    pool: PgPool,
}

impl<S, ReqBody> Service<axum::http::Request<ReqBody>> for OrmService<S>
where
    S: Service<axum::http::Request<ReqBody>> + Send + 'static,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<S::Response, S::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: axum::http::Request<ReqBody>) -> Self::Future {
        crate::orm::n1::reset();
        let pool = self.pool.clone();
        let future = self.inner.call(req);
        Box::pin(pool::scope_pool(pool, future))
    }
}
