//! [`QueryBuilder`] — fluent SQL builder.
//!
//! Use [`QueryBuilder::to_sql`] for PostgreSQL (`$N` placeholders) and
//! [`QueryBuilder::to_sql_with_dialect`] when targeting SQLite (`?` placeholders).

use std::marker::PhantomData;

use crate::condition::{Condition, JoinOp, OrderDir, SqlValue};

// ── Dialect ───────────────────────────────────────────────────────────────────

/// SQL placeholder dialect.
///
/// - [`Dialect::Postgres`] — numbered placeholders (`$1`, `$2`, …)
/// - [`Dialect::Sqlite`]   — anonymous placeholders (`?`, `?`, …)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dialect {
    #[default]
    Postgres,
    Sqlite,
}

// ── Join ─────────────────────────────────────────────────────────────────────

/// A SQL JOIN clause.
#[derive(Debug, Clone)]
pub enum Join {
    /// `INNER JOIN table ON condition`
    Inner(String, String),
    /// `LEFT JOIN table ON condition`
    Left(String, String),
    /// `RIGHT JOIN table ON condition`
    Right(String, String),
    /// Raw join fragment appended verbatim.
    Raw(String),
}

// ── CTE ───────────────────────────────────────────────────────────────────────

/// A Common Table Expression (WITH clause) definition.
#[derive(Debug, Clone)]
pub struct CteDef<T> {
    /// CTE name (e.g. `"recent_posts"`)
    pub name: String,
    /// Optional column aliases
    pub columns: Vec<String>,
    /// The subquery builder
    pub subquery: QueryBuilder<T>,
    /// Whether this CTE is recursive
    pub recursive: bool,
}

// CteDef auto-derived traits need manual Clone for the generic case,
// but #[derive(Clone)] handles it since QueryBuilder<T>: Clone.

// ── Lock Clause ──────────────────────────────────────────────────────────────

/// Row-level locking clause for SELECT statements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockClause {
    /// `FOR UPDATE`
    ForUpdate,
    /// `FOR NO KEY UPDATE`
    ForNoKeyUpdate,
    /// `FOR SHARE`
    ForShare,
    /// `FOR KEY SHARE`
    ForKeyShare,
}

impl LockClause {
    pub fn as_sql(self) -> &'static str {
        match self {
            Self::ForUpdate => "FOR UPDATE",
            Self::ForNoKeyUpdate => "FOR NO KEY UPDATE",
            Self::ForShare => "FOR SHARE",
            Self::ForKeyShare => "FOR KEY SHARE",
        }
    }
}

/// Row locking wait behaviour modifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockWait {
    /// `NOWAIT`
    NoWait,
    /// `SKIP LOCKED`
    SkipLocked,
}

impl LockWait {
    pub fn as_sql(self) -> &'static str {
        match self {
            Self::NoWait => "NOWAIT",
            Self::SkipLocked => "SKIP LOCKED",
        }
    }
}

// ── Set Operation ────────────────────────────────────────────────────────────

/// Set operation for UNION / INTERSECT / EXCEPT queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetOp {
    /// `UNION`
    Union,
    /// `UNION ALL`
    UnionAll,
    /// `INTERSECT`
    Intersect,
    /// `INTERSECT ALL`
    IntersectAll,
    /// `EXCEPT`
    Except,
    /// `EXCEPT ALL`
    ExceptAll,
}

impl SetOp {
    pub fn as_sql(self) -> &'static str {
        match self {
            Self::Union => "UNION",
            Self::UnionAll => "UNION ALL",
            Self::Intersect => "INTERSECT",
            Self::IntersectAll => "INTERSECT ALL",
            Self::Except => "EXCEPT",
            Self::ExceptAll => "EXCEPT ALL",
        }
    }
}

// ── Window ───────────────────────────────────────────────────────────────────

/// A named window definition for `WINDOW` clause.
#[derive(Debug, Clone)]
pub struct WindowDef {
    /// Window name (e.g. `"w"`)
    pub name: String,
    /// PARTITION BY columns
    pub partition: Vec<String>,
    /// ORDER BY columns with direction
    pub order: Vec<(String, OrderDir)>,
}

/// A fluent builder that produces parameterized SQL statements.
///
/// Conditions added with `where_*` methods are joined with `AND`.
/// Use `or_where_*` variants to join with `OR`.
///
/// # Example
///
/// ```rust
/// use rok_orm_core::{QueryBuilder, SqlValue};
///
/// let (sql, params) = QueryBuilder::<()>::new("users")
///     .where_eq("active", true)
///     .or_where_eq("role", "admin")
///     .order_by_desc("created_at")
///     .limit(20)
///     .offset(40)
///     .to_sql();
///
/// assert!(sql.contains("WHERE"));
/// assert!(sql.contains("ORDER BY created_at DESC"));
/// assert!(sql.contains("LIMIT 20"));
/// assert!(sql.contains("OFFSET 40"));
/// assert_eq!(params.len(), 2);
/// ```
#[derive(Debug, Clone)]
pub struct QueryBuilder<T> {
    table: String,
    select_cols: Option<Vec<String>>,
    select_raw: Option<String>,
    distinct: bool,
    distinct_on_cols: Vec<String>,
    joins: Vec<Join>,
    conditions: Vec<(JoinOp, Condition)>,
    group_by: Vec<String>,
    having: Option<String>,
    order: Vec<(String, OrderDir)>,
    order_raw: Option<String>,
    limit_val: Option<usize>,
    offset_val: Option<usize>,
    /// When `true` (default), the execution layer should route this query to a
    /// read replica if one is configured.  Set to `false` via `on_write_db`.
    pub use_replica: bool,
    /// Extra SELECT expressions appended after the main column list.
    /// Used by `with_count`, `with_sum`, etc. in `ModelQuery`.
    extra_select_exprs: Vec<String>,
    /// CTE definitions (WITH clause).
    ctes: Vec<CteDef<T>>,
    /// Window definitions (WINDOW clause).
    windows: Vec<WindowDef>,
    /// Row-level locking clause (FOR UPDATE / FOR SHARE etc.)
    lock: Option<LockClause>,
    /// Lock wait behaviour (NOWAIT / SKIP LOCKED)
    lock_wait: Option<LockWait>,
    /// Set operation (UNION / INTERSECT / EXCEPT) with right-hand query.
    set_op: Option<(SetOp, Box<QueryBuilder<T>>)>,
    _marker: PhantomData<T>,
}

