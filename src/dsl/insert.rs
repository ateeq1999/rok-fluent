//! [`InsertBuilder`] — typed `INSERT` query builder.

use super::table::Table;
use crate::core::condition::SqlValue;

/// A composable `INSERT INTO` query.
///
/// Created by [`db::insert_into()`](super::db::insert_into).
#[derive(Debug)]
#[must_use]
pub struct InsertBuilder {
    table: &'static str,
    columns: Vec<&'static str>,
    values: Vec<SqlValue>,
    returning: bool,
}

impl InsertBuilder {
    pub(crate) fn new<T: Table>(_table: T) -> Self {
        Self {
            table: T::table_name(),
            columns: Vec::new(),
            values: Vec::new(),
            returning: false,
        }
    }

    /// Provide column-value pairs to insert.
    ///
    /// ```rust,ignore
    /// db::insert_into(users::table)
    ///     .values([(users::name, "Alice"), (users::email, "alice@x.com")])
    ///     .execute(&pool).await?;
    /// ```
    pub fn values(
        mut self,
        pairs: impl IntoIterator<Item = (&'static str, impl Into<SqlValue>)>,
    ) -> Self {
        for (col, val) in pairs {
            self.columns.push(col);
            self.values.push(val.into());
        }
        self
    }

    /// Add `RETURNING *` — chain with `.fetch_one()` / `.fetch_all()`.
    pub fn returning(mut self) -> Self {
        self.returning = true;
        self
    }

    /// Render to `(sql, params)` using PostgreSQL `$N` placeholders.
    pub fn to_sql_pg(&self) -> (String, Vec<SqlValue>) {
        let cols: Vec<String> = self.columns.iter().map(|c| format!("\"{c}\"")).collect();
        let phs: Vec<String> = (1..=self.values.len()).map(|i| format!("${i}")).collect();
        let mut sql = format!(
            "INSERT INTO \"{}\" ({}) VALUES ({})",
            self.table,
            cols.join(", "),
            phs.join(", "),
        );
        if self.returning {
            sql.push_str(" RETURNING *");
        }
        (sql, self.values.clone())
    }
}

// ── PostgreSQL async terminals ────────────────────────────────────────────────

#[cfg(feature = "postgres")]
impl InsertBuilder {
    /// Execute the insert and return the number of rows affected.
    pub async fn execute(self, pool: &sqlx::PgPool) -> Result<u64, sqlx::Error> {
        let (sql, params) = self.to_sql_pg();
        crate::core::sqlx::pg::execute(pool, &sql, params).await
    }

    /// Execute with `RETURNING *` and return the inserted row.
    pub async fn fetch_one<T>(mut self, pool: &sqlx::PgPool) -> Result<T, sqlx::Error>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        self.returning = true;
        let (sql, params) = self.to_sql_pg();
        crate::core::sqlx::pg::fetch_optional_as::<T>(pool, &sql, params)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    /// Execute with `RETURNING *` and return all inserted rows.
    pub async fn fetch_all<T>(mut self, pool: &sqlx::PgPool) -> Result<Vec<T>, sqlx::Error>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        self.returning = true;
        let (sql, params) = self.to_sql_pg();
        crate::core::sqlx::pg::fetch_all_as::<T>(pool, &sql, params).await
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
    fn basic_insert() {
        let b = InsertBuilder::new(UsersTable).values([
            ("name", SqlValue::Text("Alice".into())),
            ("email", SqlValue::Text("a@x.com".into())),
        ]);
        let (sql, params) = b.to_sql_pg();
        assert_eq!(
            sql,
            "INSERT INTO \"users\" (\"name\", \"email\") VALUES ($1, $2)"
        );
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn insert_returning() {
        let b = InsertBuilder::new(UsersTable)
            .values([("name", SqlValue::Text("Bob".into()))])
            .returning();
        let (sql, _) = b.to_sql_pg();
        assert!(sql.ends_with("RETURNING *"));
    }
}
