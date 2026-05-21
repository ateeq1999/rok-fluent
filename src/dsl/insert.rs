//! [`InsertBuilder`] — typed `INSERT` query builder.

use super::{column::Column, table::Table};
use crate::core::condition::SqlValue;

/// On-conflict action for `INSERT … ON CONFLICT`.
#[derive(Debug, Clone)]
pub enum OnConflict {
    /// `ON CONFLICT DO NOTHING`
    DoNothing,
    /// `ON CONFLICT (cols) DO UPDATE SET …`
    DoUpdate {
        /// Conflict target columns (quoted names).
        target: Vec<String>,
        /// `SET col = EXCLUDED.col` pairs.
        sets: Vec<(String, ConflictSet)>,
    },
}

/// The value to assign in an `ON CONFLICT … DO UPDATE SET` clause.
#[derive(Debug, Clone)]
pub enum ConflictSet {
    /// `col = $N` — a bound parameter value.
    Value(SqlValue),
    /// `col = EXCLUDED.col` — use the value from the rejected row.
    Excluded,
}

/// A composable `INSERT INTO` query.
///
/// Created by [`db::insert_into()`](super::db::insert_into).
#[derive(Debug)]
#[must_use]
pub struct InsertBuilder {
    table: &'static str,
    columns: Vec<String>,
    values: Vec<SqlValue>,
    on_conflict: Option<OnConflict>,
    /// `None` = no RETURNING, `Some([])` = RETURNING *, `Some([col, …])` = specific cols.
    returning_cols: Option<Vec<String>>,
}

impl InsertBuilder {
    pub(crate) fn new<T: Table>(_table: T) -> Self {
        Self {
            table: T::table_name(),
            columns: Vec::new(),
            values: Vec::new(),
            on_conflict: None,
            returning_cols: None,
        }
    }

