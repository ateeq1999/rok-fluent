//! [`Column`] — typed column reference, ordering, and aggregate expressions.
//!
//! Every field on a `#[derive(Table)]` struct generates a
//! `pub const FIELD_NAME: Column<StructType, FieldType>` constant on the struct.
//! These column values are zero-size and used only at the type/query-building level.

use super::expr::Expr;
use crate::core::condition::SqlValue;

/// A typed column reference: `Column<TableStruct, ValueType>`.
///
/// Created by `#[derive(Table)]` — users do not construct these directly.
///
/// ```rust,ignore
/// // Generated for `pub id: i64` on a `#[derive(Table)] struct User`:
/// // pub const ID: Column<User, i64> = Column::new("users", "id");
///
/// // Then in queries:
/// User::ID.eq(42_i64)       // → Expr::Eq(...)
/// User::NAME.like("%Al%")   // → Expr::Like(...)
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Column<T, V> {
    pub(crate) table: &'static str,
    pub(crate) name: &'static str,
    _table: std::marker::PhantomData<T>,
    _value: std::marker::PhantomData<V>,
}

impl<T, V> Column<T, V> {
    /// Construct a column reference. Called by `#[derive(Table)]` generated code.
    pub const fn new(table: &'static str, name: &'static str) -> Self {
        Self {
            table,
            name,
            _table: std::marker::PhantomData,
            _value: std::marker::PhantomData,
        }
    }

    /// The SQL-qualified form `"table"."column"`.
    pub fn qualified(&self) -> String {
        format!("\"{}\".\"{}\"", self.table, self.name)
    }

    /// Bare column name.
    pub fn name(&self) -> &'static str {
        self.name
    }
}

// ── Comparison operators ──────────────────────────────────────────────────────

impl<T, V: Into<SqlValue>> Column<T, V> {
    /// `column = value`
    pub fn eq(self, val: impl Into<SqlValue>) -> Expr {
        Expr::Eq(self.qualified(), val.into())
    }

    /// `column != value`
    pub fn ne(self, val: impl Into<SqlValue>) -> Expr {
        Expr::Ne(self.qualified(), val.into())
    }

    /// `column > value`
    pub fn gt(self, val: impl Into<SqlValue>) -> Expr {
        Expr::Gt(self.qualified(), val.into())
    }

    /// `column >= value`
    pub fn gte(self, val: impl Into<SqlValue>) -> Expr {
        Expr::Gte(self.qualified(), val.into())
    }

    /// `column < value`
    pub fn lt(self, val: impl Into<SqlValue>) -> Expr {
        Expr::Lt(self.qualified(), val.into())
    }

    /// `column <= value`
    pub fn lte(self, val: impl Into<SqlValue>) -> Expr {
        Expr::Lte(self.qualified(), val.into())
    }

    /// `column LIKE pattern`
    pub fn like(self, pattern: impl Into<SqlValue>) -> Expr {
        Expr::Like(self.qualified(), pattern.into())
    }

    /// `column NOT LIKE pattern`
    pub fn not_like(self, pattern: impl Into<SqlValue>) -> Expr {
        Expr::NotLike(self.qualified(), pattern.into())
    }

    /// `column ILIKE pattern` — PostgreSQL case-insensitive LIKE.
    pub fn ilike(self, pattern: impl Into<SqlValue>) -> Expr {
        Expr::ILike(self.qualified(), pattern.into())
    }

    /// `column IN (v1, v2, …)`
    pub fn in_(self, vals: impl IntoIterator<Item = impl Into<SqlValue>>) -> Expr {
        Expr::In(self.qualified(), vals.into_iter().map(Into::into).collect())
    }

    /// `column NOT IN (v1, v2, …)`
    pub fn not_in(self, vals: impl IntoIterator<Item = impl Into<SqlValue>>) -> Expr {
        Expr::NotIn(self.qualified(), vals.into_iter().map(Into::into).collect())
    }

