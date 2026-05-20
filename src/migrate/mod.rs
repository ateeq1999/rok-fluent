//! Migration subsystem — schema versioning and DDL builders.
//!
//! Enable with `features = ["migrate"]` for the core traits and SQL builders,
//! or `features = ["migrate-postgres"]` to also get [`MigrationRunner`] and
//! the async [`Migration`] trait.

mod migration;
mod schema;
mod source;
mod table;

#[cfg(feature = "postgres")]
mod runner;

pub use migration::RawMigration;
pub use schema::Schema;
pub use source::{EmbeddedMigration, EmbeddedMigrations, FileSource, MigrationSource};
pub use table::TableBuilder;

#[cfg(feature = "postgres")]
pub use migration::Migration;
#[cfg(feature = "postgres")]
pub use runner::MigrationRunner;
#[cfg(feature = "postgres")]
pub use schema::SchemaExecutor;