    /// Provide column-value pairs to insert (string column names).
    ///
    /// ```rust,ignore
    /// db::insert_into(User::table())
    ///     .values([("name", "Alice"), ("email", "alice@x.com")])
    ///     .execute(&pool).await?;
    /// ```
    pub fn values(
        mut self,
        pairs: impl IntoIterator<Item = (&'static str, impl Into<SqlValue>)>,
    ) -> Self {
        for (col, val) in pairs {
            self.columns.push(col.to_owned());
            self.values.push(val.into());
        }
        self
    }

    /// Provide typed column-value pairs using `Column<T, V>` constants.
    ///
    /// ```rust,ignore
    /// db::insert_into(User::table())
    ///     .values_typed([(User::NAME, "Alice"), (User::EMAIL, "alice@x.com")])
    ///     .fetch_one::<User>(&pool).await?;
    /// ```
    pub fn values_typed<TT, V: Into<SqlValue>>(
        mut self,
        pairs: impl IntoIterator<Item = (Column<TT, V>, V)>,
    ) -> Self {
        for (col, val) in pairs {
            self.columns.push(col.name.to_owned());
            self.values.push(val.into());
        }
        self
    }

    /// `ON CONFLICT DO NOTHING`
    pub fn on_conflict_do_nothing(mut self) -> Self {
        self.on_conflict = Some(OnConflict::DoNothing);
        self
    }

    /// Begin an `ON CONFLICT (col, …) DO UPDATE` clause.
    ///
    /// ```rust,ignore
    /// db::insert_into(User::table())
    ///     .values_typed([(User::EMAIL, email)])
    ///     .on_conflict([User::EMAIL])
    ///     .do_update_excluded([User::NAME])
    ///     .fetch_one::<User>(&pool).await?;
    /// ```
    pub fn on_conflict<TT, V>(mut self, cols: impl IntoIterator<Item = Column<TT, V>>) -> Self {
        let target: Vec<String> = cols
            .into_iter()
            .map(|c| format!("\"{}\"", c.name))
            .collect();
        self.on_conflict = Some(OnConflict::DoUpdate {
            target,
            sets: Vec::new(),
        });
        self
    }

    /// Add `SET col = EXCLUDED.col` pairs to the active `DO UPDATE` clause.
    pub fn do_update_excluded<TT, V>(
        mut self,
        cols: impl IntoIterator<Item = Column<TT, V>>,
    ) -> Self {
        if let Some(OnConflict::DoUpdate { sets, .. }) = &mut self.on_conflict {
            for col in cols {
                sets.push((format!("\"{}\"", col.name), ConflictSet::Excluded));
            }
        }
        self
    }

    /// Add `SET col = $N` pairs to the active `DO UPDATE` clause.
    pub fn do_update_values<TT, V: Into<SqlValue>>(
        mut self,
        pairs: impl IntoIterator<Item = (Column<TT, V>, V)>,
    ) -> Self {
        if let Some(OnConflict::DoUpdate { sets, .. }) = &mut self.on_conflict {
            for (col, val) in pairs {
                sets.push((format!("\"{}\"", col.name), ConflictSet::Value(val.into())));
            }
        }
        self
    }

    /// Add `RETURNING *` to the query.
    pub fn returning(mut self) -> Self {
        self.returning_cols = Some(Vec::new());
        self
    }

    /// Add `RETURNING col1, col2, …` to the query.
    ///
    /// ```rust,ignore
    /// .returning_cols([User::ID, User::CREATED_AT])
    /// ```
    pub fn returning_cols<TT, V>(mut self, cols: impl IntoIterator<Item = Column<TT, V>>) -> Self {
        let names: Vec<String> = cols
            .into_iter()
            .map(|c| format!("\"{}\"", c.name))
            .collect();
        self.returning_cols = Some(names);
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
        tracing::debug!(sql = %sql, ?params, "rok-fluent insert");
        self
    }

    /// Render to `(sql, params)` using PostgreSQL `$N` placeholders.
    pub fn to_sql_pg(&self) -> (String, Vec<SqlValue>) {
        let cols: Vec<String> = self.columns.iter().map(|c| format!("\"{c}\"")).collect();
        let mut params: Vec<SqlValue> = self.values.clone();
        let phs: Vec<String> = (1..=self.values.len()).map(|i| format!("${i}")).collect();
        let mut sql = format!(
            "INSERT INTO \"{}\" ({}) VALUES ({})",
            self.table,
            cols.join(", "),
            phs.join(", "),
        );

        // ON CONFLICT
        if let Some(conflict) = &self.on_conflict {
            match conflict {
                OnConflict::DoNothing => sql.push_str(" ON CONFLICT DO NOTHING"),
                OnConflict::DoUpdate { target, sets } => {
                    let target_str = target.join(", ");
                    sql.push_str(&format!(" ON CONFLICT ({target_str}) DO UPDATE SET "));
                    let set_parts: Vec<String> = sets
                        .iter()
                        .map(|(col, val)| match val {
                            ConflictSet::Excluded => {
                                let bare = col.trim_matches('"');
                                format!("{col} = EXCLUDED.\"{bare}\"")
                            }
                            ConflictSet::Value(v) => {
                                params.push(v.clone());
                                format!("{col} = ${}", params.len())
                            }
                        })
                        .collect();
                    sql.push_str(&set_parts.join(", "));
                }
            }
        }

        // RETURNING
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
        if self.returning_cols.is_none() {
            self.returning_cols = Some(Vec::new());
        }
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
        assert!(sql.ends_with("RETURNING *"), "got: {sql}");
    }

    #[test]
    fn insert_on_conflict_do_nothing() {
        let b = InsertBuilder::new(UsersTable)
            .values([("email", SqlValue::Text("a@x.com".into()))])
            .on_conflict_do_nothing();
        let (sql, params) = b.to_sql_pg();
        assert!(sql.contains("ON CONFLICT DO NOTHING"), "got: {sql}");
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn insert_returning_specific_cols() {
        let b = InsertBuilder::new(UsersTable).values([("name", SqlValue::Text("Carol".into()))]);
        // Simulate returning_cols with raw strings since we have no Column in unit tests.
        let mut b = b;
        b.returning_cols = Some(vec!["\"id\"".to_owned(), "\"created_at\"".to_owned()]);
        let (sql, _) = b.to_sql_pg();
        assert!(
            sql.contains("RETURNING \"id\", \"created_at\""),
            "got: {sql}"
        );
    }
}
