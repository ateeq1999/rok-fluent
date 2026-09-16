//! Multi-tenancy — [`TenantLayer`] extracts a tenant ID per request into a
//! task-local scope, so [`current_tenant_id`] (and, with a database backend,
//! `Model::query()`'s automatic tenant-column filtering) can read it back
//! anywhere downstream without threading it through every function call.
//!
//! Run with zero setup — no database required, `tenant` feature only:
//!
//! ```sh
//! cargo run --example 10_multi_tenancy --features tenant
//! ```
//!
//! # Deviation from the plan
//!
//! This demonstrates [`TenantLayer`] as a plain [`tower::Service`] (no Axum
//! router, no database) so the example needs nothing beyond the `tenant`
//! feature. Building a full Axum app here would also require `axum` +
//! `postgres` + `active` (since `Model::tenant_column()` filtering only
//! takes effect via `#[derive(Model)]` structs, which need a database
//! backend) — see `examples/07_axum_integration.rs` for a full Axum + Tower
//! layer setup that this same `TenantLayer` composes with in a real app.
//! Note also: `orm::tenant::middleware::TenantService` (the `Service` type
//! `TenantLayer` produces) is intentionally private — only `TenantLayer`,
//! `TenantSource`, and `current_tenant_id()` are part of the public API.

use rok_fluent::tenant::{current_tenant_id, TenantLayer};
use tower::{Layer, Service};

/// A minimal Tower service that just reports whatever tenant ID (if any) is
/// visible in the current task-local scope — standing in for a real request
/// handler / Axum router.
#[derive(Clone)]
struct ReportTenantService;

impl Service<http::Request<()>> for ReportTenantService {
    type Response = Option<i64>;
    type Error = std::convert::Infallible;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: http::Request<()>) -> Self::Future {
        Box::pin(async move { Ok(current_tenant_id()) })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Extract the tenant ID from an `X-Tenant-Id` header.
    let layer = TenantLayer::from_header("x-tenant-id").with_default(0);
    let mut service = layer.layer(ReportTenantService);

    // Request with a tenant header — TenantLayer scopes `42` for this call.
    let req_with_tenant = http::Request::builder()
        .header("x-tenant-id", "42")
        .body(())?;
    let tenant = service.call(req_with_tenant).await?;
    println!("Request with X-Tenant-Id: 42 -> current_tenant_id() = {tenant:?}");

    // Request without the header — falls back to `.with_default(0)`.
    let req_without_tenant = http::Request::builder().body(())?;
    let tenant = service.call(req_without_tenant).await?;
    println!("Request without X-Tenant-Id -> current_tenant_id() = {tenant:?}");

    // Outside any TenantLayer-wrapped call, there is no tenant in scope.
    println!(
        "Outside any request: current_tenant_id() = {:?}",
        current_tenant_id()
    );

    Ok(())
}
