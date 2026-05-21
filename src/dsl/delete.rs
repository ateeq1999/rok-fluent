//! [`DeleteBuilder`] — typed `DELETE` query builder.

use super::{expr::Expr, table::Table};
use crate::core::condition::SqlValue;

/// A composable `DELETE FROM` query.
///
/// Created by [`db::delete_from()`](super::db::delete_from).
#[derive(Debug)]
#[must_use]
pub struct DeleteBuilder {
    table: &'static str,
    wheres: Vec<Expr>,
}

impl DeleteBuilder {
    pub(crate) fn new<T: Table>(_table: T) -> Self {
        Self {
            table: T::table_name(),
            wheres: Vec::new(),
        }
    }

    /// Add a `WHERE` predicate (multiple calls are `AND`-ed).
    pub fn where_(mut self, expr: Expr) -> Self {
        self.wheres.push(expr);
        self
    }

    /// Print the rendered SQL and bound parameters to `stderr` without breaking the chain.
    ///
    /// When the `tracing` feature is enabled, also emits a `tracing::debug!` event.
    pub fn inspect(self) -> Self {
        let (sql, params) = self.to_sql_pg();
        eprintln!("[rok-fluent] {sql}");
        if !params.is_empty() {
            eprintln!("[rok-fluent] params: {params:?}");
        }
        #[cfg(feature = "tracing")]
        tracing::debug!(sql = %sql, ?params, "rok-fluent delete");
        self
    }

    /// Render to `(sql, params)` using PostgreSQL `$N` placeholders.
    pub fn to_sql_pg(&self) -> (String, Vec<SqlValue>) {
        let mut sql = format!("DELETE FROM \"{}\"", self.table);
        let mut params: Vec<SqlValue> = Vec::new();

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
impl DeleteBuilder {
    /// Execute and return the number of rows deleted.
    pub async fn execute(self, pool: &sqlx::PgPool) -> Result<u64, sqlx::Error> {
        let (sql, params) = self.to_sql_pg();
        crate::core::sqlx::pg::execute(pool, &sql, params).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::condition::SqlValue;

    struct UsersTable;
    impl Table for UsersTable {
        fn table_name() -> &'static str {
            "users"
        }
    }

    #[test]
    fn delete_with_where() {
        let b = DeleteBuilder::new(UsersTable)
            .where_(Expr::Eq("\"users\".\"id\"".into(), SqlValue::Integer(9)));
        let (sql, params) = b.to_sql_pg();
        assert_eq!(sql, "DELETE FROM \"users\" WHERE \"users\".\"id\" = $1");
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn delete_all_no_where() {
        let b = DeleteBuilder::new(UsersTable);
        let (sql, params) = b.to_sql_pg();
        assert_eq!(sql, "DELETE FROM \"users\"");
        assert!(params.is_empty());
    }
}
