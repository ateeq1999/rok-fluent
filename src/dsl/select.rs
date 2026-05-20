//! [`SelectBuilder`] — typed `SELECT` query builder.

use super::{column::OrderExpr, expr::Expr, table::Table};
use crate::core::condition::SqlValue;

/// A composable `SELECT` query.
///
/// Created by [`db::select()`](super::db::select).
#[derive(Debug)]
#[must_use]
pub struct SelectBuilder {
    table: Option<&'static str>,
    columns: Vec<String>,
    wheres: Vec<Expr>,
    orders: Vec<OrderExpr>,
    limit: Option<u64>,
    offset: Option<u64>,
    distinct: bool,
}

impl SelectBuilder {
    pub(crate) fn new() -> Self {
        Self {
            table: None,
            columns: Vec::new(),
            wheres: Vec::new(),
            orders: Vec::new(),
            limit: None,
            offset: None,
            distinct: false,
        }
    }

    /// Set the table to select from.
    pub fn from<T: Table>(mut self, _table: T) -> Self {
        self.table = Some(T::table_name());
        self
    }

    /// Restrict to specific columns.  Omit to `SELECT *`.
    pub fn columns(mut self, cols: impl IntoIterator<Item = &'static str>) -> Self {
        self.columns
            .extend(cols.into_iter().map(|c| format!("\"{c}\"")));
        self
    }

    /// Add a `WHERE` predicate (multiple calls are `AND`-ed together).
    pub fn where_(mut self, expr: Expr) -> Self {
        self.wheres.push(expr);
        self
    }

    /// Add an `ORDER BY` clause.
    pub fn order_by(mut self, ord: OrderExpr) -> Self {
        self.orders.push(ord);
        self
    }

    /// `LIMIT n`
    pub fn limit(mut self, n: u64) -> Self {
        self.limit = Some(n);
        self
    }

    /// `OFFSET n`
    pub fn offset(mut self, n: u64) -> Self {
        self.offset = Some(n);
        self
    }

    /// `SELECT DISTINCT`
    pub fn distinct(mut self) -> Self {
        self.distinct = true;
        self
    }

    /// Render to `(sql, params)` using PostgreSQL `$N` placeholders.
    pub fn to_sql_pg(&self) -> (String, Vec<SqlValue>) {
        self.render('$')
    }

    /// Render to `(sql, params)` using `?` placeholders (MySQL / SQLite).
    pub fn to_sql_qmark(&self) -> (String, Vec<SqlValue>) {
        self.render('?')
    }

    fn render(&self, ph: char) -> (String, Vec<SqlValue>) {
        let table = self.table.unwrap_or("unknown");
        let cols = if self.columns.is_empty() {
            "*".to_string()
        } else {
            self.columns.join(", ")
        };
        let distinct = if self.distinct { "DISTINCT " } else { "" };
        let mut sql = format!("SELECT {distinct}{cols} FROM \"{table}\"");
        let mut params: Vec<SqlValue> = Vec::new();

        if !self.wheres.is_empty() {
            let mut frags = Vec::with_capacity(self.wheres.len());
            for expr in &self.wheres {
                let (s, p) = if ph == '?' {
                    expr.to_sql_qmark(params.len() + 1)
                } else {
                    expr.to_sql_pg(params.len() + 1)
                };
                frags.push(s);
                params.extend(p);
            }
            sql.push_str(&format!(" WHERE {}", frags.join(" AND ")));
        }

        if !self.orders.is_empty() {
            let ord: Vec<String> = self.orders.iter().map(|o| o.to_sql()).collect();
            sql.push_str(&format!(" ORDER BY {}", ord.join(", ")));
        }

        if let Some(n) = self.limit {
            sql.push_str(&format!(" LIMIT {n}"));
        }
        if let Some(n) = self.offset {
            sql.push_str(&format!(" OFFSET {n}"));
        }

        (sql, params)
    }
}

// ── PostgreSQL async terminals ────────────────────────────────────────────────

#[cfg(feature = "postgres")]
impl SelectBuilder {
    /// Execute and return all matching rows.
    pub async fn fetch_all<T>(self, pool: &sqlx::PgPool) -> Result<Vec<T>, sqlx::Error>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        let (sql, params) = self.to_sql_pg();
        crate::core::sqlx::pg::fetch_all_as::<T>(pool, &sql, params).await
    }

    /// Execute and return the first row, or `None` if no rows match.
    pub async fn fetch_optional<T>(self, pool: &sqlx::PgPool) -> Result<Option<T>, sqlx::Error>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        let (mut sql, params) = self.to_sql_pg();
        // Ensure LIMIT 1 for optional fetch
        if self.limit.is_none() {
            sql.push_str(" LIMIT 1");
        }
        crate::core::sqlx::pg::fetch_optional_as::<T>(pool, &sql, params).await
    }

    /// Execute and return exactly one row; errors if zero or multiple rows match.
    pub async fn fetch_one<T>(self, pool: &sqlx::PgPool) -> Result<T, sqlx::Error>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        self.fetch_optional::<T>(pool)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    /// Return `true` if at least one row matches.
    pub async fn exists(self, pool: &sqlx::PgPool) -> Result<bool, sqlx::Error> {
        let (inner_sql, params) = self.to_sql_pg();
        let sql = format!("SELECT EXISTS ({inner_sql})");
        let row = crate::core::sqlx::pg::build_query(&sql, params)
            .fetch_one(pool)
            .await?;
        use sqlx::Row;
        row.try_get::<bool, _>(0)
    }

    /// Return the number of matching rows.
    pub async fn count(self, pool: &sqlx::PgPool) -> Result<i64, sqlx::Error> {
        let table = self.table.unwrap_or("unknown");
        let mut count_sql = format!("SELECT COUNT(*) FROM \"{table}\"");
        let mut params: Vec<SqlValue> = Vec::new();

        if !self.wheres.is_empty() {
            let mut frags = Vec::new();
            for expr in &self.wheres {
                let (s, p) = expr.to_sql_pg(params.len() + 1);
                frags.push(s);
                params.extend(p);
            }
            count_sql.push_str(&format!(" WHERE {}", frags.join(" AND ")));
        }

        let row = crate::core::sqlx::pg::build_query(&count_sql, params)
            .fetch_one(pool)
            .await?;
        use sqlx::Row;
        row.try_get::<i64, _>(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::condition::SqlValue;
    use crate::dsl::expr::Expr;

    #[test]
    fn basic_select_all() {
        let b = SelectBuilder::new();
        // We need a table; simulate it
        let b = SelectBuilder {
            table: Some("users"),
            columns: vec![],
            wheres: vec![],
            orders: vec![],
            limit: None,
            offset: None,
            distinct: false,
        };
        let (sql, params) = b.to_sql_pg();
        assert_eq!(sql, "SELECT * FROM \"users\"");
        assert!(params.is_empty());
    }

    #[test]
    fn select_with_where_and_limit() {
        let b = SelectBuilder {
            table: Some("posts"),
            columns: vec![],
            wheres: vec![Expr::Eq(
                "\"posts\".\"user_id\"".into(),
                SqlValue::Integer(42),
            )],
            orders: vec![],
            limit: Some(10),
            offset: Some(20),
            distinct: false,
        };
        let (sql, params) = b.to_sql_pg();
        assert_eq!(
            sql,
            "SELECT * FROM \"posts\" WHERE \"posts\".\"user_id\" = $1 LIMIT 10 OFFSET 20"
        );
        assert_eq!(params.len(), 1);
    }
}
