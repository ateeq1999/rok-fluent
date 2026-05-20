pub mod core;
pub mod orm;

#[cfg(feature = "factory")]
pub mod factory;

#[cfg(feature = "migrate")]
pub mod migrate;

// ── Core re-exports (always available) ───────────────────────────────────────

pub use crate::core::condition::{Condition, JoinOp, OrderDir, SqlValue};
pub use crate::core::model::Model;
pub use crate::core::query::{Dialect, QueryBuilder};

// ── Database backend re-exports ───────────────────────────────────────────────

#[cfg(feature = "postgres")]
pub use crate::orm::postgres;

#[cfg(feature = "mysql")]
pub use crate::orm::mysql;

#[cfg(feature = "sqlite")]
pub use crate::orm::sqlite;

// ── Optional subsystem re-exports ─────────────────────────────────────────────

#[cfg(feature = "axum")]
pub use crate::orm::orm_layer;

#[cfg(feature = "tenant")]
pub use crate::core::tenant;

#[cfg(feature = "replica")]
pub use crate::core::replica;

// ── Proc-macro derive re-exports ──────────────────────────────────────────────

#[cfg(feature = "macros")]
pub use rok_fluent_macros::{query, Model as ModelDerive, Resource, Seed};
