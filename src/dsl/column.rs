//! [`Column`] — a typed reference to a table column.
//!
//! Every field on a `#[derive(Table)]` struct generates a
//! `pub static <field>: Column<StructType, FieldType>` constant in a
//! companion module.  These column values are zero-size and used only at
//! the type/query-building level — they carry no runtime data.

use super::expr::Expr;
use crate::core::condition::SqlValue;

/// A typed column reference: `<TableMarker, ValueType>`.
///
/// Created by `#[derive(Table)]` — users do not construct these directly.
///
/// ```rust,ignore
/// // Generated for `pub id: i64` on a `#[derive(Table)] struct User`:
/// // pub static id: Column<User, i64> = Column::new("users", "id");
///
/// // Then in queries:
/// users::id.eq(42_i64)   // → Expr::Eq(Column, SqlValue)
/// users::name.like("%Al%") // → Expr::Like(Column, SqlValue)
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Column<T, V> {
    pub(crate) table: &'static str,
    pub(crate) name: &'static str,
    _table: std::marker::PhantomData<T>,
    _value: std::marker::PhantomData<V>,
}

impl<T, V> Column<T, V> {
    /// Construct a column reference.  Called by `#[derive(Table)]` generated code.
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

    /// `column IN (v1, v2, …)`
    pub fn in_(self, vals: impl IntoIterator<Item = impl Into<SqlValue>>) -> Expr {
        Expr::In(self.qualified(), vals.into_iter().map(Into::into).collect())
    }

    /// `column NOT IN (v1, v2, …)`
    pub fn not_in(self, vals: impl IntoIterator<Item = impl Into<SqlValue>>) -> Expr {
        Expr::NotIn(self.qualified(), vals.into_iter().map(Into::into).collect())
    }
}

impl<T, V> Column<T, V> {
    /// `column IS NULL`
    pub fn is_null(self) -> Expr {
        Expr::IsNull(self.qualified())
    }

    /// `column IS NOT NULL`
    pub fn is_not_null(self) -> Expr {
        Expr::IsNotNull(self.qualified())
    }

    /// `column ASC` (for `order_by`)
    pub fn asc(self) -> OrderExpr {
        OrderExpr {
            col: self.qualified(),
            dir: OrderDir::Asc,
        }
    }

    /// `column DESC` (for `order_by`)
    pub fn desc(self) -> OrderExpr {
        OrderExpr {
            col: self.qualified(),
            dir: OrderDir::Desc,
        }
    }
}

// ── OrderExpr ─────────────────────────────────────────────────────────────────

/// A column ordering expression produced by `.asc()` / `.desc()`.
#[derive(Debug, Clone)]
pub struct OrderExpr {
    pub(crate) col: String,
    pub(crate) dir: OrderDir,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderDir {
    /// Ascending order.
    Asc,
    /// Descending order.
    Desc,
}

impl OrderExpr {
    /// Render as `"table"."col" ASC` or `"table"."col" DESC`.
    pub fn to_sql(&self) -> String {
        let dir = match self.dir {
            OrderDir::Asc => "ASC",
            OrderDir::Desc => "DESC",
        };
        format!("{} {}", self.col, dir)
    }
}
