//! [`FilterBuilder<M>`] — composable, reusable WHERE clause sets.

use crate::core::condition::SqlValue;
use crate::orm::model_query::ModelQuery;
use crate::orm::postgres::model::PgModel;

/// A reusable set of WHERE conditions that can be applied to any [`ModelQuery<M>`].
///
/// Useful for building query filters from user input in a safe, composable way.
///
/// ```rust,no_run
/// # use rok_fluent::services::FilterBuilder;
/// # async fn run() -> Result<(), sqlx::Error> {
/// // let filter = FilterBuilder::<User>::new()
/// //     .eq("active", true)
/// //     .gte("age", 18_i64)
/// //     .like("email", "%@example.com");
/// // let users = filter.apply(User::all_query()).get().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct FilterBuilder<M> {
    conditions: Vec<FilterCondition>,
    _marker: std::marker::PhantomData<M>,
}

impl<M: PgModel + Sync> Default for FilterBuilder<M> {
    fn default() -> Self {
        Self {
            conditions: Vec::new(),
            _marker: std::marker::PhantomData,
        }
    }
}

#[derive(Debug, Clone)]
enum FilterCondition {
    Eq(String, SqlValue),
    Ne(String, SqlValue),
    Gt(String, SqlValue),
    Gte(String, SqlValue),
    Lt(String, SqlValue),
    Lte(String, SqlValue),
    Like(String, String),
    ILike(String, String),
    IsNull(String),
    IsNotNull(String),
    In(String, Vec<SqlValue>),
    NotIn(String, Vec<SqlValue>),
}

impl<M: PgModel + Sync> FilterBuilder<M> {
    /// Create an empty filter set.
    pub fn new() -> Self {
        Self::default()
    }

    /// `col = val`
    pub fn eq(mut self, col: impl Into<String>, val: impl Into<SqlValue>) -> Self {
        self.conditions
            .push(FilterCondition::Eq(col.into(), val.into()));
        self
    }

    /// `col != val`
    pub fn ne(mut self, col: impl Into<String>, val: impl Into<SqlValue>) -> Self {
        self.conditions
            .push(FilterCondition::Ne(col.into(), val.into()));
        self
    }

    /// `col > val`
    pub fn gt(mut self, col: impl Into<String>, val: impl Into<SqlValue>) -> Self {
        self.conditions
            .push(FilterCondition::Gt(col.into(), val.into()));
        self
    }

    /// `col >= val`
    pub fn gte(mut self, col: impl Into<String>, val: impl Into<SqlValue>) -> Self {
        self.conditions
            .push(FilterCondition::Gte(col.into(), val.into()));
        self
    }

    /// `col < val`
    pub fn lt(mut self, col: impl Into<String>, val: impl Into<SqlValue>) -> Self {
        self.conditions
            .push(FilterCondition::Lt(col.into(), val.into()));
        self
    }

    /// `col <= val`
    pub fn lte(mut self, col: impl Into<String>, val: impl Into<SqlValue>) -> Self {
        self.conditions
            .push(FilterCondition::Lte(col.into(), val.into()));
        self
    }

    /// `col LIKE pattern`
    pub fn like(mut self, col: impl Into<String>, pattern: impl Into<String>) -> Self {
        self.conditions
            .push(FilterCondition::Like(col.into(), pattern.into()));
        self
    }

    /// `col ILIKE pattern` — case-insensitive (PostgreSQL).
    pub fn ilike(mut self, col: impl Into<String>, pattern: impl Into<String>) -> Self {
        self.conditions
            .push(FilterCondition::ILike(col.into(), pattern.into()));
        self
    }

    /// `col IS NULL`
    pub fn is_null(mut self, col: impl Into<String>) -> Self {
        self.conditions.push(FilterCondition::IsNull(col.into()));
        self
    }

    /// `col IS NOT NULL`
    pub fn is_not_null(mut self, col: impl Into<String>) -> Self {
        self.conditions.push(FilterCondition::IsNotNull(col.into()));
        self
    }

    /// `col IN (vals)`
    pub fn in_(
        mut self,
        col: impl Into<String>,
        vals: impl IntoIterator<Item = impl Into<SqlValue>>,
    ) -> Self {
        self.conditions.push(FilterCondition::In(
            col.into(),
            vals.into_iter().map(Into::into).collect(),
        ));
        self
    }

    /// `col NOT IN (vals)`
    pub fn not_in(
        mut self,
        col: impl Into<String>,
        vals: impl IntoIterator<Item = impl Into<SqlValue>>,
    ) -> Self {
        self.conditions.push(FilterCondition::NotIn(
            col.into(),
            vals.into_iter().map(Into::into).collect(),
        ));
        self
    }

    /// Apply all accumulated conditions to an existing [`ModelQuery<M>`].
    pub fn apply(&self, mut query: ModelQuery<M>) -> ModelQuery<M> {
        for cond in &self.conditions {
            query = match cond {
                FilterCondition::Eq(c, v) => query.and_where(c, v.clone()),
                FilterCondition::Ne(c, v) => query.and_where_op(c, "!=", v.clone()),
                FilterCondition::Gt(c, v) => query.and_where_op(c, ">", v.clone()),
                FilterCondition::Gte(c, v) => query.and_where_op(c, ">=", v.clone()),
                FilterCondition::Lt(c, v) => query.and_where_op(c, "<", v.clone()),
                FilterCondition::Lte(c, v) => query.and_where_op(c, "<=", v.clone()),
                FilterCondition::Like(c, p) => query.and_where_like(c, p),
                FilterCondition::ILike(c, p) => query.and_where_ilike(c, p),
                FilterCondition::IsNull(c) => query.and_where_null(c),
                FilterCondition::IsNotNull(c) => query.and_where_not_null(c),
                FilterCondition::In(c, vs) => query.and_where_in(c, vs.clone()),
                FilterCondition::NotIn(c, vs) => query.and_where_not_in(c, vs.clone()),
            };
        }
        query
    }
}
