//! SQLite backend — executor and optional Active Record model trait.

pub mod executor;

/// Active Record model trait for SQLite — gated behind `active`.
#[cfg(feature = "active")]
pub mod model;
