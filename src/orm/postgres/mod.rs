//! PostgreSQL backend — executor, pool, transactions, and optional Active Record model trait.

pub mod executor;
pub mod pool;
pub mod query_log;
pub mod transaction;

/// Active Record model trait for PostgreSQL — gated behind `active`.
#[cfg(feature = "active")]
pub mod model;

/// Many-to-many pivot queries — gated behind `active`.
#[cfg(feature = "active")]
pub mod pivot_query;
