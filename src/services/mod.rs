//! Built-in service layer — re-usable patterns on top of Active Record models.
//!
//! Enabled with `features = ["active", "postgres"]`.
//!
//! | Type | Description |
//! |---|---|
//! | [`CrudService<M>`] | Pool-owning CRUD wrapper — find, list, create, update, delete, paginate, search |
//! | [`FilterBuilder<M>`] | Composable, reusable WHERE clause sets |
//! | [`SortBuilder<M>`] | Whitelist-validated user-driven sorting |
//! | [`BatchService<M>`] | Efficient multi-row operations — bulk insert, update, upsert, delete |
//! | [`SoftDeleteService<M>`] | Scoped queries for soft-deletable models |
//! | [`SearchService<M>`] | ILIKE and full-text search across declared columns |
//! | [`TransactionService`] | Closure-based transactions with savepoints and CRUD |
//! | [`TxCtx`] | Transaction context obtained from `TransactionService::run` |
//! | [`AuditService<M>`] | `touch()` and `history()` for timestamped models |
//!
//! # Quick start
//!
//! ```rust,ignore
//! let svc = CrudService::<User>::new(pool.clone());
//! let page = svc.paginate(1, 25).await?;
//! let hits = svc.search("alice", &["name", "email"]).await?;
//! ```

#[cfg(all(feature = "active", feature = "postgres"))]
pub mod audit;
#[cfg(all(feature = "active", feature = "postgres"))]
pub mod batch;
#[cfg(all(feature = "active", feature = "postgres"))]
pub mod crud;
#[cfg(all(feature = "active", feature = "postgres"))]
pub mod filter;
#[cfg(all(feature = "active", feature = "postgres"))]
pub mod search;
#[cfg(all(feature = "active", feature = "postgres"))]
pub mod soft_delete;
#[cfg(all(feature = "active", feature = "postgres"))]
pub mod sort;
#[cfg(all(feature = "active", feature = "postgres"))]
pub mod transaction;

#[cfg(all(feature = "active", feature = "postgres"))]
pub use audit::{AuditEntry, AuditService};
#[cfg(all(feature = "active", feature = "postgres"))]
pub use batch::BatchService;
#[cfg(all(feature = "active", feature = "postgres"))]
pub use crud::CrudService;
#[cfg(all(feature = "active", feature = "postgres"))]
pub use filter::FilterBuilder;
#[cfg(all(feature = "active", feature = "postgres"))]
pub use search::SearchService;
#[cfg(all(feature = "active", feature = "postgres"))]
pub use soft_delete::SoftDeleteService;
#[cfg(all(feature = "active", feature = "postgres"))]
pub use sort::SortBuilder;
#[cfg(all(feature = "active", feature = "postgres"))]
pub use transaction::{TransactionService, TxCtx};
