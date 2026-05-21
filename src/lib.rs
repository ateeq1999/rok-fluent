#![cfg_attr(docsrs, feature(doc_auto_cfg))]

//! **rok-fluent** — async ORM for Rust with PostgreSQL, MySQL, and SQLite.
//!
//! # Feature flags
//!
//! | Flag | Description |
//! |------|-------------|
//! | `macros` *(default)* | `#[derive(Model)]`, `#[derive(Resource)]`, `#[derive(Seed)]` |
//! | `active` | Active Record / Eloquent style — [`orm::postgres::model::PgModel`], [`ModelQuery`](orm::model_query::ModelQuery), `MorphTo*`, `ThroughQuery` |
//! | `query` | Drizzle-style typed query DSL — [`dsl::db`] entry points + `#[derive(Table)]` |
//! | `postgres` | PostgreSQL backend via sqlx |
//! | `sqlite` | SQLite backend via sqlx |
//! | `mysql` | MySQL backend via sqlx |
//! | `axum` | Tower middleware for Axum (implies `postgres`) |
//! | `tracing` | OpenTelemetry-compatible span instrumentation |
//! | `metrics` | Prometheus-style query counters |
//! | `tenant` | Task-local multi-tenancy via Tower layer |
//! | `replica` | Read-replica routing strategies |
//! | `migrate` | Schema migration runner and DDL builders |
//! | `factory` | Test factory helpers and `Faker` data generator |
//! | `full` | Enables all of the above |

pub mod core;
pub mod orm;
pub use orm::casts::TypedJson;

#[cfg(feature = "factory")]
pub mod factory;

#[cfg(feature = "migrate")]
pub mod migrate;

#[cfg(feature = "query")]
pub mod dsl;

pub mod services;

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

// ── Active Record style re-exports ────────────────────────────────────────────

#[cfg(all(feature = "active", feature = "postgres"))]
pub use crate::orm::model_query;

#[cfg(all(feature = "active", feature = "postgres"))]
pub use crate::orm::morph;

#[cfg(all(feature = "active", feature = "postgres"))]
pub use crate::orm::through;

// ── Optional subsystem re-exports ─────────────────────────────────────────────

#[cfg(feature = "axum")]
pub use crate::orm::orm_layer;

#[cfg(feature = "tenant")]
pub use crate::core::tenant;

#[cfg(feature = "replica")]
pub use crate::core::replica;

// ── Proc-macro derive re-exports ──────────────────────────────────────────────

#[cfg(feature = "macros")]
pub use rok_fluent_macros::{query, Model as ModelDerive, Resource, Seed, Table as TableDerive};
