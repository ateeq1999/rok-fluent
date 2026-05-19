//! rok-orm-core — traits and query builder for the rok ORM.

pub mod condition;
pub mod model;
pub mod query;
pub mod replica;
pub mod schema_cache;
pub mod tenant;

/// PostgreSQL binding helpers.  Enable with `features = ["sqlx-postgres"]`.
#[cfg(feature = "sqlx-postgres")]
pub mod sqlx_pg;

/// SQLite binding helpers.  Enable with `features = ["sqlx-sqlite"]`.
#[cfg(feature = "sqlx-sqlite")]
pub mod sqlx_sqlite;

/// MySQL binding helpers.  Enable with `features = ["sqlx-mysql"]`.
#[cfg(feature = "sqlx-mysql")]
pub mod sqlx_mysql;

pub use condition::{Condition, JoinOp, OrderDir, SqlValue};
pub use model::Model;
pub use query::{Dialect, Join, LockClause, LockWait, QueryBuilder, SetOp, WindowDef};
// CteDef<T> is used internally; not re-exported since it's generic.
pub use replica::{DatabaseConfig, ReadStrategy, RoundRobinCounter};
pub use schema_cache::{clear, get_schema, invalidate, set_schema, ColumnMeta, TableSchema};
pub use tenant::current_tenant_id;

#[cfg(feature = "tenant")]
pub use tenant::TenantLayer;
