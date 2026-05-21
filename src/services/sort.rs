//! [`SortBuilder<M>`] — declarative, whitelisted user-driven sorting.

use crate::orm::model_query::ModelQuery;
use crate::orm::postgres::model::PgModel;

/// A whitelist-validated sort specification for [`ModelQuery<M>`].
///
/// Prevents arbitrary column injection from user input by requiring all
/// sortable columns to be declared upfront.
///
/// ```rust,no_run
/// # use rok_fluent::services::SortBuilder;
/// # async fn run() -> Result<(), sqlx::Error> {
/// // let sort = SortBuilder::<User>::new()
/// //     .allow("name")
/// //     .allow("created_at")
/// //     .apply_user_input("created_at", false);  // false = DESC
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct SortBuilder<M> {
    allowed: Vec<String>,
    sorts: Vec<(String, bool)>,
    _marker: std::marker::PhantomData<M>,
}

impl<M: PgModel + Sync> SortBuilder<M> {
    /// Create an empty sort builder (no allowed columns).
    pub fn new() -> Self {
        Self {
            allowed: Vec::new(),
            sorts: Vec::new(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Declare a column as sortable.
    pub fn allow(mut self, col: impl Into<String>) -> Self {
        self.allowed.push(col.into());
        self
    }

    /// Apply a user-supplied sort if the column is on the allow-list.
    ///
    /// `asc = true` for ascending, `false` for descending.
    /// Silently ignored if `col` is not in the allowed list.
    pub fn apply_user_input(mut self, col: &str, asc: bool) -> Self {
        if self.allowed.iter().any(|a| a == col) {
            self.sorts.push((col.to_owned(), asc));
        }
        self
    }

    /// Add a hardcoded sort regardless of the allow-list.
    pub fn then_by(mut self, col: impl Into<String>, asc: bool) -> Self {
        self.sorts.push((col.into(), asc));
        self
    }

    /// Apply all accumulated sorts to an existing [`ModelQuery<M>`].
    pub fn apply(&self, mut query: ModelQuery<M>) -> ModelQuery<M> {
        for (col, asc) in &self.sorts {
            query = if *asc {
                query.order_by(col)
            } else {
                query.order_by_desc(col)
            };
        }
        query
    }
}

impl<M: PgModel + Sync> Default for SortBuilder<M> {
    fn default() -> Self {
        Self::new()
    }
}
