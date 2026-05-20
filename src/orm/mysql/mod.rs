//! MySQL backend — executor and optional Active Record model trait.

pub mod executor;

/// Active Record model trait for MySQL — gated behind `active`.
#[cfg(feature = "active")]
pub mod model;
