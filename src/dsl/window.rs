//! Window function support — [`Window`] definitions and [`WinExpr`] expressions
//! for `RANK()`, `ROW_NUMBER()`, `LAG()`, `LEAD()`, etc.
//!
//! # Example
//!
//! ```rust,ignore
//! use rok_fluent::dsl::{Window, rank};
//!
//! db::select()
//!     .from(Employee::table())
//!     .columns([Employee::NAME, Employee::SALARY])
//!     .win_col(
//!         rank().over(
//!             Window::new()
//!                 .partition_by(Employee::DEPT)
//!                 .order_by(Employee::SALARY.desc())
//!         ).alias("dept_rank")
//!     )
//!     .fetch_all(&pool)
//!     .await?;
//! ```

use super::column::{Column, OrderExpr};

/// A window definition: `PARTITION BY … ORDER BY …`.
///
/// Created via [`Window::new`] and chained with `.partition_by()` / `.order_by()`.
///
/// ```rust,ignore
/// let w = Window::new()
///     .partition_by(Employee::DEPT)
///     .order_by(Employee::SALARY.desc());
/// ```
#[derive(Debug, Clone)]
pub struct Window {
    pub(crate) partitions: Vec<String>,
    pub(crate) orders: Vec<String>,
}

impl Window {
    pub fn new() -> Self {
        Self {
            partitions: Vec::new(),
            orders: Vec::new(),
        }
    }

    /// Add a `PARTITION BY` column.
    #[must_use]
    pub fn partition_by<T, V>(mut self, col: Column<T, V>) -> Self {
        self.partitions.push(col.qualified());
        self
    }

    /// Add an `ORDER BY` expression.
    #[must_use]
    pub fn order_by(mut self, order: OrderExpr) -> Self {
        self.orders.push(order.to_sql());
        self
    }

    pub(crate) fn to_sql(&self) -> String {
        let mut parts = Vec::new();
        if !self.partitions.is_empty() {
            parts.push(format!("PARTITION BY {}", self.partitions.join(", ")));
        }
        if !self.orders.is_empty() {
            parts.push(format!("ORDER BY {}", self.orders.join(", ")));
        }
        parts.join(" ")
    }
}

impl Default for Window {
    fn default() -> Self {
        Self::new()
    }
}

/// A window function expression — the function call plus optional `OVER` clause.
///
/// Constructed via free functions [`rank()`], [`row_number()`], [`dense_rank()`],
/// [`ntile()`] or via column methods [`Column::lag()`], [`Column::lead()`],
/// [`Column::first_value()`], [`Column::last_value()`].
///
/// ```rust,ignore
/// rank().over(Window::new().order_by(User::ID.asc())).alias("r")
/// ```
#[derive(Debug, Clone)]
pub struct WinExpr {
    pub(crate) sql: String,
    pub(crate) over: Option<Window>,
    pub(crate) alias: Option<String>,
}

impl WinExpr {
    pub(crate) fn new(sql: String) -> Self {
        Self {
            sql,
            over: None,
            alias: None,
        }
    }

    /// Attach an `OVER (window)` clause.
    #[must_use]
    pub fn over(mut self, window: Window) -> Self {
        self.over = Some(window);
        self
    }

    /// Assign an alias for the SELECT projection: `expr AS "alias"`.
    #[must_use]
    pub fn alias(mut self, name: impl Into<String>) -> Self {
        self.alias = Some(name.into());
        self
    }

    /// Render for use in a SELECT projection, e.g.
    /// `RANK() OVER (ORDER BY "score" DESC) AS "r"`.
    pub fn to_projection_sql(&self) -> String {
        let mut out = self.sql.clone();
        if let Some(ref w) = self.over {
            let over_sql = w.to_sql();
            if !over_sql.is_empty() {
                out.push_str(&format!(" OVER ({over_sql})"));
            }
        }
        if let Some(ref a) = self.alias {
            out.push_str(&format!(" AS \"{a}\""));
        }
        out
    }
}

// ── Free-standing window functions ──────────────────────────────────────────

/// `RANK() OVER (…)` — rank of the current row with gaps.
pub fn rank() -> WinExpr {
    WinExpr::new("RANK()".into())
}

/// `ROW_NUMBER() OVER (…)` — sequential row number within the partition.
pub fn row_number() -> WinExpr {
    WinExpr::new("ROW_NUMBER()".into())
}

/// `DENSE_RANK() OVER (…)` — rank without gaps.
pub fn dense_rank() -> WinExpr {
    WinExpr::new("DENSE_RANK()".into())
}

/// `NTILE(n) OVER (…)` — bucket number (1 to n).
pub fn ntile(n: u64) -> WinExpr {
    WinExpr::new(format!("NTILE({n})"))
}

// ── Column-based window functions ───────────────────────────────────────────

impl<T, V> Column<T, V> {
    /// `LAG(column, offset) OVER (…)` — value from a previous row.
    pub fn lag(self, offset: u64) -> WinExpr {
        WinExpr::new(format!("LAG({}, {offset})", self.qualified()))
    }

    /// `LAG(column, offset, default) OVER (…)` — with a default when no preceding row.
    pub fn lag_with_default(self, offset: u64, default: impl Into<String>) -> WinExpr {
        WinExpr::new(format!(
            "LAG({}, {offset}, {})",
            self.qualified(),
            default.into()
        ))
    }

