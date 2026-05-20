//! ORM runtime — Active Record style and supporting utilities.

pub mod casts;
pub mod collection;
pub mod eager;
pub mod hooks;
pub mod n1;
pub mod pagination;
pub mod resource;
pub mod scopes;

// Active Record style: fluent model queries, morphic relations, pivot queries.
// Requires both `active` and a database backend.
#[cfg(all(feature = "active", feature = "postgres"))]
pub mod model_query;

#[cfg(all(feature = "active", feature = "postgres"))]
pub mod morph;

#[cfg(all(feature = "active", feature = "postgres"))]
pub mod through;

#[cfg(feature = "axum")]
pub mod orm_layer;

#[cfg(feature = "postgres")]
pub mod postgres;

#[cfg(feature = "mysql")]
pub mod mysql;

#[cfg(feature = "sqlite")]
pub mod sqlite;