impl<T> QueryBuilder<T> {
    pub fn new(table: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            select_cols: None,
            select_raw: None,
            distinct: false,
            distinct_on_cols: Vec::new(),
            joins: Vec::new(),
            conditions: Vec::new(),
            group_by: Vec::new(),
            having: None,
            order: Vec::new(),
            order_raw: None,
            limit_val: None,
            offset_val: None,
            use_replica: true,
            extra_select_exprs: Vec::new(),
            ctes: Vec::new(),
            windows: Vec::new(),
            lock: None,
            lock_wait: None,
            set_op: None,
            _marker: PhantomData,
        }
    }

    // ── column selection ──────────────────────────────────────────────────

    pub fn select(mut self, cols: &[&str]) -> Self {
        self.select_cols = Some(cols.iter().map(|s| s.to_string()).collect());
        self
    }

    /// Emit `SELECT <raw_expr> FROM …` — overrides `select()`.
    pub fn select_raw(mut self, expr: &str) -> Self {
        self.select_raw = Some(expr.to_string());
        self
    }

    /// Append an extra expression to the SELECT list without replacing existing columns.
    /// Used internally by `with_count`, `with_sum`, etc.
    pub fn add_select_expr(mut self, expr: impl Into<String>) -> Self {
        self.extra_select_exprs.push(expr.into());
        self
    }

    /// Emit `SELECT DISTINCT …`.
    pub fn distinct(mut self) -> Self {
        self.distinct = true;
        self
    }

    /// Emit `SELECT DISTINCT ON (col, …) …` (PostgreSQL only).
    pub fn distinct_on(mut self, cols: &[&str]) -> Self {
        self.distinct_on_cols = cols.iter().map(|s| s.to_string()).collect();
        self
    }

    // ── joins ─────────────────────────────────────────────────────────────

    /// Add an `INNER JOIN table ON condition`.
    ///
    /// ```rust
    /// use rok_orm_core::QueryBuilder;
    ///
    /// let (sql, _) = QueryBuilder::<()>::new("orders")
    ///     .inner_join("users", "users.id = orders.user_id")
    ///     .select(&["orders.id", "users.name"])
    ///     .to_sql();
    ///
    /// assert!(sql.contains("INNER JOIN users ON users.id = orders.user_id"));
    /// ```
    pub fn inner_join(mut self, table: &str, on: &str) -> Self {
        self.joins
            .push(Join::Inner(table.to_string(), on.to_string()));
        self
    }

    /// Add a `LEFT JOIN table ON condition`.
    pub fn left_join(mut self, table: &str, on: &str) -> Self {
        self.joins
            .push(Join::Left(table.to_string(), on.to_string()));
        self
    }

    /// Add a `RIGHT JOIN table ON condition`.
    pub fn right_join(mut self, table: &str, on: &str) -> Self {
        self.joins
            .push(Join::Right(table.to_string(), on.to_string()));
        self
    }

    /// Append a raw JOIN fragment verbatim (e.g. `"INNER JOIN s ON s.user_id = users.id AND s.active"`).
    pub fn join_raw(mut self, raw: &str) -> Self {
        self.joins.push(Join::Raw(raw.to_string()));
        self
    }

    // ── GROUP BY / HAVING ─────────────────────────────────────────────────

    /// Add a `GROUP BY` clause.
    ///
    /// ```rust
    /// use rok_orm_core::QueryBuilder;
    ///
    /// let (sql, _) = QueryBuilder::<()>::new("orders")
    ///     .select(&["user_id", "COUNT(*) as total"])
    ///     .group_by(&["user_id"])
    ///     .having("COUNT(*) > 5")
    ///     .to_sql();
    ///
    /// assert!(sql.contains("GROUP BY user_id"));
    /// assert!(sql.contains("HAVING COUNT(*) > 5"));
    /// ```
    pub fn group_by(mut self, cols: &[&str]) -> Self {
        self.group_by = cols.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Add a `HAVING` clause (requires `group_by`).
    pub fn having(mut self, expr: &str) -> Self {
        self.having = Some(expr.to_string());
        self
    }

    // ── AND conditions ────────────────────────────────────────────────────

    pub fn where_eq(self, col: &str, val: impl Into<SqlValue>) -> Self {
        self.push(JoinOp::And, Condition::Eq(col.into(), val.into()))
    }

    pub fn where_ne(self, col: &str, val: impl Into<SqlValue>) -> Self {
        self.push(JoinOp::And, Condition::Ne(col.into(), val.into()))
    }

    pub fn where_gt(self, col: &str, val: impl Into<SqlValue>) -> Self {
        self.push(JoinOp::And, Condition::Gt(col.into(), val.into()))
    }

    pub fn where_gte(self, col: &str, val: impl Into<SqlValue>) -> Self {
        self.push(JoinOp::And, Condition::Gte(col.into(), val.into()))
    }

    pub fn where_lt(self, col: &str, val: impl Into<SqlValue>) -> Self {
        self.push(JoinOp::And, Condition::Lt(col.into(), val.into()))
    }

    pub fn where_lte(self, col: &str, val: impl Into<SqlValue>) -> Self {
        self.push(JoinOp::And, Condition::Lte(col.into(), val.into()))
    }

    pub fn where_like(self, col: &str, pattern: &str) -> Self {
        self.push(JoinOp::And, Condition::Like(col.into(), pattern.into()))
    }

    pub fn where_not_like(self, col: &str, pattern: &str) -> Self {
        self.push(JoinOp::And, Condition::NotLike(col.into(), pattern.into()))
    }

    pub fn where_null(self, col: &str) -> Self {
        self.push(JoinOp::And, Condition::IsNull(col.into()))
    }

    pub fn where_not_null(self, col: &str) -> Self {
        self.push(JoinOp::And, Condition::IsNotNull(col.into()))
    }

    pub fn where_in(self, col: &str, vals: Vec<impl Into<SqlValue>>) -> Self {
        self.push(
            JoinOp::And,
            Condition::In(col.into(), vals.into_iter().map(Into::into).collect()),
        )
    }

    pub fn where_not_in(self, col: &str, vals: Vec<impl Into<SqlValue>>) -> Self {
        self.push(
            JoinOp::And,
            Condition::NotIn(col.into(), vals.into_iter().map(Into::into).collect()),
        )
    }

    pub fn where_between(
        self,
        col: &str,
        lo: impl Into<SqlValue>,
        hi: impl Into<SqlValue>,
    ) -> Self {
        self.push(
            JoinOp::And,
            Condition::Between(col.into(), lo.into(), hi.into()),
        )
    }

    pub fn where_not_between(
        self,
        col: &str,
        lo: impl Into<SqlValue>,
        hi: impl Into<SqlValue>,
    ) -> Self {
        self.push(
            JoinOp::And,
            Condition::NotBetween(col.into(), lo.into(), hi.into()),
        )
    }

    pub fn where_raw(self, sql: &str) -> Self {
        self.push(JoinOp::And, Condition::Raw(sql.into()))
    }

    /// Add `AND col ILIKE pattern` (case-insensitive LIKE, PostgreSQL only).
    pub fn where_ilike(self, col: &str, pattern: &str) -> Self {
        self.push(JoinOp::And, Condition::ILike(col.into(), pattern.into()))
    }

    /// Add `OR col ILIKE pattern`.
    pub fn or_where_ilike(self, col: &str, pattern: &str) -> Self {
        self.push(JoinOp::Or, Condition::ILike(col.into(), pattern.into()))
    }

    /// Add `AND col->>'key' = val` (JSONB text extraction, PostgreSQL).
    pub fn where_json(self, col: &str, key: &str, val: impl Into<SqlValue>) -> Self {
        self.push(
            JoinOp::And,
            Condition::JsonGet(col.into(), key.into(), val.into()),
        )
    }

    /// Add `OR col->>'key' = val`.
    pub fn or_where_json(self, col: &str, key: &str, val: impl Into<SqlValue>) -> Self {
        self.push(
            JoinOp::Or,
            Condition::JsonGet(col.into(), key.into(), val.into()),
        )
    }

    /// Add `AND col @> 'json_val'::jsonb` (JSONB containment, PostgreSQL).
    pub fn where_json_contains(self, col: &str, json_val: &str) -> Self {
        self.push(
            JoinOp::And,
            Condition::Raw(format!("{col} @> '{json_val}'::jsonb")),
        )
    }

    /// Add `AND col <op> val` — ergonomic alias for `where_op`.
    ///
    /// ```rust
    /// use rok_orm_core::QueryBuilder;
    /// let (sql, _) = QueryBuilder::<()>::new("users").where_column("age", ">", 18i64).to_sql();
    /// assert!(sql.contains("age > $1"));
    /// ```
    pub fn where_column(self, col: &str, op: &str, val: impl Into<SqlValue>) -> Self {
        self.where_op(col, op, val)
    }

    /// Add a comparison condition with an explicit operator string (`=`, `!=`, `>`, `>=`, `<`, `<=`).
    pub fn where_op(self, col: &str, op: &str, val: impl Into<SqlValue>) -> Self {
        let cond = match op {
            "=" | "==" => Condition::Eq(col.into(), val.into()),
            "!=" | "<>" => Condition::Ne(col.into(), val.into()),
            ">" => Condition::Gt(col.into(), val.into()),
            ">=" => Condition::Gte(col.into(), val.into()),
            "<" => Condition::Lt(col.into(), val.into()),
            "<=" => Condition::Lte(col.into(), val.into()),
            other => Condition::Raw(format!("{col} {other} {}", val.into())),
        };
        self.push(JoinOp::And, cond)
    }

    /// Add a grouped sub-condition: `AND (sub_cond1 AND/OR sub_cond2 …)`.
    ///
    /// The closure receives a fresh `QueryBuilder` to build the sub-conditions.
    /// Only the conditions of that builder are used (table/select/order etc. are ignored).
    pub fn where_group<F>(self, f: F) -> Self
    where
        F: FnOnce(QueryBuilder<T>) -> QueryBuilder<T>,
    {
        let inner_builder = f(QueryBuilder::new(""));
        if inner_builder.conditions.is_empty() {
            return self;
        }
        self.push(JoinOp::And, Condition::Group(inner_builder.conditions))
    }

    /// Add an OR grouped sub-condition: `OR (sub_cond1 AND/OR sub_cond2 …)`.
    pub fn or_where_group<F>(self, f: F) -> Self
    where
        F: FnOnce(QueryBuilder<T>) -> QueryBuilder<T>,
    {
        let inner_builder = f(QueryBuilder::new(""));
        if inner_builder.conditions.is_empty() {
            return self;
        }
        self.push(JoinOp::Or, Condition::Group(inner_builder.conditions))
    }

    // ── OR conditions ─────────────────────────────────────────────────────

    /// Add `OR col = val` — ergonomic shorthand for `or_where_eq`.
    pub fn or_where(self, col: &str, val: impl Into<SqlValue>) -> Self {
        self.or_where_eq(col, val)
    }

    pub fn or_where_eq(self, col: &str, val: impl Into<SqlValue>) -> Self {
        self.push(JoinOp::Or, Condition::Eq(col.into(), val.into()))
    }

    pub fn or_where_ne(self, col: &str, val: impl Into<SqlValue>) -> Self {
        self.push(JoinOp::Or, Condition::Ne(col.into(), val.into()))
    }

    pub fn or_where_gt(self, col: &str, val: impl Into<SqlValue>) -> Self {
        self.push(JoinOp::Or, Condition::Gt(col.into(), val.into()))
    }

    pub fn or_where_gte(self, col: &str, val: impl Into<SqlValue>) -> Self {
        self.push(JoinOp::Or, Condition::Gte(col.into(), val.into()))
    }

    pub fn or_where_lt(self, col: &str, val: impl Into<SqlValue>) -> Self {
        self.push(JoinOp::Or, Condition::Lt(col.into(), val.into()))
    }

    pub fn or_where_lte(self, col: &str, val: impl Into<SqlValue>) -> Self {
        self.push(JoinOp::Or, Condition::Lte(col.into(), val.into()))
    }

    pub fn or_where_like(self, col: &str, pattern: &str) -> Self {
        self.push(JoinOp::Or, Condition::Like(col.into(), pattern.into()))
    }

    pub fn or_where_null(self, col: &str) -> Self {
        self.push(JoinOp::Or, Condition::IsNull(col.into()))
    }

    pub fn or_where_not_null(self, col: &str) -> Self {
        self.push(JoinOp::Or, Condition::IsNotNull(col.into()))
    }

    pub fn or_where_in(self, col: &str, vals: Vec<impl Into<SqlValue>>) -> Self {
        self.push(
            JoinOp::Or,
            Condition::In(col.into(), vals.into_iter().map(Into::into).collect()),
        )
    }

    pub fn or_where_between(
        self,
        col: &str,
        lo: impl Into<SqlValue>,
        hi: impl Into<SqlValue>,
    ) -> Self {
        self.push(
            JoinOp::Or,
            Condition::Between(col.into(), lo.into(), hi.into()),
        )
    }

    pub fn or_where_raw(self, sql: &str) -> Self {
        self.push(JoinOp::Or, Condition::Raw(sql.into()))
    }

    // ── ordering ──────────────────────────────────────────────────────────

    pub fn order_by(mut self, col: &str) -> Self {
        self.order.push((col.into(), OrderDir::Asc));
        self
    }

    pub fn order_by_desc(mut self, col: &str) -> Self {
        self.order.push((col.into(), OrderDir::Desc));
        self
    }

    /// Append a raw ORDER BY expression (e.g. `"NULLS LAST, score DESC"`).
    /// Rendered after any column-based orders.
    pub fn order_by_raw(mut self, expr: &str) -> Self {
        self.order_raw = Some(expr.to_string());
        self
    }

    /// Append multiple column orders at once.
    pub fn order_by_many(mut self, cols: &[(&str, OrderDir)]) -> Self {
        for (col, dir) in cols {
            self.order.push((col.to_string(), *dir));
        }
        self
    }

    /// Replace all existing orders with `col ASC`.
    pub fn reorder(mut self, col: &str) -> Self {
        self.order.clear();
        self.order_raw = None;
        self.order.push((col.into(), OrderDir::Asc));
        self
    }

    /// Replace all existing orders with `col DESC`.
    pub fn reorder_desc(mut self, col: &str) -> Self {
        self.order.clear();
        self.order_raw = None;
        self.order.push((col.into(), OrderDir::Desc));
        self
    }

    // ── pagination ────────────────────────────────────────────────────────

    pub fn limit(mut self, n: usize) -> Self {
        self.limit_val = Some(n);
        self
    }

    pub fn offset(mut self, n: usize) -> Self {
        self.offset_val = Some(n);
        self
    }

    // ── SQL generation ────────────────────────────────────────────────────

    /// Build a parameterized `SELECT` statement (PostgreSQL `$N` placeholders).
    ///
    /// Returns `(sql, params)` — params are ordered to match `$1`, `$2`, …
    ///
    /// For SQLite use [`to_sql_with_dialect(Dialect::Sqlite)`](Self::to_sql_with_dialect).
    pub fn to_sql(&self) -> (String, Vec<SqlValue>) {
        self.to_sql_with_dialect(Dialect::Postgres)
    }

    /// Build a parameterized `SELECT` statement for the given [`Dialect`].
    ///
    /// - [`Dialect::Postgres`] emits `$1, $2, …`
    /// - [`Dialect::Sqlite`]   emits `?, ?, …`
    pub fn to_sql_with_dialect(&self, dialect: Dialect) -> (String, Vec<SqlValue>) {
        let base_cols = if let Some(raw) = &self.select_raw {
            raw.clone()
        } else {
            self.select_cols
                .as_ref()
                .map(|c| c.join(", "))
                .unwrap_or_else(|| "*".into())
        };
        let cols = if self.extra_select_exprs.is_empty() {
            base_cols
        } else {
            format!("{}, {}", base_cols, self.extra_select_exprs.join(", "))
        };

        let distinct_kw = if !self.distinct_on_cols.is_empty() {
            format!("DISTINCT ON ({}) ", self.distinct_on_cols.join(", "))
        } else if self.distinct {
            "DISTINCT ".to_string()
        } else {
            String::new()
        };
        let mut params: Vec<SqlValue> = Vec::new();

        let cte_prefix = self.build_ctes(dialect, &mut params);
        let mut sql = format!("SELECT {distinct_kw}{cols} FROM {}", self.table);

        sql.push_str(&self.build_joins());
        sql.push_str(&self.build_where_dialect(dialect, &mut params));
        sql.push_str(&self.build_group_by());
        sql.push_str(&self.build_windows());
        sql.push_str(&self.build_order());

        if let Some(n) = self.limit_val {
            sql.push_str(&format!(" LIMIT {n}"));
        }
        if let Some(n) = self.offset_val {
            sql.push_str(&format!(" OFFSET {n}"));
        }

        if let Some(lock) = self.lock {
            sql.push(' ');
            sql.push_str(lock.as_sql());
            if let Some(wait) = self.lock_wait {
                sql.push(' ');
                sql.push_str(wait.as_sql());
            }
        }

        sql = cte_prefix + &sql;

        if let Some((op, rhs)) = &self.set_op {
            let (rhs_sql, rhs_params) = rhs.to_sql_with_dialect(dialect);
            let param_offset = params.len();
            let rhs_renumbered = if dialect == Dialect::Postgres && param_offset > 0 {
                renumber_params(&rhs_sql, param_offset)
            } else {
                rhs_sql
            };
            let rhs_clean = rhs_renumbered
                .trim_start()
                .strip_prefix("SELECT")
                .map(|s| s.trim_start())
                .unwrap_or(&rhs_renumbered);
            sql = format!("{} {} SELECT {}", sql, op.as_sql(), rhs_clean);
            params.extend(rhs_params);
        }

        (sql, params)
    }

    /// Build a `SELECT COUNT(*) FROM …` statement (PostgreSQL dialect).
    pub fn to_count_sql(&self) -> (String, Vec<SqlValue>) {
        self.to_count_sql_with_dialect(Dialect::Postgres)
    }

    /// Build a `SELECT COUNT(*) FROM …` statement for the given dialect.
    pub fn to_count_sql_with_dialect(&self, dialect: Dialect) -> (String, Vec<SqlValue>) {
        let mut params: Vec<SqlValue> = Vec::new();
        let cte_prefix = self.build_ctes(dialect, &mut params);
        let joins = self.build_joins();
        let where_clause = self.build_where_dialect(dialect, &mut params);
        (
            format!(
                "{}SELECT COUNT(*) FROM {}{}{}",
                cte_prefix, self.table, joins, where_clause
            ),
            params,
        )
    }

    /// Build a `DELETE FROM … WHERE …` statement (PostgreSQL dialect).
    pub fn to_delete_sql(&self) -> (String, Vec<SqlValue>) {
        self.to_delete_sql_with_dialect(Dialect::Postgres)
    }

    /// Build a `DELETE FROM … WHERE …` statement for the given dialect.
    pub fn to_delete_sql_with_dialect(&self, dialect: Dialect) -> (String, Vec<SqlValue>) {
        assert!(
            !self.conditions.is_empty(),
            "refusing to DELETE from {} without a WHERE clause — this would delete all rows in the table. Use on_write_db() if you really intend this.",
            self.table,
        );
        let mut params: Vec<SqlValue> = Vec::new();
        let cte_prefix = self.build_ctes(dialect, &mut params);
        let where_clause = self.build_where_dialect(dialect, &mut params);
        (
            format!("{}DELETE FROM {}{}", cte_prefix, self.table, where_clause),
            params,
        )
    }

    /// Build an `UPDATE … SET … WHERE …` statement (PostgreSQL dialect).
    pub fn to_update_sql(&self, data: &[(&str, SqlValue)]) -> (String, Vec<SqlValue>) {
        self.to_update_sql_with_dialect(Dialect::Postgres, data)
    }

    /// Build an `UPDATE … SET … WHERE …` statement for the given dialect.
    pub fn to_update_sql_with_dialect(
        &self,
        dialect: Dialect,
        data: &[(&str, SqlValue)],
    ) -> (String, Vec<SqlValue>) {
        assert!(
            !self.conditions.is_empty(),
            "refusing to UPDATE {} without a WHERE clause — this would update all rows in the table. Use on_write_db() if you really intend this.",
            self.table,
        );
        let mut params: Vec<SqlValue> = Vec::new();
        let cte_prefix = self.build_ctes(dialect, &mut params);
        let set_clauses: Vec<String> = data
            .iter()
            .enumerate()
            .map(|(i, (col, val))| {
                params.push(val.clone());
                match dialect {
                    Dialect::Postgres => format!("{col} = ${}", i + 1),
                    Dialect::Sqlite => format!("{col} = ?"),
                }
            })
            .collect();

        let mut sql = format!(
            "{}UPDATE {} SET {}",
            cte_prefix,
            self.table,
            set_clauses.join(", ")
        );
        sql.push_str(&self.build_where_dialect(dialect, &mut params));
        (sql, params)
    }

    // ── static helpers ────────────────────────────────────────────────────

    /// Build an `INSERT INTO` statement (PostgreSQL `$N` placeholders).
    pub fn insert_sql(table: &str, data: &[(&str, SqlValue)]) -> (String, Vec<SqlValue>) {
        Self::insert_sql_with_dialect(Dialect::Postgres, table, data)
    }

    /// Build an `INSERT INTO` statement for the given dialect.
    pub fn insert_sql_with_dialect(
        dialect: Dialect,
        table: &str,
        data: &[(&str, SqlValue)],
    ) -> (String, Vec<SqlValue>) {
        let cols: Vec<&str> = data.iter().map(|(c, _)| *c).collect();
        let placeholders: Vec<String> = match dialect {
            Dialect::Postgres => (1..=data.len()).map(|i| format!("${i}")).collect(),
            Dialect::Sqlite => (0..data.len()).map(|_| "?".to_string()).collect(),
        };
        let params: Vec<SqlValue> = data.iter().map(|(_, v)| v.clone()).collect();
        (
            format!(
                "INSERT INTO {table} ({}) VALUES ({})",
                cols.join(", "),
                placeholders.join(", ")
            ),
            params,
        )
    }

    /// Build an `INSERT INTO … VALUES …, …` statement for multiple rows.
    ///
    /// All rows must have the same columns in the same order as the first row.
    ///
    /// ```rust
    /// use rok_orm_core::{QueryBuilder, SqlValue};
    ///
    /// let rows: Vec<Vec<(&str, SqlValue)>> = vec![
    ///     vec![("name", "Alice".into()), ("email", "a@a.com".into())],
    ///     vec![("name", "Bob".into()),   ("email", "b@b.com".into())],
    /// ];
    /// let (sql, params) = QueryBuilder::<()>::bulk_insert_sql("users", &rows);
    /// assert!(sql.contains("($1, $2), ($3, $4)"));
    /// assert_eq!(params.len(), 4);
    /// ```
    pub fn bulk_insert_sql(table: &str, rows: &[Vec<(&str, SqlValue)>]) -> (String, Vec<SqlValue>) {
        assert!(
            !rows.is_empty(),
            "bulk_insert_sql requires at least one row"
        );
        let cols: Vec<&str> = rows[0].iter().map(|(c, _)| *c).collect();
        let mut params: Vec<SqlValue> = Vec::new();
        let mut value_groups: Vec<String> = Vec::new();
        let mut offset = 1usize;

        for row in rows {
            let placeholders: Vec<String> = (offset..offset + row.len())
                .map(|i| format!("${i}"))
                .collect();
            value_groups.push(format!("({})", placeholders.join(", ")));
            for (_, v) in row.iter() {
                params.push(v.clone());
            }
            offset += row.len();
        }

        (
            format!(
                "INSERT INTO {table} ({}) VALUES {}",
                cols.join(", "),
                value_groups.join(", ")
            ),
            params,
        )
    }

    /// Build an `UPDATE … SET … WHERE …` statement from explicit conditions.
    ///
    /// Prefer `to_update_sql` when you already have a `QueryBuilder`.
    pub fn update_sql(
        table: &str,
        data: &[(&str, SqlValue)],
        conditions: &[(JoinOp, Condition)],
    ) -> (String, Vec<SqlValue>) {
        let mut params: Vec<SqlValue> = Vec::new();
        let set_clauses: Vec<String> = data
            .iter()
            .enumerate()
            .map(|(i, (col, val))| {
                params.push(val.clone());
                format!("{col} = ${}", i + 1)
            })
            .collect();

        let mut sql = format!("UPDATE {table} SET {}", set_clauses.join(", "));

        if !conditions.is_empty() {
            let where_frag = build_where_from(conditions, &mut params);
            sql.push_str(&where_frag);
        }

        (sql, params)
    }

    // ── conditional chaining ──────────────────────────────────────────────────

    /// Apply `f(self)` only when `condition` is `true`; otherwise pass through unchanged.
    ///
    /// ```rust
    /// use rok_orm_core::QueryBuilder;
    ///
    /// let active_only = true;
    /// let (sql, _) = QueryBuilder::<()>::new("users")
    ///     .when(active_only, |q| q.where_eq("active", true))
    ///     .to_sql();
    /// assert!(sql.contains("WHERE active = $1"));
    /// ```
    pub fn when<F>(self, condition: bool, f: F) -> Self
    where
        F: FnOnce(Self) -> Self,
    {
        if condition {
            f(self)
        } else {
            self
        }
    }

    /// Apply `f(self, val)` when `opt` is `Some(val)`; otherwise pass through unchanged.
    ///
    /// ```rust
    /// use rok_orm_core::QueryBuilder;
    ///
    /// let role: Option<&str> = Some("admin");
    /// let (sql, _) = QueryBuilder::<()>::new("users")
    ///     .when_some(role, |q, r| q.where_eq("role", r))
    ///     .to_sql();
    /// assert!(sql.contains("WHERE role = $1"));
    /// ```
    pub fn when_some<V, F>(self, opt: Option<V>, f: F) -> Self
    where
        F: FnOnce(Self, V) -> Self,
    {
        match opt {
            Some(v) => f(self, v),
            None => self,
        }
    }

    /// Push an arbitrary [`Condition`] with the given join operator.
    ///
    /// Useful when building conditions programmatically (e.g. `Condition::Subquery`).
    pub fn push_condition(self, op: JoinOp, cond: Condition) -> Self {
        self.push(op, cond)
    }

    // ── CTE (Common Table Expressions) ────────────────────────────────────────

    /// Add a `WITH name (columns) AS (subquery_sql)` before the main SELECT.
    pub fn with_cte(mut self, name: &str, columns: &[&str], subquery: QueryBuilder<T>) -> Self {
        self.ctes.push(CteDef {
            name: name.to_string(),
            columns: columns.iter().map(|s| s.to_string()).collect(),
            subquery,
            recursive: false,
        });
        self
    }

    /// Add a `WITH RECURSIVE name (columns) AS (subquery_sql)` before the main SELECT.
    pub fn with_recursive_cte(
        mut self,
        name: &str,
        columns: &[&str],
        subquery: QueryBuilder<T>,
    ) -> Self {
        self.ctes.push(CteDef {
            name: name.to_string(),
            columns: columns.iter().map(|s| s.to_string()).collect(),
            subquery,
            recursive: true,
        });
        self
    }

    // ── Window Functions ───────────────────────────────────────────────────────

    /// Store a named window definition.
    ///
    /// Renders as `WINDOW name AS (PARTITION BY ... ORDER BY ...)` after HAVING.
    pub fn window(mut self, name: &str, partition: &[&str], order: &[(&str, OrderDir)]) -> Self {
        self.windows.push(WindowDef {
            name: name.to_string(),
            partition: partition.iter().map(|s| s.to_string()).collect(),
            order: order.iter().map(|(c, d)| (c.to_string(), *d)).collect(),
        });
        self
    }

    /// Add a window function expression to the SELECT list.
    ///
    /// Renders as `window_fn(col) OVER window_name` in the column list.
    pub fn select_with_window(mut self, col: &str, window_fn: &str, window_name: &str) -> Self {
        self.extra_select_exprs
            .push(format!("{window_fn}({col}) OVER {window_name}"));
        self
    }

    // ── Full-Text Search ───────────────────────────────────────────────────────

    /// Add `AND col @@ to_tsquery(query)` (PostgreSQL) or `col MATCH ?` (SQLite).
    pub fn where_fts(self, col: &str, query: &str) -> Self {
        self.push(
            JoinOp::And,
            Condition::FullTextMatch {
                col: col.into(),
                query: query.into(),
                config: None,
            },
        )
    }

    /// Add `AND col @@ to_tsquery('config', query)` with text search configuration.
    pub fn where_fts_with_config(self, col: &str, query: &str, config: &str) -> Self {
        self.push(
            JoinOp::And,
            Condition::FullTextMatch {
                col: col.into(),
                query: query.into(),
                config: Some(config.into()),
            },
        )
    }

    /// Add `ORDER BY ts_rank(to_tsvector(col), to_tsquery(...)) DESC`.
    pub fn order_by_rank(mut self, col: &str, query: &str, config: Option<&str>) -> Self {
        let config_fragment = config
            .map(|c| format!("'{}', ", c.replace('\'', "''")))
            .unwrap_or_default();
        let rank_expr =
            format!("ts_rank(to_tsvector({col}), to_tsquery({config_fragment}'{query}')) DESC");
        self.order.push((rank_expr, OrderDir::Desc));
        self
    }

    // ── Read-replica routing ──────────────────────────────────────────────────

    /// Force this query to the **write** (primary) pool, bypassing read replicas.
    pub fn on_write_db(mut self) -> Self {
        self.use_replica = false;
        self
    }

    // ── Row-level locking ──────────────────────────────────────────────────

    /// Append `FOR UPDATE` at the end of the SELECT statement.
    pub fn for_update(mut self) -> Self {
        self.lock = Some(LockClause::ForUpdate);
        self
    }

    /// Append `FOR NO KEY UPDATE` at the end of the SELECT statement.
    pub fn for_no_key_update(mut self) -> Self {
        self.lock = Some(LockClause::ForNoKeyUpdate);
        self
    }

    /// Append `FOR SHARE` at the end of the SELECT statement.
    pub fn for_share(mut self) -> Self {
        self.lock = Some(LockClause::ForShare);
        self
    }

    /// Append `FOR KEY SHARE` at the end of the SELECT statement.
    pub fn for_key_share(mut self) -> Self {
        self.lock = Some(LockClause::ForKeyShare);
        self
    }

    /// Append `NOWAIT` after the locking clause.
    pub fn nowait(mut self) -> Self {
        self.lock_wait = Some(LockWait::NoWait);
        self
    }

    /// Append `SKIP LOCKED` after the locking clause.
    pub fn skip_locked(mut self) -> Self {
        self.lock_wait = Some(LockWait::SkipLocked);
        self
    }

    // ── Set operations (UNION / INTERSECT / EXCEPT) ─────────────────────────

    /// Append `UNION (rhs_sql)`.
    pub fn union(self, rhs: QueryBuilder<T>) -> Self {
        self.set_op_impl(SetOp::Union, rhs)
    }

    /// Append `UNION ALL (rhs_sql)`.
    pub fn union_all(self, rhs: QueryBuilder<T>) -> Self {
        self.set_op_impl(SetOp::UnionAll, rhs)
    }

    /// Append `INTERSECT (rhs_sql)`.
    pub fn intersect(self, rhs: QueryBuilder<T>) -> Self {
        self.set_op_impl(SetOp::Intersect, rhs)
    }

    /// Append `INTERSECT ALL (rhs_sql)`.
    pub fn intersect_all(self, rhs: QueryBuilder<T>) -> Self {
        self.set_op_impl(SetOp::IntersectAll, rhs)
    }

    /// Append `EXCEPT (rhs_sql)`.
    pub fn except(self, rhs: QueryBuilder<T>) -> Self {
        self.set_op_impl(SetOp::Except, rhs)
    }

    /// Append `EXCEPT ALL (rhs_sql)`.
    pub fn except_all(self, rhs: QueryBuilder<T>) -> Self {
        self.set_op_impl(SetOp::ExceptAll, rhs)
    }

    fn set_op_impl(mut self, op: SetOp, rhs: QueryBuilder<T>) -> Self {
        self.set_op = Some((op, Box::new(rhs)));
        self
    }

    // ── pgvector: vector distance queries ─────────────────────────────────────

    /// KNN search: `ORDER BY {col} <-> '[…]'::vector LIMIT k` (L2 distance).
    pub fn nearest_to(self, col: &str, embedding: &[f32], k: usize) -> Self {
        let vec_lit = format_vector(embedding);
        self.order_by_raw(&format!("{col} <-> '{vec_lit}'::vector"))
            .limit(k)
    }

    /// `WHERE {col} <=> '[…]'::vector {op} {threshold}` (cosine distance).
    pub fn where_cosine_distance(
        self,
        col: &str,
        embedding: &[f32],
        op: &str,
        threshold: f64,
    ) -> Self {
        let vec_lit = format_vector(embedding);
        self.where_raw(&format!("{col} <=> '{vec_lit}'::vector {op} {threshold}"))
    }

    /// `WHERE {col} <-> '[…]'::vector {op} {threshold}` (L2 distance filter).
    pub fn where_vector_distance(
        self,
        col: &str,
        embedding: &[f32],
        op: &str,
        threshold: f64,
    ) -> Self {
        let vec_lit = format_vector(embedding);
        self.where_raw(&format!("{col} <-> '{vec_lit}'::vector {op} {threshold}"))
    }

    /// `WHERE {col} <#> '[…]'::vector {op} {threshold}` (negative inner product).
    pub fn where_inner_product(
        self,
        col: &str,
        embedding: &[f32],
        op: &str,
        threshold: f64,
    ) -> Self {
        let vec_lit = format_vector(embedding);
        self.where_raw(&format!("{col} <#> '{vec_lit}'::vector {op} {threshold}"))
    }

    // ── internals ─────────────────────────────────────────────────────────

    fn push(mut self, op: JoinOp, cond: Condition) -> Self {
        self.conditions.push((op, cond));
        self
    }

    fn build_joins(&self) -> String {
        let mut out = String::new();
        for join in &self.joins {
            match join {
                Join::Inner(t, on) => out.push_str(&format!(" INNER JOIN {t} ON {on}")),
                Join::Left(t, on) => out.push_str(&format!(" LEFT JOIN {t} ON {on}")),
                Join::Right(t, on) => out.push_str(&format!(" RIGHT JOIN {t} ON {on}")),
                Join::Raw(raw) => {
                    out.push(' ');
                    out.push_str(raw);
                }
            }
        }
        out
    }

    fn build_where_dialect(&self, dialect: Dialect, params: &mut Vec<SqlValue>) -> String {
        build_where_from_dialect(dialect, &self.conditions, params)
    }

    fn build_group_by(&self) -> String {
        let mut out = String::new();
        if !self.group_by.is_empty() {
            out.push_str(&format!(" GROUP BY {}", self.group_by.join(", ")));
        }
        if let Some(ref h) = self.having {
            out.push_str(&format!(" HAVING {h}"));
        }
        out
    }

    fn build_order(&self) -> String {
        let mut parts: Vec<String> = self
            .order
            .iter()
            .map(|(col, dir)| format!("{col} {dir}"))
            .collect();
        if let Some(raw) = &self.order_raw {
            parts.push(raw.clone());
        }
        if parts.is_empty() {
            return String::new();
        }
        format!(" ORDER BY {}", parts.join(", "))
    }

    /// Expose the raw conditions (useful for callers that need to inspect them).
    pub fn conditions(&self) -> &[(JoinOp, Condition)] {
        &self.conditions
    }

    /// Build only the `WHERE …` fragment and its bound parameters.
    ///
    /// Useful when you need to compose a raw `UPDATE … SET … WHERE …` or similar
    /// statement while still leveraging the query builder's condition logic.
    ///
    /// Returns `("", vec![])` when there are no conditions.
    pub fn to_where_clause(&self) -> (String, Vec<SqlValue>) {
        let mut params = Vec::new();
        let clause = self.build_where_dialect(Dialect::Postgres, &mut params);
        (clause, params)
    }

    /// Build `SELECT <agg_expr> FROM … WHERE …` for aggregates like `MAX(col)`, `SUM(col)`.
    ///
    /// `agg_expr` is a raw SQL expression, e.g. `"MAX(total)"` or `"SUM(price)"`.
    pub fn to_aggregate_sql(&self, agg_expr: &str) -> (String, Vec<SqlValue>) {
        let mut params: Vec<SqlValue> = Vec::new();
        let joins = self.build_joins();
        let where_clause = self.build_where_dialect(Dialect::Postgres, &mut params);
        (
            format!(
                "SELECT {agg_expr} FROM {}{}{}",
                self.table, joins, where_clause
            ),
            params,
        )
    }

    fn build_ctes(&self, dialect: Dialect, params: &mut Vec<SqlValue>) -> String {
        if self.ctes.is_empty() {
            return String::new();
        }
        let keyword = if self.ctes.iter().any(|c| c.recursive) {
            "WITH RECURSIVE "
        } else {
            "WITH "
        };
        let mut parts: Vec<String> = Vec::new();
        for cte in &self.ctes {
            let (sub_sql, sub_params) = cte.subquery.to_sql_with_dialect(dialect);
            let offset = params.len();
            let adjusted_sql = if offset > 0 {
                renumber_params(&sub_sql, offset)
            } else {
                sub_sql
            };
            params.extend(sub_params);
            let cols = if cte.columns.is_empty() {
                String::new()
            } else {
                format!(" ({})", cte.columns.join(", "))
            };
            parts.push(format!("{}{} AS ({adjusted_sql})", cte.name, cols));
        }
        format!("{keyword}{} ", parts.join(", "))
    }

    fn build_windows(&self) -> String {
        if self.windows.is_empty() {
            return String::new();
        }
        let parts: Vec<String> = self
            .windows
            .iter()
            .map(|w| {
                let partition = if w.partition.is_empty() {
                    String::new()
                } else {
                    format!(" PARTITION BY {}", w.partition.join(", "))
                };
                let order = if w.order.is_empty() {
                    String::new()
                } else {
                    let order_parts: Vec<String> =
                        w.order.iter().map(|(c, d)| format!("{c} {d}")).collect();
                    format!(" ORDER BY {}", order_parts.join(", "))
                };
                format!("{} AS ({partition}{order})", w.name)
            })
            .collect();
        format!(" WINDOW {}", parts.join(", "))
    }

    /// Build an `INSERT INTO … ON CONFLICT (cols) DO UPDATE SET …` statement (PostgreSQL).
    ///
    /// - `data` — column-value pairs to insert
    /// - `conflict_cols` — the conflict target columns
    /// - `update_cols` — which columns to update on conflict (defaults to all `data` columns
    ///   that are not in `conflict_cols` when empty)
    pub fn upsert_sql(
        table: &str,
        data: &[(&str, SqlValue)],
        conflict_cols: &[&str],
    ) -> (String, Vec<SqlValue>) {
        let cols: Vec<&str> = data.iter().map(|(c, _)| *c).collect();
        let placeholders: Vec<String> = (1..=data.len()).map(|i| format!("${i}")).collect();
        let params: Vec<SqlValue> = data.iter().map(|(_, v)| v.clone()).collect();

        let conflict_target = conflict_cols.join(", ");
        let update_set: Vec<String> = cols
            .iter()
            .filter(|c| !conflict_cols.contains(c))
            .map(|c| format!("{c} = EXCLUDED.{c}"))
            .collect();

        let sql = if update_set.is_empty() {
            format!(
                "INSERT INTO {table} ({}) VALUES ({}) ON CONFLICT ({conflict_target}) DO NOTHING",
                cols.join(", "),
                placeholders.join(", "),
            )
        } else {
            format!(
                "INSERT INTO {table} ({}) VALUES ({}) ON CONFLICT ({conflict_target}) DO UPDATE SET {}",
                cols.join(", "),
                placeholders.join(", "),
                update_set.join(", "),
            )
        };

        (sql, params)
    }
}

