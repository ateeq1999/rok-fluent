//! Built-in service layer — re-usable patterns on top of Active Record models.
//!
//! Enabled with `features = ["active", "postgres"]`.
//!
//! # Quick start
//!
//! ```rust,no_run
//! # use rok_fluent::services::CrudService;
//! # async fn run() -> Result<(), sqlx::Error> {
//! # let pool: sqlx::PgPool = todo!();
//! # let svc: CrudService<()> = todo!();
//! // let svc = CrudService::<User>::new(pool.clone());
//! // let user = svc.find_or_fail(1_i64).await?;
//! # Ok(())
//! # }
//! ```

#[cfg(all(feature = "active", feature = "postgres"))]
pub mod batch;
#[cfg(all(feature = "active", feature = "postgres"))]
pub mod crud;
#[cfg(all(feature = "active", feature = "postgres"))]
pub mod filter;
#[cfg(all(feature = "active", feature = "postgres"))]
pub mod sort;

#[cfg(all(feature = "active", feature = "postgres"))]
pub use batch::BatchService;
#[cfg(all(feature = "active", feature = "postgres"))]
pub use crud::CrudService;
#[cfg(all(feature = "active", feature = "postgres"))]
pub use filter::FilterBuilder;
#[cfg(all(feature = "active", feature = "postgres"))]
pub use sort::SortBuilder;
