//! [`QueryBuilder`] — fluent SQL builder.

use std::marker::PhantomData;

use super::condition::{Condition, JoinOp, OrderDir, SqlValue};

/// SQL placeholder dialect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dialect {
    #[default]
    Postgres,
    Sqlite,
}

/// A SQL JOIN clause.
#[derive(Debug, Clone)]
pub enum Join {
    Inner(String, String),
    Left(String, String),
    Right(String, String),
    Raw(String),
}

/// A Common Table Expression definition.
#[derive(Debug, Clone)]
pub struct CteDef<T> {
    pub name: String,
    pub columns: Vec<String>,
    pub subquery: QueryBuilder<T>,
    pub recursive: bool,
}

/// Row-level locking clause for SELECT statements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockClause {
    ForUpdate,
    ForNoKeyUpdate,
    ForShare,
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
    NoWait,
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

/// Set operation for UNION / INTERSECT / EXCEPT queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetOp {
    Union,
    UnionAll,
    Intersect,
    IntersectAll,
    Except,
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

/// A named window definition for `WINDOW` clause.
#[derive(Debug, Clone)]
pub struct WindowDef {
    pub name: String,
    pub partition: Vec<String>,
    pub order: Vec<(String, OrderDir)>,
}

/// A fluent builder that produces parameterized SQL statements.
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
    pub use_replica: bool,
    extra_select_exprs: Vec<String>,
    ctes: Vec<CteDef<T>>,
    windows: Vec<WindowDef>,
    lock: Option<LockClause>,
    lock_wait: Option<LockWait>,
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

    pub fn select(mut self, cols: &[&str]) -> Self {
        self.select_cols = Some(cols.iter().map(|s| s.to_string()).collect());
        self
    }

    pub fn select_raw(mut self, expr: &str) -> Self {
        self.select_raw = Some(expr.to_string());
        self
    }

    pub fn add_select_expr(mut self, expr: impl Into<String>) -> Self {
        self.extra_select_exprs.push(expr.into());
        self
    }

    pub fn distinct(mut self) -> Self {
        self.distinct = true;
        self
    }

    pub fn distinct_on(mut self, cols: &[&str]) -> Self {
        self.distinct_on_cols = cols.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn inner_join(mut self, table: &str, on: &str) -> Self {
        self.joins
            .push(Join::Inner(table.to_string(), on.to_string()));
        self
    }

    pub fn left_join(mut self, table: &str, on: &str) -> Self {
        self.joins
            .push(Join::Left(table.to_string(), on.to_string()));
        self
    }

    pub fn right_join(mut self, table: &str, on: &str) -> Self {
        self.joins
            .push(Join::Right(table.to_string(), on.to_string()));
        self
    }

    pub fn join_raw(mut self, raw: &str) -> Self {
        self.joins.push(Join::Raw(raw.to_string()));
        self
    }

    pub fn group_by(mut self, cols: &[&str]) -> Self {
        self.group_by = cols.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn having(mut self, expr: &str) -> Self {
        self.having = Some(expr.to_string());
        self
    }

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

    /// `WHERE col = ANY(ARRAY[$1, $2, …])` — single array parameter; better for prepared-statement reuse.
    pub fn where_eq_any(self, col: &str, vals: Vec<impl Into<SqlValue>>) -> Self {
        self.push(
            JoinOp::And,
            Condition::EqAny(col.into(), vals.into_iter().map(Into::into).collect()),
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

    pub fn where_ilike(self, col: &str, pattern: &str) -> Self {
        self.push(JoinOp::And, Condition::ILike(col.into(), pattern.into()))
    }

    pub fn or_where_ilike(self, col: &str, pattern: &str) -> Self {
        self.push(JoinOp::Or, Condition::ILike(col.into(), pattern.into()))
    }

    pub fn where_json(self, col: &str, key: &str, val: impl Into<SqlValue>) -> Self {
        self.push(
            JoinOp::And,
            Condition::JsonGet(col.into(), key.into(), val.into()),
        )
    }

    pub fn or_where_json(self, col: &str, key: &str, val: impl Into<SqlValue>) -> Self {
        self.push(
            JoinOp::Or,
            Condition::JsonGet(col.into(), key.into(), val.into()),
        )
    }

    pub fn where_json_contains(self, col: &str, json_val: &str) -> Self {
        self.push(
            JoinOp::And,
            Condition::Raw(format!("{col} @> '{json_val}'::jsonb")),
        )
    }

    pub fn where_column(self, col: &str, op: &str, val: impl Into<SqlValue>) -> Self {
        self.where_op(col, op, val)
    }

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

    pub fn order_by(mut self, col: &str) -> Self {
        self.order.push((col.into(), OrderDir::Asc));
        self
    }

    pub fn order_by_desc(mut self, col: &str) -> Self {
        self.order.push((col.into(), OrderDir::Desc));
        self
    }

    pub fn order_by_raw(mut self, expr: &str) -> Self {
        self.order_raw = Some(expr.to_string());
        self
    }

    pub fn order_by_many(mut self, cols: &[(&str, OrderDir)]) -> Self {
        for (col, dir) in cols {
            self.order.push((col.to_string(), *dir));
        }
        self
    }

    pub fn reorder(mut self, col: &str) -> Self {
        self.order.clear();
        self.order_raw = None;
        self.order.push((col.into(), OrderDir::Asc));
        self
    }

    pub fn reorder_desc(mut self, col: &str) -> Self {
        self.order.clear();
        self.order_raw = None;
        self.order.push((col.into(), OrderDir::Desc));
        self
    }

    pub fn limit(mut self, n: usize) -> Self {
        self.limit_val = Some(n);
        self
    }

    pub fn offset(mut self, n: usize) -> Self {
        self.offset_val = Some(n);
        self
    }

    pub fn to_sql(&self) -> (String, Vec<SqlValue>) {
        self.to_sql_with_dialect(Dialect::Postgres)
    }

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

    pub fn to_count_sql(&self) -> (String, Vec<SqlValue>) {
        self.to_count_sql_with_dialect(Dialect::Postgres)
    }

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

    pub fn to_delete_sql(&self) -> (String, Vec<SqlValue>) {
        self.to_delete_sql_with_dialect(Dialect::Postgres)
    }

    pub fn to_delete_sql_with_dialect(&self, dialect: Dialect) -> (String, Vec<SqlValue>) {
        assert!(
            !self.conditions.is_empty(),
            "refusing to DELETE from {} without a WHERE clause — this would delete all rows in the table.",
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

    pub fn to_update_sql(&self, data: &[(&str, SqlValue)]) -> (String, Vec<SqlValue>) {
        self.to_update_sql_with_dialect(Dialect::Postgres, data)
    }

    pub fn to_update_sql_with_dialect(
        &self,
        dialect: Dialect,
        data: &[(&str, SqlValue)],
    ) -> (String, Vec<SqlValue>) {
        assert!(
            !self.conditions.is_empty(),
            "refusing to UPDATE {} without a WHERE clause — this would update all rows in the table.",
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

    pub fn insert_sql(table: &str, data: &[(&str, SqlValue)]) -> (String, Vec<SqlValue>) {
        Self::insert_sql_with_dialect(Dialect::Postgres, table, data)
    }

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
            sql.push_str(&build_where_from(conditions, &mut params));
        }
        (sql, params)
    }

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

    pub fn when_some<V, F>(self, opt: Option<V>, f: F) -> Self
    where
        F: FnOnce(Self, V) -> Self,
    {
        match opt {
            Some(v) => f(self, v),
            None => self,
        }
    }

    pub fn push_condition(self, op: JoinOp, cond: Condition) -> Self {
        self.push(op, cond)
    }

    pub fn with_cte(mut self, name: &str, columns: &[&str], subquery: QueryBuilder<T>) -> Self {
        self.ctes.push(CteDef {
            name: name.to_string(),
            columns: columns.iter().map(|s| s.to_string()).collect(),
            subquery,
            recursive: false,
        });
        self
    }

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

    pub fn window(mut self, name: &str, partition: &[&str], order: &[(&str, OrderDir)]) -> Self {
        self.windows.push(WindowDef {
            name: name.to_string(),
            partition: partition.iter().map(|s| s.to_string()).collect(),
            order: order.iter().map(|(c, d)| (c.to_string(), *d)).collect(),
        });
        self
    }

    pub fn select_with_window(mut self, col: &str, window_fn: &str, window_name: &str) -> Self {
        self.extra_select_exprs
            .push(format!("{window_fn}({col}) OVER {window_name}"));
        self
    }

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

    pub fn order_by_rank(mut self, col: &str, query: &str, config: Option<&str>) -> Self {
        let config_fragment = config
            .map(|c| format!("'{}', ", c.replace('\'', "''")))
            .unwrap_or_default();
        let rank_expr =
            format!("ts_rank(to_tsvector({col}), to_tsquery({config_fragment}'{query}')) DESC");
        self.order.push((rank_expr, OrderDir::Desc));
        self
    }

    pub fn on_write_db(mut self) -> Self {
        self.use_replica = false;
        self
    }

    pub fn for_update(mut self) -> Self {
        self.lock = Some(LockClause::ForUpdate);
        self
    }
    pub fn for_no_key_update(mut self) -> Self {
        self.lock = Some(LockClause::ForNoKeyUpdate);
        self
    }
    pub fn for_share(mut self) -> Self {
        self.lock = Some(LockClause::ForShare);
        self
    }
    pub fn for_key_share(mut self) -> Self {
        self.lock = Some(LockClause::ForKeyShare);
        self
    }
    pub fn nowait(mut self) -> Self {
        self.lock_wait = Some(LockWait::NoWait);
        self
    }
    pub fn skip_locked(mut self) -> Self {
        self.lock_wait = Some(LockWait::SkipLocked);
        self
    }

    pub fn union(self, rhs: QueryBuilder<T>) -> Self {
        self.set_op_impl(SetOp::Union, rhs)
    }
    pub fn union_all(self, rhs: QueryBuilder<T>) -> Self {
        self.set_op_impl(SetOp::UnionAll, rhs)
    }
    pub fn intersect(self, rhs: QueryBuilder<T>) -> Self {
        self.set_op_impl(SetOp::Intersect, rhs)
    }
    pub fn intersect_all(self, rhs: QueryBuilder<T>) -> Self {
        self.set_op_impl(SetOp::IntersectAll, rhs)
    }
    pub fn except(self, rhs: QueryBuilder<T>) -> Self {
        self.set_op_impl(SetOp::Except, rhs)
    }
    pub fn except_all(self, rhs: QueryBuilder<T>) -> Self {
        self.set_op_impl(SetOp::ExceptAll, rhs)
    }

    fn set_op_impl(mut self, op: SetOp, rhs: QueryBuilder<T>) -> Self {
        self.set_op = Some((op, Box::new(rhs)));
        self
    }

    pub fn nearest_to(self, col: &str, embedding: &[f32], k: usize) -> Self {
        let vec_lit = format_vector(embedding);
        self.order_by_raw(&format!("{col} <-> '{vec_lit}'::vector"))
            .limit(k)
    }

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

    pub fn conditions(&self) -> &[(JoinOp, Condition)] {
        &self.conditions
    }

    /// Return the table name (used by the AR ↔ DSL bridge).
    #[cfg(all(feature = "active", feature = "query"))]
    pub fn table_name_str(&self) -> &str {
        &self.table
    }

    /// Return the ORDER BY clauses (used by the AR ↔ DSL bridge).
    #[cfg(all(feature = "active", feature = "query"))]
    pub fn order_clauses(&self) -> &[(String, OrderDir)] {
        &self.order
    }

    /// Return the LIMIT value (used by the AR ↔ DSL bridge).
    #[cfg(all(feature = "active", feature = "query"))]
    pub fn limit_value(&self) -> Option<usize> {
        self.limit_val
    }

    /// Return the OFFSET value (used by the AR ↔ DSL bridge).
    #[cfg(all(feature = "active", feature = "query"))]
    pub fn offset_value(&self) -> Option<usize> {
        self.offset_val
    }

    pub fn to_where_clause(&self) -> (String, Vec<SqlValue>) {
        let mut params = Vec::new();
        let clause = self.build_where_dialect(Dialect::Postgres, &mut params);
        (clause, params)
    }

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
                placeholders.join(", ")
            )
        } else {
            format!("INSERT INTO {table} ({}) VALUES ({}) ON CONFLICT ({conflict_target}) DO UPDATE SET {}", cols.join(", "), placeholders.join(", "), update_set.join(", "))
        };
        (sql, params)
    }
}

fn format_vector(v: &[f32]) -> String {
    let elems: Vec<String> = v.iter().map(|x| x.to_string()).collect();
    format!("[{}]", elems.join(","))
}

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

#[cfg(test)]
mod tests {
    use super::super::condition::{Condition, JoinOp, SqlValue};
    use super::*;

    #[test]
    fn simple_select() {
        let (sql, params) = QueryBuilder::<()>::new("users").to_sql();
        assert_eq!(sql, "SELECT * FROM users");
        assert!(params.is_empty());
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
            .limit(10)
            .offset(20)
            .to_sql();
        assert!(sql.contains("ORDER BY created_at DESC"));
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
    fn bulk_insert_sql_two_rows() {
        let rows: Vec<Vec<(&str, SqlValue)>> = vec![
            vec![("name", "Alice".into()), ("email", "a@a.com".into())],
            vec![("name", "Bob".into()), ("email", "b@b.com".into())],
        ];
        let (sql, params) = QueryBuilder::<()>::bulk_insert_sql("users", &rows);
        assert!(sql.contains("($1, $2), ($3, $4)"));
        assert_eq!(params.len(), 4);
    }

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
    fn subquery_exists_no_inner() {
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
    #[should_panic(expected = "refusing to UPDATE")]
    fn update_no_where_panics() {
        QueryBuilder::<()>::new("users").to_update_sql(&[("name", "Bob".into())]);
    }

    #[test]
    #[should_panic(expected = "refusing to DELETE")]
    fn delete_no_where_panics() {
        QueryBuilder::<()>::new("users").to_delete_sql();
    }

    #[test]
    fn for_update_generates_correct_sql() {
        let (sql, _) = QueryBuilder::<()>::new("users")
            .where_eq("id", 1i64)
            .for_update()
            .to_sql();
        assert!(sql.contains("FOR UPDATE"), "sql={sql}");
    }

    #[test]
    fn union_generates_correct_sql() {
        let lhs = QueryBuilder::<()>::new("users").where_eq("role", "admin");
        let rhs = QueryBuilder::<()>::new("users").where_eq("role", "moderator");
        let (sql, params) = lhs.union(rhs).to_sql();
        assert!(sql.contains("UNION"), "sql={sql}");
        assert_eq!(params.len(), 2);
    }
}
