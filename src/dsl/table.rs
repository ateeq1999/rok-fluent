//! [`Table`] trait — implemented by `#[derive(Table)]` for every model struct.

/// A database table with a known name.
///
/// Implemented automatically by `#[derive(Table)]`.  Users never need to
/// implement this by hand.
pub trait Table: Send + Sync + 'static {
    /// The SQL table name (e.g. `"users"`).
    fn table_name() -> &'static str
    where
        Self: Sized;

    /// Instance accessor returning the same table name.
    ///
    /// Used internally by builders that receive `impl Table` and need to call
    /// through a value rather than through a type parameter.
    fn name(&self) -> &'static str;
}