/// Encode a `f32` slice as a pgvector literal string, e.g. `"[1.0,2.0,3.0]"`.
fn format_vector(v: &[f32]) -> String {
    let elems: Vec<String> = v.iter().map(|x| x.to_string()).collect();
    format!("[{}]", elems.join(","))
}

/// Renumber `$N` parameter placeholders by adding `offset` to each number.
fn renumber_params(sql: &str, offset: usize) -> String {
    if offset == 0 {
        return sql.to_string();
    }
    let mut result = String::with_capacity(sql.len());
    let chars: Vec<char> = sql.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '$' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
            let start = i + 1;
            let mut end = start;
            while end < chars.len() && chars[end].is_ascii_digit() {
                end += 1;
            }
            let num: usize = chars[start..end]
                .iter()
                .collect::<String>()
                .parse()
                .unwrap_or(0);
            result.push_str(&format!("${}", num + offset));
            i = end;
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

fn build_where_from(conditions: &[(JoinOp, Condition)], params: &mut Vec<SqlValue>) -> String {
    build_where_from_dialect(Dialect::Postgres, conditions, params)
}

fn build_where_from_dialect(
    dialect: Dialect,
    conditions: &[(JoinOp, Condition)],
    params: &mut Vec<SqlValue>,
) -> String {
    if conditions.is_empty() {
        return String::new();
    }
    let mut out = " WHERE ".to_string();
    for (idx, (op, cond)) in conditions.iter().enumerate() {
        let (frag, ps) = match dialect {
            Dialect::Postgres => cond.to_param_sql(params.len() + 1),
            Dialect::Sqlite => cond.to_param_sql_sqlite(),
        };
        params.extend(ps);
        if idx > 0 {
            out.push(' ');
            out.push_str(&op.to_string());
            out.push(' ');
        }
        out.push_str(&frag);
    }
    out
}

// ── tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_select() {
        let (sql, params) = QueryBuilder::<()>::new("users").to_sql();
        assert_eq!(sql, "SELECT * FROM users");
        assert!(params.is_empty());
    }

    #[test]
    fn distinct_select() {
        let (sql, _) = QueryBuilder::<()>::new("users").distinct().to_sql();
        assert!(sql.starts_with("SELECT DISTINCT * FROM users"));
    }

    #[test]
    fn where_eq_generates_param() {
        let (sql, params) = QueryBuilder::<()>::new("users")
            .where_eq("id", 42i64)
            .to_sql();
        assert!(sql.contains("WHERE id = $1"));
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], SqlValue::Integer(42));
    }

    #[test]
    fn multiple_conditions() {
        let (sql, params) = QueryBuilder::<()>::new("posts")
            .where_eq("active", true)
            .where_like("title", "%rust%")
            .to_sql();
        assert!(sql.contains("WHERE active = $1 AND title LIKE $2"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn or_conditions() {
        let (sql, params) = QueryBuilder::<()>::new("users")
            .where_eq("role", "admin")
            .or_where_eq("role", "moderator")
            .to_sql();
        assert!(sql.contains("WHERE role = $1 OR role = $2"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn where_between() {
        let (sql, params) = QueryBuilder::<()>::new("orders")
            .where_between("amount", 10i64, 100i64)
            .to_sql();
        assert!(sql.contains("amount BETWEEN $1 AND $2"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn where_not_in() {
        let (sql, params) = QueryBuilder::<()>::new("users")
            .where_not_in("status", vec!["banned", "deleted"])
            .to_sql();
        assert!(sql.contains("status NOT IN ($1, $2)"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn where_not_like() {
        let (sql, _) = QueryBuilder::<()>::new("users")
            .where_not_like("email", "%@spam.com")
            .to_sql();
        assert!(sql.contains("email NOT LIKE $1"));
    }

    #[test]
    fn to_update_sql() {
        let (sql, params) = QueryBuilder::<()>::new("users")
            .where_eq("id", 1i64)
            .to_update_sql(&[("name", "Bob".into()), ("active", true.into())]);
        assert!(sql.starts_with("UPDATE users SET name = $1, active = $2"));
        assert!(sql.contains("WHERE id = $3"));
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn order_limit_offset() {
        let (sql, _) = QueryBuilder::<()>::new("users")
            .order_by_desc("created_at")
            .order_by("name")
            .limit(10)
            .offset(20)
            .to_sql();
        assert!(sql.contains("ORDER BY created_at DESC, name ASC"));
        assert!(sql.contains("LIMIT 10"));
        assert!(sql.contains("OFFSET 20"));
    }

    #[test]
    fn count_sql() {
        let (sql, _) = QueryBuilder::<()>::new("users")
            .where_eq("active", true)
            .to_count_sql();
        assert!(sql.starts_with("SELECT COUNT(*) FROM users"));
    }

    #[test]
    fn delete_sql() {
        let (sql, params) = QueryBuilder::<()>::new("sessions")
            .where_eq("user_id", 5i64)
            .to_delete_sql();
        assert!(sql.contains("DELETE FROM sessions WHERE user_id = $1"));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn insert_sql() {
        let (sql, params) = QueryBuilder::<()>::insert_sql(
            "users",
            &[("name", "Alice".into()), ("email", "a@a.com".into())],
        );
        assert!(sql.contains("INSERT INTO users (name, email) VALUES ($1, $2)"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn where_in() {
        let (sql, params) = QueryBuilder::<()>::new("users")
            .where_in("id", vec![1i64, 2, 3])
            .to_sql();
        assert!(sql.contains("id IN ($1, $2, $3)"));
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn select_specific_columns() {
        let (sql, _) = QueryBuilder::<()>::new("users")
            .select(&["id", "email"])
            .to_sql();
        assert!(sql.starts_with("SELECT id, email FROM users"));
    }

    #[test]
    fn option_value_null() {
        let val: SqlValue = Option::<i64>::None.into();
        assert_eq!(val, SqlValue::Null);
    }

    #[test]
    fn option_value_some() {
        let val: SqlValue = Some(42i64).into();
        assert_eq!(val, SqlValue::Integer(42));
    }

    #[test]
    fn inner_join() {
        let (sql, _) = QueryBuilder::<()>::new("orders")
            .inner_join("users", "users.id = orders.user_id")
            .to_sql();
        assert!(sql.contains("INNER JOIN users ON users.id = orders.user_id"));
    }

    #[test]
    fn left_join_with_where() {
        let (sql, params) = QueryBuilder::<()>::new("orders")
            .left_join("users", "users.id = orders.user_id")
            .where_eq("orders.status", "paid")
            .to_sql();
        assert!(sql.contains("LEFT JOIN users ON users.id = orders.user_id"));
        assert!(sql.contains("WHERE orders.status = $1"));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn right_join() {
        let (sql, _) = QueryBuilder::<()>::new("orders")
            .right_join("products", "products.id = orders.product_id")
            .to_sql();
        assert!(sql.contains("RIGHT JOIN products ON products.id = orders.product_id"));
    }

    #[test]
    fn group_by_and_having() {
        let (sql, _) = QueryBuilder::<()>::new("orders")
            .select(&["user_id", "COUNT(*) as total"])
            .group_by(&["user_id"])
            .having("COUNT(*) > 5")
            .to_sql();
        assert!(sql.contains("GROUP BY user_id"));
        assert!(sql.contains("HAVING COUNT(*) > 5"));
        // GROUP BY must come before ORDER BY
        let gpos = sql.find("GROUP BY").unwrap();
        let hpos = sql.find("HAVING").unwrap();
        assert!(gpos < hpos);
    }

    #[test]
    fn count_sql_with_join() {
        let (sql, _) = QueryBuilder::<()>::new("orders")
            .inner_join("users", "users.id = orders.user_id")
            .where_eq("users.active", true)
            .to_count_sql();
        assert!(sql.contains("INNER JOIN users ON users.id = orders.user_id"));
        assert!(sql.contains("SELECT COUNT(*) FROM orders"));
    }

    #[test]
    fn bulk_insert_sql_two_rows() {
        let rows: Vec<Vec<(&str, SqlValue)>> = vec![
            vec![("name", "Alice".into()), ("email", "a@a.com".into())],
            vec![("name", "Bob".into()), ("email", "b@b.com".into())],
        ];
        let (sql, params) = QueryBuilder::<()>::bulk_insert_sql("users", &rows);
        assert!(sql.starts_with("INSERT INTO users (name, email) VALUES"));
        assert!(sql.contains("($1, $2), ($3, $4)"));
        assert_eq!(params.len(), 4);
    }

    #[test]
    fn bulk_insert_sql_single_row() {
        let rows = vec![vec![("x", SqlValue::Integer(1))]];
        let (sql, params) = QueryBuilder::<()>::bulk_insert_sql("t", &rows);
        assert!(sql.contains("($1)"));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn where_ilike() {
        let (sql, params) = QueryBuilder::<()>::new("users")
            .where_ilike("name", "alice%")
            .to_sql();
        assert!(sql.contains("name ILIKE $1"));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn where_op_gt() {
        let (sql, params) = QueryBuilder::<()>::new("users")
            .where_op("age", ">", 18i64)
            .to_sql();
        assert!(sql.contains("age > $1"));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn where_group_subquery() {
        let (sql, params) = QueryBuilder::<()>::new("users")
            .where_eq("active", true)
            .where_group(|q| q.where_eq("role", "admin").or_where_eq("role", "mod"))
            .to_sql();
        assert!(sql.contains("active = $1"));
        assert!(sql.contains("AND (role = $2 OR role = $3)"));
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn select_raw() {
        let (sql, _) = QueryBuilder::<()>::new("users")
            .select_raw("id, LOWER(email) as email_lower")
            .to_sql();
        assert!(sql.starts_with("SELECT id, LOWER(email) as email_lower FROM users"));
    }

    #[test]
    fn distinct_on() {
        let (sql, _) = QueryBuilder::<()>::new("users")
            .distinct_on(&["email"])
            .to_sql();
        assert!(sql.starts_with("SELECT DISTINCT ON (email) * FROM users"));
    }

    #[test]
    fn join_raw() {
        let (sql, _) = QueryBuilder::<()>::new("users")
            .join_raw("INNER JOIN subscriptions s ON s.user_id = users.id AND s.active = true")
            .to_sql();
        assert!(sql.contains("INNER JOIN subscriptions s ON s.user_id = users.id"));
    }

    #[test]
    fn order_by_raw() {
        let (sql, _) = QueryBuilder::<()>::new("users")
            .order_by_raw("NULLS LAST, score DESC")
            .to_sql();
        assert!(sql.contains("ORDER BY NULLS LAST, score DESC"));
    }

    #[test]
    fn order_by_many() {
        let (sql, _) = QueryBuilder::<()>::new("users")
            .order_by_many(&[("role", OrderDir::Asc), ("created_at", OrderDir::Desc)])
            .to_sql();
        assert!(sql.contains("ORDER BY role ASC, created_at DESC"));
    }

    #[test]
    fn reorder_clears_previous() {
        let (sql, _) = QueryBuilder::<()>::new("users")
            .order_by("name")
            .reorder_desc("created_at")
            .to_sql();
        assert!(sql.contains("ORDER BY created_at DESC"));
        assert!(!sql.contains("name"));
    }

    #[test]
    fn upsert_sql_basic() {
        let (sql, params) = QueryBuilder::<()>::upsert_sql(
            "users",
            &[("email", "x@y.com".into()), ("name", "Bob".into())],
            &["email"],
        );
        assert!(sql.contains("ON CONFLICT (email) DO UPDATE SET name = EXCLUDED.name"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn where_json_extraction() {
        let (sql, params) = QueryBuilder::<()>::new("users")
            .where_json("settings", "theme", "dark")
            .to_sql();
        assert!(sql.contains("settings->>'theme' = $1"), "sql={sql}");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], SqlValue::Text("dark".into()));
    }

    #[test]
    fn where_json_contains_raw() {
        let (sql, _) = QueryBuilder::<()>::new("posts")
            .where_json_contains("tags", r#"["rust"]"#)
            .to_sql();
        assert!(sql.contains(r#"tags @> '["rust"]'::jsonb"#), "sql={sql}");
    }

    #[test]
    fn to_aggregate_sql() {
        let (sql, params) = QueryBuilder::<()>::new("orders")
            .where_eq("user_id", 1i64)
            .to_aggregate_sql("MAX(total)");
        assert!(sql.starts_with("SELECT MAX(total) FROM orders"));
        assert!(sql.contains("WHERE user_id = $1"));
        assert_eq!(params.len(), 1);
    }

    // ── new v4 methods ────────────────────────────────────────────────────────

    #[test]
    fn when_applies_closure_when_true() {
        let (sql, params) = QueryBuilder::<()>::new("users")
            .when(true, |q| q.where_eq("active", true))
            .to_sql();
        assert!(sql.contains("WHERE active = $1"));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn when_noop_when_false() {
        let (sql, params) = QueryBuilder::<()>::new("users")
            .when(false, |q| q.where_eq("active", true))
            .to_sql();
        assert!(!sql.contains("WHERE"));
        assert!(params.is_empty());
    }

    #[test]
    fn when_some_applies_with_value() {
        let role: Option<&str> = Some("admin");
        let (sql, params) = QueryBuilder::<()>::new("users")
            .when_some(role, |q, r| q.where_eq("role", r))
            .to_sql();
        assert!(sql.contains("role = $1"));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn when_some_noop_when_none() {
        let role: Option<&str> = None;
        let (sql, params) = QueryBuilder::<()>::new("users")
            .when_some(role, |q, r| q.where_eq("role", r))
            .to_sql();
        assert!(!sql.contains("WHERE"));
        assert!(params.is_empty());
    }

    #[test]
    fn chained_when_calls() {
        let (sql, params) = QueryBuilder::<()>::new("users")
            .when(true, |q| q.where_eq("active", true))
            .when(true, |q| q.where_eq("role", "admin"))
            .when(false, |q| q.where_eq("deleted", true))
            .to_sql();
        assert!(sql.contains("active = $1"));
        assert!(sql.contains("role = $2"));
        assert!(!sql.contains("deleted"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn add_select_expr_appends_to_star() {
        let (sql, _) = QueryBuilder::<()>::new("users")
            .add_select_expr(
                "(SELECT COUNT(*) FROM posts WHERE posts.user_id = users.id) AS posts_count",
            )
            .to_sql();
        assert!(sql.starts_with("SELECT *, (SELECT COUNT(*)"));
    }

    #[test]
    fn add_select_expr_appends_to_cols() {
        let (sql, _) = QueryBuilder::<()>::new("users")
            .select(&["id", "email"])
            .add_select_expr("42 AS answer")
            .to_sql();
        assert!(sql.starts_with("SELECT id, email, 42 AS answer FROM users"));
    }

    #[test]
    fn multiple_add_select_exprs() {
        let (sql, _) = QueryBuilder::<()>::new("users")
            .add_select_expr("(SELECT COUNT(*) FROM posts WHERE posts.user_id = users.id) AS posts_count")
            .add_select_expr("(SELECT COUNT(*) FROM comments WHERE comments.user_id = users.id) AS comments_count")
            .to_sql();
        assert!(sql.contains("posts_count"));
        assert!(sql.contains("comments_count"));
    }

    #[test]
    fn where_column_alias() {
        let (sql, params) = QueryBuilder::<()>::new("users")
            .where_column("age", ">", 18i64)
            .to_sql();
        assert!(sql.contains("age > $1"));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn or_where_alias() {
        let (sql, params) = QueryBuilder::<()>::new("users")
            .where_eq("role", "admin")
            .or_where("role", "moderator")
            .to_sql();
        assert!(sql.contains("role = $1 OR role = $2"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn subquery_exists_no_inner() {
        use crate::condition::Condition;
        use crate::condition::JoinOp;
        let (sql, params) = QueryBuilder::<()>::new("users")
            .push_condition(
                JoinOp::And,
                Condition::Subquery {
                    exists: true,
                    table: "posts".to_string(),
                    fk_expr: "posts.user_id = users.id".to_string(),
                    inner: vec![],
                },
            )
            .to_sql();
        assert!(sql.contains("EXISTS (SELECT 1 FROM posts WHERE posts.user_id = users.id)"));
        assert!(params.is_empty());
    }

    #[test]
    fn subquery_not_exists_with_inner() {
        let inner = vec![(
            JoinOp::And,
            Condition::Eq("published".to_string(), SqlValue::Bool(true)),
        )];
        let (sql, params) = QueryBuilder::<()>::new("users")
            .push_condition(
                JoinOp::And,
                Condition::Subquery {
                    exists: false,
                    table: "posts".to_string(),
                    fk_expr: "posts.user_id = users.id".to_string(),
                    inner,
                },
            )
            .to_sql();
        assert!(sql.contains(
            "NOT EXISTS (SELECT 1 FROM posts WHERE posts.user_id = users.id AND published = $1)"
        ));
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], SqlValue::Bool(true));
    }

    #[test]
    fn subquery_outer_params_plus_inner_params() {
        let inner = vec![(
            JoinOp::And,
            Condition::Eq("published".to_string(), SqlValue::Bool(true)),
        )];
        let (sql, params) = QueryBuilder::<()>::new("users")
            .where_eq("active", true) // $1
            .push_condition(
                JoinOp::And,
                Condition::Subquery {
                    exists: true,
                    table: "posts".to_string(),
                    fk_expr: "posts.user_id = users.id".to_string(),
                    inner,
                },
            )
            .to_sql();
        assert!(sql.contains("active = $1"));
        assert!(sql.contains("published = $2"), "sql={sql}");
        assert_eq!(params.len(), 2);
    }

    // ── replica routing ───────────────────────────────────────────────────────

    #[test]
    fn use_replica_default_true() {
        let q = QueryBuilder::<()>::new("users");
        assert!(q.use_replica);
    }

    #[test]
    fn on_write_db_clears_replica() {
        let q = QueryBuilder::<()>::new("users").on_write_db();
        assert!(!q.use_replica);
    }

    // ── pgvector ──────────────────────────────────────────────────────────────

    #[test]
    fn nearest_to_generates_order_and_limit() {
        let embedding = vec![1.0f32, 2.0, 3.0];
        let (sql, params) = QueryBuilder::<()>::new("documents")
            .nearest_to("embedding", &embedding, 10)
            .to_sql();
        assert!(sql.contains("embedding <-> '[1,2,3]'::vector"), "sql={sql}");
        assert!(sql.contains("LIMIT 10"), "sql={sql}");
        assert!(params.is_empty());
    }

    #[test]
    fn where_cosine_distance_generates_filter() {
        let embedding = vec![0.5f32, 0.5];
        let (sql, params) = QueryBuilder::<()>::new("docs")
            .where_cosine_distance("embedding", &embedding, "<", 0.3)
            .to_sql();
        assert!(
            sql.contains("embedding <=> '[0.5,0.5]'::vector < 0.3"),
            "sql={sql}"
        );
        assert!(params.is_empty());
    }

    #[test]
    fn where_vector_distance_generates_filter() {
        let embedding = vec![1.0f32];
        let (sql, _) = QueryBuilder::<()>::new("docs")
            .where_vector_distance("vec", &embedding, "<", 1.5)
            .to_sql();
        assert!(sql.contains("vec <-> '[1]'::vector < 1.5"), "sql={sql}");
    }

    #[test]
    fn nearest_to_with_additional_filter() {
        let embedding = vec![1.0f32, 0.0];
        let (sql, params) = QueryBuilder::<()>::new("docs")
            .where_eq("active", true)
            .nearest_to("emb", &embedding, 5)
            .to_sql();
        assert!(sql.contains("WHERE active = $1"), "sql={sql}");
        assert!(sql.contains("emb <-> '[1,0]'::vector"), "sql={sql}");
        assert!(sql.contains("LIMIT 5"), "sql={sql}");
        assert_eq!(params.len(), 1);
    }

    // ── M2.5: Empty where_in guard ─────────────────────────────────────────

    #[test]
    fn where_in_empty_guard_returns_1_0() {
        let (sql, params) = QueryBuilder::<()>::new("users")
            .where_in("id", Vec::<i64>::new())
            .to_sql();
        assert!(sql.contains("1=0"), "sql={sql}");
        assert!(params.is_empty());
    }

    #[test]
    fn where_not_in_empty_guard_returns_1_1() {
        let (sql, params) = QueryBuilder::<()>::new("users")
            .where_not_in("status", Vec::<String>::new())
            .to_sql();
        assert!(sql.contains("1=1"), "sql={sql}");
        assert!(params.is_empty());
    }

    #[test]
    fn where_in_empty_guard_sqlite_dialect() {
        let (sql, params) = QueryBuilder::<()>::new("users")
            .where_in("id", Vec::<i64>::new())
            .to_sql_with_dialect(Dialect::Sqlite);
        assert!(sql.contains("1=0"), "sql={sql}");
        assert!(params.is_empty());
    }

    // ── M2.6: UPDATE/DELETE no-WHERE guard ─────────────────────────────────

    #[test]
    #[should_panic(expected = "refusing to UPDATE")]
    fn update_no_where_panics() {
        QueryBuilder::<()>::new("users").to_update_sql(&[("name", "Bob".into())]);
    }

    #[test]
    #[should_panic(expected = "refusing to DELETE")]
    fn delete_no_where_panics() {
        QueryBuilder::<()>::new("users").to_delete_sql();
    }

    // ── M2.8: FOR UPDATE / FOR SHARE ────────────────────────────────────────

    #[test]
    fn for_update_generates_correct_sql() {
        let (sql, params) = QueryBuilder::<()>::new("users")
            .where_eq("id", 1i64)
            .for_update()
            .to_sql();
        assert!(sql.contains("FOR UPDATE"), "sql={sql}");
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn for_no_key_update_generates_correct_sql() {
        let (sql, _) = QueryBuilder::<()>::new("users")
            .for_no_key_update()
            .to_sql();
        assert!(sql.contains("FOR NO KEY UPDATE"), "sql={sql}");
    }

    #[test]
    fn for_share_generates_correct_sql() {
        let (sql, _) = QueryBuilder::<()>::new("users").for_share().to_sql();
        assert!(sql.contains("FOR SHARE"), "sql={sql}");
    }

    #[test]
    fn for_key_share_generates_correct_sql() {
        let (sql, _) = QueryBuilder::<()>::new("users").for_key_share().to_sql();
        assert!(sql.contains("FOR KEY SHARE"), "sql={sql}");
    }

    #[test]
    fn for_update_with_nowait() {
        let (sql, _) = QueryBuilder::<()>::new("users")
            .where_eq("id", 1i64)
            .for_update()
            .nowait()
            .to_sql();
        assert!(sql.contains("FOR UPDATE NOWAIT"), "sql={sql}");
    }

    #[test]
    fn for_share_with_skip_locked() {
        let (sql, _) = QueryBuilder::<()>::new("users")
            .for_share()
            .skip_locked()
            .to_sql();
        assert!(sql.contains("FOR SHARE SKIP LOCKED"), "sql={sql}");
    }

    #[test]
    fn lock_clause_after_limit_offset() {
        let (sql, _) = QueryBuilder::<()>::new("users")
            .limit(10)
            .offset(20)
            .for_update()
            .to_sql();
        assert!(sql.contains("LIMIT 10"), "sql={sql}");
        assert!(sql.contains("OFFSET 20"), "sql={sql}");
        assert!(sql.contains("FOR UPDATE"), "sql={sql}");
        // FOR UPDATE must come after LIMIT/OFFSET
        let limit_pos = sql.find("LIMIT 10").unwrap();
        let for_pos = sql.find("FOR UPDATE").unwrap();
        assert!(limit_pos < for_pos, "FOR UPDATE should come after LIMIT");
    }

    // ── M2.9: UNION / INTERSECT / EXCEPT ────────────────────────────────────

    #[test]
    fn union_generates_correct_sql() {
        let lhs = QueryBuilder::<()>::new("users").where_eq("role", "admin");
        let rhs = QueryBuilder::<()>::new("users").where_eq("role", "moderator");
        let (sql, params) = lhs.union(rhs).to_sql();
        assert!(sql.contains("UNION"), "sql={sql}");
        assert!(sql.contains("WHERE role = $1"), "sql={sql}");
        assert!(sql.contains("WHERE role = $2"), "sql={sql}");
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn union_all_generates_correct_sql() {
        let lhs = QueryBuilder::<()>::new("users");
        let rhs = QueryBuilder::<()>::new("users");
        let (sql, _) = lhs.union_all(rhs).to_sql();
        assert!(sql.contains("UNION ALL"), "sql={sql}");
    }

    #[test]
    fn intersect_generates_correct_sql() {
        let lhs = QueryBuilder::<()>::new("users").where_eq("active", true);
        let rhs = QueryBuilder::<()>::new("users").where_eq("role", "admin");
        let (sql, params) = lhs.intersect(rhs).to_sql();
        assert!(sql.contains("INTERSECT"), "sql={sql}");
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn except_generates_correct_sql() {
        let lhs = QueryBuilder::<()>::new("users").where_eq("active", true);
        let rhs = QueryBuilder::<()>::new("users").where_eq("deleted", true);
        let (sql, params) = lhs.except(rhs).to_sql();
        assert!(sql.contains("EXCEPT"), "sql={sql}");
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn union_with_limit_and_order() {
        let lhs = QueryBuilder::<()>::new("users")
            .where_eq("role", "admin")
            .order_by("name");
        let rhs = QueryBuilder::<()>::new("users")
            .where_eq("role", "moderator")
            .order_by("name");
        let (sql, _) = lhs.union(rhs).limit(10).to_sql();
        assert!(sql.contains("UNION"), "sql={sql}");
        assert!(sql.contains("LIMIT 10"), "sql={sql}");
    }
}