    /// `column BETWEEN lo AND hi`
    pub fn between(self, lo: impl Into<SqlValue>, hi: impl Into<SqlValue>) -> Expr {
        Expr::Between(self.qualified(), lo.into(), hi.into())
    }

    /// `column NOT BETWEEN lo AND hi`
    pub fn not_between(self, lo: impl Into<SqlValue>, hi: impl Into<SqlValue>) -> Expr {
        Expr::NotBetween(self.qualified(), lo.into(), hi.into())
    }
}

// ── Null checks and ordering (no value bound required) ───────────────────────

impl<T, V> Column<T, V> {
    /// `column IS NULL`
    pub fn is_null(self) -> Expr {
        Expr::IsNull(self.qualified())
    }

    /// `column IS NOT NULL`
    pub fn is_not_null(self) -> Expr {
        Expr::IsNotNull(self.qualified())
    }

    /// `column ASC` — pass to `.order_by()`.
    pub fn asc(self) -> OrderExpr {
        OrderExpr {
            col: self.qualified(),
            dir: OrderDir::Asc,
            nulls: NullsOrder::Default,
        }
    }

    /// `column DESC` — pass to `.order_by()`.
    pub fn desc(self) -> OrderExpr {
        OrderExpr {
            col: self.qualified(),
            dir: OrderDir::Desc,
            nulls: NullsOrder::Default,
        }
    }

    /// `column ASC NULLS LAST`
    pub fn asc_nulls_last(self) -> OrderExpr {
        OrderExpr {
            col: self.qualified(),
            dir: OrderDir::Asc,
            nulls: NullsOrder::Last,
        }
    }

    /// `column ASC NULLS FIRST`
    pub fn asc_nulls_first(self) -> OrderExpr {
        OrderExpr {
            col: self.qualified(),
            dir: OrderDir::Asc,
            nulls: NullsOrder::First,
        }
    }

    /// `column DESC NULLS LAST`
    pub fn desc_nulls_last(self) -> OrderExpr {
        OrderExpr {
            col: self.qualified(),
            dir: OrderDir::Desc,
            nulls: NullsOrder::Last,
        }
    }

    /// `column DESC NULLS FIRST`
    pub fn desc_nulls_first(self) -> OrderExpr {
        OrderExpr {
            col: self.qualified(),
            dir: OrderDir::Desc,
            nulls: NullsOrder::First,
        }
    }

    /// Column-to-column equality for JOIN ON clauses: `self = other`.
    ///
    /// ```rust,ignore
    /// .inner_join(Post::table(), Post::USER_ID.references(User::ID))
    /// ```
    pub fn references<T2, V2>(self, other: Column<T2, V2>) -> Expr {
        Expr::ColEq(self.qualified(), other.qualified())
    }

    /// Column-to-column equality — alias for [`references`](Self::references)
    /// intended for use in `WHERE` expressions.
    pub fn eq_col<T2, V2>(self, other: Column<T2, V2>) -> Expr {
        Expr::ColEq(self.qualified(), other.qualified())
    }
}

// ── Aggregate expressions (Phase 24) ─────────────────────────────────────────

impl<T, V> Column<T, V> {
    /// `COUNT(column)` — returns an [`AggExpr`] for use in `.columns()` or `.having()`.
    pub fn count(self) -> AggExpr {
        AggExpr::new(format!("COUNT({})", self.qualified()))
    }

    /// `COUNT(DISTINCT column)`
    pub fn count_distinct(self) -> AggExpr {
        AggExpr::new(format!("COUNT(DISTINCT {})", self.qualified()))
    }

    /// `SUM(column)`
    pub fn sum(self) -> AggExpr {
        AggExpr::new(format!("SUM({})", self.qualified()))
    }

    /// `AVG(column)`
    pub fn avg(self) -> AggExpr {
        AggExpr::new(format!("AVG({})", self.qualified()))
    }