    /// `LEAD(column, offset) OVER (…)` — value from a following row.
    pub fn lead(self, offset: u64) -> WinExpr {
        WinExpr::new(format!("LEAD({}, {offset})", self.qualified()))
    }

    /// `LEAD(column, offset, default) OVER (…)` — with a default when no following row.
    pub fn lead_with_default(self, offset: u64, default: impl Into<String>) -> WinExpr {
        WinExpr::new(format!(
            "LEAD({}, {offset}, {})",
            self.qualified(),
            default.into()
        ))
    }

    /// `FIRST_VALUE(column) OVER (…)` — value from the first row in the window frame.
    pub fn first_value(self) -> WinExpr {
        WinExpr::new(format!("FIRST_VALUE({})", self.qualified()))
    }

    /// `LAST_VALUE(column) OVER (…)` — value from the last row in the window frame.
    pub fn last_value(self) -> WinExpr {
        WinExpr::new(format!("LAST_VALUE({})", self.qualified()))
    }
}

#[cfg(test)]
mod tests {
    use super::super::column::Column;

    struct TestTable;
    const ID: Column<TestTable, i64> = Column::new("test", "id");
    const NAME: Column<TestTable, i64> = Column::new("test", "name");
    const SCORE: Column<TestTable, i64> = Column::new("test", "score");
    const DEPT: Column<TestTable, i64> = Column::new("test", "dept");

    #[test]
    fn rank_renders() {
        let w = super::Window::new().order_by(SCORE.desc());
        let expr = super::rank().over(w).alias("r");
        assert_eq!(
            expr.to_projection_sql(),
            "RANK() OVER (ORDER BY \"test\".\"score\" DESC) AS \"r\""
        );
    }

    #[test]
    fn row_number_with_partition() {
        let w = super::Window::new().partition_by(DEPT).order_by(ID.asc());
        let expr = super::row_number().over(w).alias("rn");
        assert_eq!(
            expr.to_projection_sql(),
            "ROW_NUMBER() OVER (PARTITION BY \"test\".\"dept\" ORDER BY \"test\".\"id\" ASC) AS \"rn\""
        );
    }

    #[test]
    fn dense_rank_renders() {
        let expr = super::dense_rank().alias("dr");
        assert_eq!(expr.to_projection_sql(), "DENSE_RANK() AS \"dr\"");
    }

    #[test]
    fn ntile_renders() {
        let w = super::Window::new().order_by(SCORE.desc());
        let expr = super::ntile(4).over(w);
        assert_eq!(
            expr.to_projection_sql(),
            "NTILE(4) OVER (ORDER BY \"test\".\"score\" DESC)"
        );
    }

    #[test]
    fn lag_renders() {
        let w = super::Window::new().order_by(ID.asc());
        let expr = NAME.lag(1).over(w).alias("prev_name");
        assert_eq!(
            expr.to_projection_sql(),
            "LAG(\"test\".\"name\", 1) OVER (ORDER BY \"test\".\"id\" ASC) AS \"prev_name\""
        );
    }

    #[test]
    fn lag_with_default() {
        let expr = NAME.lag_with_default(2, "'N/A'").alias("prev_name");
        assert_eq!(
            expr.to_projection_sql(),
            "LAG(\"test\".\"name\", 2, 'N/A') AS \"prev_name\""
        );
    }

    #[test]
    fn lead_renders() {
        let w = super::Window::new().order_by(ID.desc());
        let expr = NAME.lead(1).over(w);
        assert_eq!(
            expr.to_projection_sql(),
            "LEAD(\"test\".\"name\", 1) OVER (ORDER BY \"test\".\"id\" DESC)"
        );
    }

    #[test]
    fn lead_with_default() {
        let expr = NAME.lead_with_default(3, "0").alias("next");
        assert_eq!(
            expr.to_projection_sql(),
            "LEAD(\"test\".\"name\", 3, 0) AS \"next\""
        );
    }

    #[test]
    fn first_value_renders() {
        let expr = SCORE
            .first_value()
            .over(super::Window::new().order_by(ID.asc()));
        assert_eq!(
            expr.to_projection_sql(),
            "FIRST_VALUE(\"test\".\"score\") OVER (ORDER BY \"test\".\"id\" ASC)"
        );
    }

    #[test]
    fn last_value_renders() {
        let expr = SCORE
            .last_value()
            .over(super::Window::new().order_by(ID.asc()));
        assert_eq!(
            expr.to_projection_sql(),
            "LAST_VALUE(\"test\".\"score\") OVER (ORDER BY \"test\".\"id\" ASC)"
        );
    }

    #[test]
    fn window_partition_and_order() {
        let w = super::Window::new()
            .partition_by(DEPT)
            .order_by(SCORE.desc())
            .order_by(ID.asc());
        assert_eq!(
            w.to_sql(),
            "PARTITION BY \"test\".\"dept\" ORDER BY \"test\".\"score\" DESC, \"test\".\"id\" ASC"
        );
    }

    #[test]
    fn win_col_on_select_builder() {
        use super::super::select::SelectBuilder;
        let w = super::Window::new().order_by(SCORE.desc());
        let expr = super::rank().over(w).alias("r");
        let (sql, _) = SelectBuilder::new().win_col(expr).to_sql_pg();
        assert!(sql.contains("RANK() OVER (ORDER BY \"test\".\"score\" DESC) AS \"r\""));
    }
}
