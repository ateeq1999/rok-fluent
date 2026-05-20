//! [`UpdateBuilder`] — typed `UPDATE` query builder.

use super::{expr::Expr, table::Table};
use crate::core::condition::SqlValue;

/// A composable `UPDATE` query.
///
/// Created by [`db::update()`](super::db::update).
#[derive(Debug)]
#[must_use]
pub struct UpdateBuilder {
    table: &'static str,
    sets: Vec<(&'static str, SqlValue)>,
    wheres: Vec<Expr>,
}

impl UpdateBuilder {
    pub(crate) fn new<T: Table>(_table: T) -> Self {
        Self {
            table: T::table_name(),
            sets: Vec::new(),
            wheres: Vec::new(),
        }
    }

    /// Set `column = value`.  Call multiple times for multiple columns.
    ///
    /// ```rust,ignore
    /// db::update(users::table)
    ///     .set("name", "Alice")
    ///     .set("active", true)
    ///     .where_(users::id.eq(1_i64))
    ///     .execute(&pool).await?;
    /// ```
    pub fn set(mut self, col: &'static str, val: impl Into<SqlValue>) -> Self {
        self.sets.push((col, val.into()));
        self
    }

    /// Add a `WHERE` predicate (multiple calls are `AND`-ed).
    pub fn where_(mut self, expr: Expr) -> Self {
        self.wheres.push(expr);
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
}
