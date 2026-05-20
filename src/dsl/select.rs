//! [`SelectBuilder`] — typed `SELECT` query builder.

use super::{
    column::{AggExpr, Column, OrderExpr},
    expr::Expr,
    table::Table,
};
use crate::core::condition::SqlValue;

// ── Join helpers ──────────────────────────────────────────────────────────────

/// JOIN type for [`SelectBuilder`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinKind {
    /// `INNER JOIN`
    Inner,
    /// `LEFT JOIN`
    Left,
    /// `RIGHT JOIN`
    Right,
    /// `CROSS JOIN` (no ON clause)
    Cross,
}

/// A single JOIN clause.
#[derive(Debug, Clone)]
pub struct Join {
    pub(crate) kind: JoinKind,
    pub(crate) table: String,
    /// `None` for CROSS JOIN.
    pub(crate) on: Option<Expr>,
}

// ── SelectBuilder ─────────────────────────────────────────────────────────────

/// A composable `SELECT` query.
///
/// Created by [`db::select()`](super::db::select).
///
/// ```rust,ignore
/// let users: Vec<User> = db::select()
///     .from(User::table())
///     .where_(User::ACTIVE.eq(true))
///     .order_by(User::NAME.asc())
///     .limit(25)
///     .fetch_all::<User>(&pool)
///     .await?;
/// ```
#[derive(Debug)]
#[must_use]
pub struct SelectBuilder {
    table: Option<&'static str>,
    columns: Vec<String>,
    joins: Vec<Join>,
    wheres: Vec<Expr>,
    or_wheres: Vec<Expr>,
    group_by: Vec<String>,
    havings: Vec<Expr>,
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
            joins: Vec::new(),
            wheres: Vec::new(),
            or_wheres: Vec::new(),
            group_by: Vec::new(),
            havings: Vec::new(),
            orders: Vec::new(),
            limit: None,
            offset: None,
            distinct: false,
        }
    }

    // ── Source ────────────────────────────────────────────────────────────────

    /// Set the table to select from.
    pub fn from<T: Table>(mut self, _table: T) -> Self {
        self.table = Some(T::table_name());
        self
    }

    // ── Projection ────────────────────────────────────────────────────────────

    /// Restrict to specific columns by name (default: `SELECT *`).
    pub fn columns(mut self, cols: impl IntoIterator<Item = &'static str>) -> Self {
        self.columns
            .extend(cols.into_iter().map(|c| format!("\"{c}\"")));
        self
    }

    /// Restrict to specific typed columns. Accepts `Column<T,V>` constants.
    pub fn select<T, V>(mut self, cols: impl IntoIterator<Item = Column<T, V>>) -> Self {
        self.columns
            .extend(cols.into_iter().map(|c| format!("\"{}\"", c.name)));
        self
    }

    /// Add an aggregate expression to the projection (e.g. `SUM(col) AS total`).
    pub fn agg_col(mut self, expr: AggExpr) -> Self {
        self.columns.push(expr.to_projection_sql());
        self
    }

    /// `SELECT DISTINCT`
    pub fn distinct(mut self) -> Self {
        self.distinct = true;
        self
    }

    // ── Filtering ─────────────────────────────────────────────────────────────

    /// Add a `WHERE` predicate (multiple calls are `AND`-ed together).
    pub fn where_(mut self, expr: Expr) -> Self {
        self.wheres.push(expr);
        self
    }

    /// Add an `OR WHERE` predicate.
    pub fn or_where(mut self, expr: Expr) -> Self {
        self.or_wheres.push(expr);
        self
    }

    // ── Joins ─────────────────────────────────────────────────────────────────

    /// `INNER JOIN table ON expr`
    pub fn inner_join(mut self, table: impl Table, on: Expr) -> Self {
        self.joins.push(Join {
            kind: JoinKind::Inner,
            table: table.name().to_owned(),
            on: Some(on),
        });
        self
    }

    /// `LEFT JOIN table ON expr`
    pub fn left_join(mut self, table: impl Table, on: Expr) -> Self {
        self.joins.push(Join {
            kind: JoinKind::Left,
            table: table.name().to_owned(),
            on: Some(on),
        });
        self
    }

    /// `RIGHT JOIN table ON expr`
    pub fn right_join(mut self, table: impl Table, on: Expr) -> Self {
        self.joins.push(Join {
            kind: JoinKind::Right,
            table: table.name().to_owned(),
            on: Some(on),
        });
        self
    }

    /// `CROSS JOIN table` (no ON clause)
    pub fn cross_join(mut self, table: impl Table) -> Self {
        self.joins.push(Join {
            kind: JoinKind::Cross,
            table: table.name().to_owned(),
            on: None,
        });
        self
    }

    // ── Grouping ──────────────────────────────────────────────────────────────

    /// Add `GROUP BY` columns.
    pub fn group_by<T, V>(mut self, cols: impl IntoIterator<Item = Column<T, V>>) -> Self {
        self.group_by
            .extend(cols.into_iter().map(|c| c.qualified()));
        self
    }

    /// Add a `HAVING` predicate (usually an [`AggExpr`] comparison).
    pub fn having(mut self, expr: Expr) -> Self {
        self.havings.push(expr);
        self
    }

    // ── Sorting ───────────────────────────────────────────────────────────────

    /// Add an `ORDER BY` clause.
    pub fn order_by(mut self, ord: OrderExpr) -> Self {
        self.orders.push(ord);
        self
    }

    // ── Pagination ────────────────────────────────────────────────────────────

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

    // ── SQL rendering ─────────────────────────────────────────────────────────

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

        // JOINs
        for join in &self.joins {
            let kind = match join.kind {
                JoinKind::Inner => "INNER JOIN",
                JoinKind::Left => "LEFT JOIN",
                JoinKind::Right => "RIGHT JOIN",
                JoinKind::Cross => "CROSS JOIN",
            };
            sql.push_str(&format!(" {kind} \"{}\"", join.table));
            if let Some(on_expr) = &join.on {
                let (on_sql, on_params) = if ph == '?' {
                    on_expr.to_sql_qmark(params.len() + 1)
                } else {
                    on_expr.to_sql_pg(params.len() + 1)
                };
                sql.push_str(&format!(" ON {on_sql}"));
                params.extend(on_params);
            }
        }

        // WHERE
        if !self.wheres.is_empty() || !self.or_wheres.is_empty() {
            let mut all_frags: Vec<String> = Vec::new();
            for expr in &self.wheres {
                let (s, p) = if ph == '?' {
                    expr.to_sql_qmark(params.len() + 1)
                } else {
                    expr.to_sql_pg(params.len() + 1)
                };
                all_frags.push(s);
                params.extend(p);
            }
            let and_part = all_frags.join(" AND ");

            if self.or_wheres.is_empty() {
                sql.push_str(&format!(" WHERE {and_part}"));
            } else {
                let mut or_frags: Vec<String> = Vec::new();
                for expr in &self.or_wheres {
                    let (s, p) = if ph == '?' {
                        expr.to_sql_qmark(params.len() + 1)
                    } else {
                        expr.to_sql_pg(params.len() + 1)
                    };
                    or_frags.push(s);
                    params.extend(p);
                }
                let or_part = or_frags.join(" OR ");
                if and_part.is_empty() {
                    sql.push_str(&format!(" WHERE {or_part}"));
                } else {
                    sql.push_str(&format!(" WHERE ({and_part}) OR ({or_part})"));
                }
            }
        }

        // GROUP BY
        if !self.group_by.is_empty() {
            sql.push_str(&format!(" GROUP BY {}", self.group_by.join(", ")));
        }

        // HAVING
        if !self.havings.is_empty() {
            let mut frags = Vec::new();
            for expr in &self.havings {
                let (s, p) = if ph == '?' {
                    expr.to_sql_qmark(params.len() + 1)
                } else {
                    expr.to_sql_pg(params.len() + 1)
                };
                frags.push(s);
                params.extend(p);
            }
            sql.push_str(&format!(" HAVING {}", frags.join(" AND ")));
        }

        // ORDER BY
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

    /// Build a COUNT(*) SQL string from the current WHERE clauses.
    fn count_sql(&self, ph: char) -> (String, Vec<SqlValue>) {
        let table = self.table.unwrap_or("unknown");
        let mut sql = format!("SELECT COUNT(*) FROM \"{table}\"");
        let mut params: Vec<SqlValue> = Vec::new();

        for join in &self.joins {
            let kind = match join.kind {
                JoinKind::Inner => "INNER JOIN",
                JoinKind::Left => "LEFT JOIN",
                JoinKind::Right => "RIGHT JOIN",
                JoinKind::Cross => "CROSS JOIN",
            };
            sql.push_str(&format!(" {kind} \"{}\"", join.table));
            if let Some(on_expr) = &join.on {
                let (on_sql, on_params) = if ph == '?' {
                    on_expr.to_sql_qmark(params.len() + 1)
                } else {
                    on_expr.to_sql_pg(params.len() + 1)
                };
                sql.push_str(&format!(" ON {on_sql}"));
                params.extend(on_params);
            }
        }

        if !self.wheres.is_empty() {
            let mut frags = Vec::new();
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
        let had_limit = self.limit.is_some();
        let (mut sql, params) = self.to_sql_pg();
        if !had_limit {
            sql.push_str(" LIMIT 1");
        }
        crate::core::sqlx::pg::fetch_optional_as::<T>(pool, &sql, params).await
    }

    /// Execute and return exactly one row; errors if no rows match.
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

    /// Return the number of matching rows (`SELECT COUNT(*)`).
    pub async fn count(self, pool: &sqlx::PgPool) -> Result<i64, sqlx::Error> {
        let (count_sql, params) = self.count_sql('$');
        let row = crate::core::sqlx::pg::build_query(&count_sql, params)
            .fetch_one(pool)
            .await?;
        use sqlx::Row;
        row.try_get::<i64, _>(0)
    }

    /// Offset pagination — runs a `COUNT(*)` query and a data query.
    ///
    /// Returns a [`Page<T>`](crate::orm::pagination::Page) with full metadata.
    pub async fn paginate<T>(
        self,
        page: u32,
        per_page: u32,
        pool: &sqlx::PgPool,
    ) -> Result<crate::orm::pagination::Page<T>, sqlx::Error>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin + serde::Serialize,
    {
        let page = page.max(1);
        let per_page = per_page.max(1);

        // COUNT query
        let (count_sql, count_params) = self.count_sql('$');
        let count_row = crate::core::sqlx::pg::build_query(&count_sql, count_params)
            .fetch_one(pool)
            .await?;
        use sqlx::Row;
        let total: i64 = count_row.try_get::<i64, _>(0)?;

        // Data query
        let offset = (page - 1) as u64 * per_page as u64;
        let (mut data_sql, data_params) = self.to_sql_pg();
        data_sql.push_str(&format!(" LIMIT {per_page} OFFSET {offset}"));
        let data = crate::core::sqlx::pg::fetch_all_as::<T>(pool, &data_sql, data_params).await?;

        Ok(crate::orm::pagination::Page::new(
            data, total, per_page, page,
        ))
    }

    /// Simple pagination — no `COUNT(*)` query; detects next page by fetching `per_page + 1`.
    pub async fn simple_paginate<T>(
        self,
        page: u32,
        per_page: u32,
        pool: &sqlx::PgPool,
    ) -> Result<crate::orm::pagination::SimplePage<T>, sqlx::Error>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin + serde::Serialize,
    {
        let page = page.max(1);
        let per_page = per_page.max(1);
        let offset = (page - 1) as u64 * per_page as u64;
        let (mut sql, params) = self.to_sql_pg();
        // Fetch one extra to detect whether a next page exists.
        sql.push_str(&format!(" LIMIT {} OFFSET {offset}", per_page + 1));
        let data = crate::core::sqlx::pg::fetch_all_as::<T>(pool, &sql, params).await?;
        Ok(crate::orm::pagination::SimplePage::new(
            data, per_page, page,
        ))
    }

    /// Cursor pagination — stable, efficient for infinite scroll.
    ///
    /// `cursor` is the opaque string returned by the previous page's
    /// [`CursorPage::next_cursor`](crate::orm::pagination::CursorPage::next_cursor).
    /// Pass `None` for the first page.
    pub async fn cursor_paginate<T, ST, SV>(
        mut self,
        cursor_col: Column<ST, SV>,
        cursor: Option<&str>,
        per_page: u32,
        pool: &sqlx::PgPool,
    ) -> Result<crate::orm::pagination::CursorPage<T>, sqlx::Error>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin + serde::Serialize,
        SV: Into<SqlValue>,
    {
        use base64::Engine;
        use sqlx::Row;

        let per_page = per_page.max(1);
        let prev_cursor = cursor.map(|c| c.to_owned());

        // Decode cursor → append WHERE cursor_col > last_value
        if let Some(c) = cursor {
            if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(c) {
                if let Ok(val_str) = std::str::from_utf8(&decoded) {
                    let sql_val = if let Ok(n) = val_str.parse::<i64>() {
                        SqlValue::Integer(n)
                    } else {
                        SqlValue::Text(val_str.to_owned())
                    };
                    self = self.where_(Expr::Gt(cursor_col.qualified(), sql_val));
                }
            }
        }

        // Fetch per_page + 1 raw rows so we can extract the cursor value before
        // deserializing into T (generic T has no typed cursor field accessor).
        let col_name = cursor_col.name();
        let (mut sql, params) = self.to_sql_pg();
        sql.push_str(&format!(
            " ORDER BY {} ASC LIMIT {}",
            cursor_col.qualified(),
            per_page + 1
        ));
        let raw_rows = crate::core::sqlx::pg::build_query(&sql, params)
            .fetch_all(pool)
            .await?;

        let has_more = raw_rows.len() > per_page as usize;

        // Extract the cursor value from the last *kept* row.
        let next_cursor = if has_more {
            raw_rows.get(per_page as usize - 1).and_then(|row| {
                if let Ok(v) = row.try_get::<i64, _>(col_name) {
                    Some(base64::engine::general_purpose::STANDARD.encode(v.to_string()))
                } else if let Ok(v) = row.try_get::<String, _>(col_name) {
                    Some(base64::engine::general_purpose::STANDARD.encode(&v))
                } else {
                    None
                }
            })
        } else {
            None
        };

        // Deserialize kept rows into T.
        let data: Vec<T> = raw_rows
            .iter()
            .take(per_page as usize)
            .map(T::from_row)
            .collect::<Result<Vec<T>, _>>()?;

        Ok(crate::orm::pagination::CursorPage::new(
            data,
            per_page,
            next_cursor,
            prev_cursor,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::condition::SqlValue;
    use crate::dsl::expr::Expr;

    fn make(table: &'static str) -> SelectBuilder {
        SelectBuilder {
            table: Some(table),
            columns: vec![],
            joins: vec![],
            wheres: vec![],
            or_wheres: vec![],
            group_by: vec![],
            havings: vec![],
            orders: vec![],
            limit: None,
            offset: None,
            distinct: false,
        }
    }

    #[test]
    fn basic_select_all() {
        let (sql, params) = make("users").to_sql_pg();
        assert_eq!(sql, "SELECT * FROM \"users\"");
        assert!(params.is_empty());
    }

    #[test]
    fn select_with_where_and_limit() {
        let b = make("posts")
            .where_(Expr::Eq(
                "\"posts\".\"user_id\"".into(),
                SqlValue::Integer(42),
            ))
            .limit(10)
            .offset(20);
        let (sql, params) = b.to_sql_pg();
        assert_eq!(
            sql,
            "SELECT * FROM \"posts\" WHERE \"posts\".\"user_id\" = $1 LIMIT 10 OFFSET 20"
        );
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn inner_join_renders() {
        let b = make("users").inner_join_raw(
            "posts",
            Expr::ColEq("\"posts\".\"user_id\"".into(), "\"users\".\"id\"".into()),
        );
        let (sql, params) = b.to_sql_pg();
        assert_eq!(
            sql,
            "SELECT * FROM \"users\" INNER JOIN \"posts\" ON \"posts\".\"user_id\" = \"users\".\"id\""
        );
        assert!(params.is_empty());
    }

    #[test]
    fn group_by_having_renders() {
        let b = make("orders")
            .group_by_raw(vec!["\"orders\".\"user_id\"".to_string()])
            .having(Expr::AggCmp(
                "COUNT(\"orders\".\"id\")".into(),
                ">",
                SqlValue::Integer(3),
            ));
        let (sql, params) = b.to_sql_pg();
        assert_eq!(
            sql,
            "SELECT * FROM \"orders\" GROUP BY \"orders\".\"user_id\" HAVING COUNT(\"orders\".\"id\") > $1"
        );
        assert_eq!(params.len(), 1);
    }
}

// ── Test helpers (raw string versions for unit tests) ────────────────────────

impl SelectBuilder {
    #[cfg(test)]
    fn inner_join_raw(mut self, table: &'static str, on: Expr) -> Self {
        self.joins.push(Join {
            kind: JoinKind::Inner,
            table: table.to_owned(),
            on: Some(on),
        });
        self
    }

    #[cfg(test)]
    fn group_by_raw(mut self, cols: Vec<String>) -> Self {
        self.group_by.extend(cols);
        self
    }
}
