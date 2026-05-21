//! [`UpdateBuilder`] — typed `UPDATE` query builder.

use super::{column::Column, expr::Expr, table::Table};
use crate::core::condition::SqlValue;

/// A composable `UPDATE` query.
///
/// Created by [`db::update()`](super::db::update).
#[derive(Debug)]
#[must_use]
pub struct UpdateBuilder {
    table: &'static str,
    sets: Vec<(String, SqlValue)>,
    wheres: Vec<Expr>,
    /// `None` = no RETURNING, `Some([])` = RETURNING *, `Some([col, …])` = specific cols.
    returning_cols: Option<Vec<String>>,
}

impl UpdateBuilder {
    pub(crate) fn new<T: Table>(_table: T) -> Self {
        Self {
            table: T::table_name(),
            sets: Vec::new(),
            wheres: Vec::new(),
            returning_cols: None,
        }
    }

    /// Set `column = value` by string column name.
    ///
    /// ```rust,ignore
    /// db::update(User::table())
    ///     .set("name", "Alice")
    ///     .where_(User::ID.eq(1_i64))
    ///     .execute(&pool).await?;
    /// ```
    pub fn set(mut self, col: &'static str, val: impl Into<SqlValue>) -> Self {
        self.sets.push((col.to_owned(), val.into()));
        self
    }

    /// Set `column = value` using a typed `Column<T, V>` constant.
    ///
    /// ```rust,ignore
    /// db::update(User::table())
    ///     .set_col(User::NAME, "Alice")
    ///     .where_(User::ID.eq(1_i64))
    ///     .execute(&pool).await?;
    /// ```
    pub fn set_col<TT, V: Into<SqlValue>>(mut self, col: Column<TT, V>, val: V) -> Self {
        self.sets.push((col.name.to_owned(), val.into()));
        self
    }

    /// Set multiple columns at once using typed `Column<T, V>` pairs.
    ///
    /// ```rust,ignore
    /// db::update(User::table())
    ///     .set_typed([(User::NAME, "Alice"), (User::EMAIL, "a@b.com")])
    ///     .where_(User::ID.eq(1_i64))
    ///     .execute(&pool).await?;
    /// ```
    pub fn set_typed<TT, V: Into<SqlValue>>(
        mut self,
        pairs: impl IntoIterator<Item = (Column<TT, V>, V)>,
    ) -> Self {
        for (col, val) in pairs {
            self.sets.push((col.name.to_owned(), val.into()));
        }
        self
    }

    /// Add a `WHERE` predicate (multiple calls are `AND`-ed).
    pub fn where_(mut self, expr: Expr) -> Self {
        self.wheres.push(expr);
        self
    }

    /// Add `RETURNING *`.
    pub fn returning(mut self) -> Self {
        self.returning_cols = Some(Vec::new());
        self
    }

    /// Add `RETURNING col1, col2, …`.
    ///
    /// ```rust,ignore
    /// db::update(User::table())
    ///     .set_col(User::NAME, "Bob")
    ///     .where_(User::ID.eq(1_i64))
    ///     .returning_cols([User::ID, User::NAME])
    ///     .fetch_one::<User>(&pool).await?;
    /// ```
    pub fn returning_cols<TT, V>(mut self, cols: impl IntoIterator<Item = Column<TT, V>>) -> Self {
        let names: Vec<String> = cols
            .into_iter()
            .map(|c| format!("\"{}\"", c.name))
            .collect();
        self.returning_cols = Some(names);
        self
    }

    /// Render to `(sql, params)` using PostgreSQL `$N` placeholders.
    ///
    /// # Panics
    ///
    /// Panics if no `SET` columns were provided (would generate invalid SQL).
    pub fn to_sql_pg(&self) -> (String, Vec<SqlValue>) {
        assert!(
            !self.sets.is_empty(),
            "UpdateBuilder: at least one .set() is required"
        );
        let mut params: Vec<SqlValue> = Vec::new();

        let set_clause: Vec<String> = self
            .sets
            .iter()
            .enumerate()
            .map(|(i, (col, val))| {
                params.push(val.clone());
                format!("\"{}\" = ${}", col, i + 1)
            })
            .collect();

        let mut sql = format!("UPDATE \"{}\" SET {}", self.table, set_clause.join(", "));

        if !self.wheres.is_empty() {
            let mut frags = Vec::new();
            for expr in &self.wheres {
                let (s, p) = expr.to_sql_pg(params.len() + 1);
                frags.push(s);
                params.extend(p);
            }
            sql.push_str(&format!(" WHERE {}", frags.join(" AND ")));
        }

        if let Some(ret_cols) = &self.returning_cols {
            if ret_cols.is_empty() {
                sql.push_str(" RETURNING *");
            } else {
                sql.push_str(&format!(" RETURNING {}", ret_cols.join(", ")));
            }
        }

        (sql, params)
    }
}

// ── PostgreSQL async terminals ────────────────────────────────────────────────

#[cfg(feature = "postgres")]
impl UpdateBuilder {
    /// Execute and return the number of rows affected.
    pub async fn execute(self, pool: &sqlx::PgPool) -> Result<u64, sqlx::Error> {
        let (sql, params) = self.to_sql_pg();
        crate::core::sqlx::pg::execute(pool, &sql, params).await
    }

    /// Execute with `RETURNING *` and return the first updated row.
    pub async fn fetch_one<T>(mut self, pool: &sqlx::PgPool) -> Result<T, sqlx::Error>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        if self.returning_cols.is_none() {
            self.returning_cols = Some(Vec::new());
        }
        let (sql, params) = self.to_sql_pg();
        crate::core::sqlx::pg::fetch_optional_as::<T>(pool, &sql, params)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    /// Execute with `RETURNING *` and return all updated rows.
    pub async fn fetch_all<T>(mut self, pool: &sqlx::PgPool) -> Result<Vec<T>, sqlx::Error>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        if self.returning_cols.is_none() {
            self.returning_cols = Some(Vec::new());
        }
        let (sql, params) = self.to_sql_pg();
        crate::core::sqlx::pg::fetch_all_as::<T>(pool, &sql, params).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::condition::SqlValue;

    struct PostsTable;
    impl Table for PostsTable {
        fn table_name() -> &'static str {
            "posts"
        }
    }

    #[test]
    fn update_with_where() {
        let b = UpdateBuilder::new(PostsTable)
            .set("title", SqlValue::Text("New".into()))
            .where_(Expr::Eq("\"posts\".\"id\"".into(), SqlValue::Integer(5)));
        let (sql, params) = b.to_sql_pg();
        assert_eq!(
            sql,
            "UPDATE \"posts\" SET \"title\" = $1 WHERE \"posts\".\"id\" = $2"
        );
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn update_returning_star() {
        let b = UpdateBuilder::new(PostsTable)
            .set("title", SqlValue::Text("New".into()))
            .returning();
        let (sql, _) = b.to_sql_pg();
        assert!(sql.ends_with("RETURNING *"), "got: {sql}");
    }
}