    /// `MIN(column)`
    pub fn min(self) -> AggExpr {
        AggExpr::new(format!("MIN({})", self.qualified()))
    }

    /// `MAX(column)`
    pub fn max(self) -> AggExpr {
        AggExpr::new(format!("MAX({})", self.qualified()))
    }
}

// ── AggExpr ───────────────────────────────────────────────────────────────────

/// An aggregate function expression — used in `HAVING` clauses and in projection.
///
/// Obtained from column aggregate methods like `.count()`, `.sum()`, `.avg()`, etc.
///
/// ```rust,ignore
/// // HAVING COUNT("orders"."id") > 5
/// .having(Order::ID.count().gt(5_i64))
/// ```
#[derive(Debug, Clone)]
pub struct AggExpr {
    /// Pre-rendered aggregate SQL, e.g. `COUNT("users"."id")`.
    pub(crate) sql: String,
    /// Optional alias for use in SELECT projections.
    alias: Option<String>,
}

impl AggExpr {
    pub(crate) fn new(sql: String) -> Self {
        Self { sql, alias: None }
    }

    /// Assign an alias: `COUNT("users"."id") AS total`.
    #[must_use]
    pub fn alias(mut self, name: impl Into<String>) -> Self {
        self.alias = Some(name.into());
        self
    }

    /// Render the aggregate expression for use in SELECT projection.
    pub fn to_projection_sql(&self) -> String {
        match &self.alias {
            Some(a) => format!("{} AS \"{}\"", self.sql, a),
            None => self.sql.clone(),
        }
    }

    /// `AGG > value` — produces a HAVING predicate.
    pub fn gt(self, val: impl Into<SqlValue>) -> Expr {
        Expr::AggCmp(self.sql, ">", val.into())
    }

    /// `AGG >= value`
    pub fn gte(self, val: impl Into<SqlValue>) -> Expr {
        Expr::AggCmp(self.sql, ">=", val.into())
    }

    /// `AGG < value`
    pub fn lt(self, val: impl Into<SqlValue>) -> Expr {
        Expr::AggCmp(self.sql, "<", val.into())
    }

    /// `AGG <= value`
    pub fn lte(self, val: impl Into<SqlValue>) -> Expr {
        Expr::AggCmp(self.sql, "<=", val.into())
    }

    /// `AGG = value`
    pub fn eq(self, val: impl Into<SqlValue>) -> Expr {
        Expr::AggCmp(self.sql, "=", val.into())
    }

    /// `AGG != value`
    pub fn ne(self, val: impl Into<SqlValue>) -> Expr {
        Expr::AggCmp(self.sql, "!=", val.into())
    }
}

// ── OrderExpr ─────────────────────────────────────────────────────────────────

/// A column ordering expression produced by `.asc()` / `.desc()` etc.
#[derive(Debug, Clone)]
pub struct OrderExpr {
    pub(crate) col: String,
    pub(crate) dir: OrderDir,
    pub(crate) nulls: NullsOrder,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderDir {
    /// Ascending order.
    Asc,
    /// Descending order.
    Desc,
}

/// NULL ordering modifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullsOrder {
    /// Database default — no explicit NULLS FIRST / NULLS LAST.
    Default,
    /// NULLS FIRST
    First,
    /// NULLS LAST
    Last,
}

impl OrderExpr {
    /// Render as `"table"."col" ASC [NULLS FIRST|LAST]`.
    pub fn to_sql(&self) -> String {
        let dir = match self.dir {
            OrderDir::Asc => "ASC",
            OrderDir::Desc => "DESC",
        };
        let nulls = match self.nulls {
            NullsOrder::Default => String::new(),
            NullsOrder::First => " NULLS FIRST".to_string(),
            NullsOrder::Last => " NULLS LAST".to_string(),
        };
        format!("{} {}{}", self.col, dir, nulls)
    }
}
