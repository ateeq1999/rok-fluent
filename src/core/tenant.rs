//! Row-level multi-tenancy: task-local tenant ID + Tower middleware.

/// Returns the current request's tenant ID from the task-local scope, if any.
pub fn current_tenant_id() -> Option<i64> {
    #[cfg(feature = "tenant")]
    return inner::CURRENT_TENANT_ID.try_with(|id| *id).ok();
    #[cfg(not(feature = "tenant"))]
    None
}

#[cfg(feature = "tenant")]
pub(crate) mod inner {
    tokio::task_local! {
        pub(crate) static CURRENT_TENANT_ID: i64;
    }
}

#[cfg(feature = "tenant")]
pub use middleware::{TenantLayer, TenantSource};

#[cfg(feature = "tenant")]
mod middleware {
    use super::inner::CURRENT_TENANT_ID;
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll},
    };
    use tower::{Layer, Service};

    /// How the `TenantLayer` extracts the tenant ID from each request.
    #[derive(Debug, Clone)]
    pub enum TenantSource {
        Header(String),
        QueryParam(String),
        Subdomain,
    }

    impl TenantSource {
        fn extract<B>(&self, req: &http::Request<B>) -> Option<i64> {
            match self {
                TenantSource::Header(name) => req
                    .headers()
                    .get(name.as_str())
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse().ok()),
                TenantSource::QueryParam(param) => req.uri().query().and_then(|q| {
                    q.split('&').find_map(|pair| {
                        let mut kv = pair.splitn(2, '=');
                        (kv.next() == Some(param.as_str()))
                            .then(|| kv.next().and_then(|v| v.parse().ok()))
                            .flatten()
                    })
                }),
                TenantSource::Subdomain => req
                    .headers()
                    .get("host")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|host| host.split('.').next())
                    .and_then(|sub| sub.parse().ok()),
            }
        }
    }

    /// Tower layer that extracts a tenant ID per-request and scopes it in
    /// task-local storage so that `Model::query()` auto-applies tenant filters.
    #[derive(Clone)]
    pub struct TenantLayer {
        source: TenantSource,
        default: Option<i64>,
    }

    impl TenantLayer {
        pub fn from_header(name: impl Into<String>) -> Self {
            Self {
                source: TenantSource::Header(name.into()),
                default: None,
            }
        }

        pub fn from_query_param(name: impl Into<String>) -> Self {
            Self {
                source: TenantSource::QueryParam(name.into()),
                default: None,
            }
        }

        pub fn from_subdomain() -> Self {
            Self {
                source: TenantSource::Subdomain,
                default: None,
            }
        }

        pub fn with_default(mut self, id: i64) -> Self {
            self.default = Some(id);
            self
        }
    }

    impl<S: Clone> Layer<S> for TenantLayer {
        type Service = TenantService<S>;

        fn layer(&self, inner: S) -> Self::Service {
            TenantService {
                inner,
                source: self.source.clone(),
                default: self.default,
            }
        }
    }

    #[derive(Clone)]
    pub struct TenantService<S> {
        inner: S,
        source: TenantSource,
        default: Option<i64>,
    }

    impl<S, ReqBody> Service<http::Request<ReqBody>> for TenantService<S>
    where
        S: Service<http::Request<ReqBody>> + Clone + Send + 'static,
        S::Future: Send + 'static,
        ReqBody: Send + 'static,
    {
        type Response = S::Response;
        type Error = S::Error;
        type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

        fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            self.inner.poll_ready(cx)
        }

        fn call(&mut self, req: http::Request<ReqBody>) -> Self::Future {
            let tenant_id = self.source.extract(&req).or(self.default);
            let fut = self.inner.call(req);
            Box::pin(async move {
                match tenant_id {
                    Some(id) => CURRENT_TENANT_ID.scope(id, fut).await,
                    None => fut.await,
                }
            })
        }
    }
}
