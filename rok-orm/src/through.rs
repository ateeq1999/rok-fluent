//! [`ThroughQuery`] — indirect "has many through" / "has one through" relationship.
//!
//! SQL pattern:
//! ```sql
//! SELECT target.* FROM target
//! INNER JOIN through ON through.id = target.second_key
//! WHERE through.first_key = $1
//! ```
//!
//! Returned by `has_many_through!` and `has_one_through!` macros.

use std::marker::PhantomData;

use rok_orm_core::{sqlx_pg, Model, SqlValue};
use sqlx::postgres::PgRow;
use sqlx::PgPool;

use crate::pool;

/// An indirect relationship query via a bridge table.
///
/// Use `has_many_through!` or `has_one_through!` to construct one.
pub struct ThroughQuery<T> {
    owner_id: SqlValue,
    through: &'static str,
    first_key: &'static str,
    second_key: &'static str,
    extra_where: Vec<(String, SqlValue)>,
    order: Option<String>,
    limit: Option<u64>,
    offset: Option<u64>,
    _marker: PhantomData<T>,
}

// ── construction + chaining (Model bound only) ────────────────────────────────

impl<T: Model> ThroughQuery<T> {
    pub fn new(
        owner_id: impl Into<SqlValue>,
        through: &'static str,
        first_key: &'static str,
        second_key: &'static str,
    ) -> Self {
        Self {
            owner_id: owner_id.into(),
            through,
            first_key,
            second_key,
            extra_where: Vec::new(),
            order: None,
            limit: None,
            offset: None,
            _marker: PhantomData,
        }
    }

    /// Filter on a column of the target table.
    pub fn where_eq(mut self, col: &str, val: impl Into<SqlValue>) -> Self {
        self.extra_where.push((col.to_string(), val.into()));
        self
    }

    /// Order results ascending.
    pub fn order_by(mut self, col: &str) -> Self {
        self.order = Some(format!("{col} ASC"));
        self
    }

    /// Order results descending.
    pub fn order_by_desc(mut self, col: &str) -> Self {
        self.order = Some(format!("{col} DESC"));
        self
    }

    pub fn limit(mut self, n: u64) -> Self {
        self.limit = Some(n);
        self
    }

    pub fn offset(mut self, n: u64) -> Self {
        self.offset = Some(n);
        self
    }

    // ── SQL builders ──────────────────────────────────────────────────────────

    pub(crate) fn base_sql(&self) -> (String, Vec<SqlValue>) {
        let target = T::table_name();
        let through = self.through;
        let first_key = self.first_key;
        let second_key = self.second_key;

        let mut sql = format!(
            "SELECT {target}.* FROM {target} \
             INNER JOIN {through} ON {through}.id = {target}.{second_key} \
             WHERE {through}.{first_key} = $1"
        );
        let mut params: Vec<SqlValue> = vec![self.owner_id.clone()];

        for (col, val) in &self.extra_where {
            let ph = format!("${}", params.len() + 1);
            sql.push_str(&format!(" AND {target}.{col} = {ph}"));
            params.push(val.clone());
        }

        if let Some(ord) = &self.order {
            sql.push_str(&format!(" ORDER BY {ord}"));
        }

        (sql, params)
    }

    pub(crate) fn select_sql(&self) -> (String, Vec<SqlValue>) {
        let (mut sql, params) = self.base_sql();
        if let Some(lim) = self.limit {
            sql.push_str(&format!(" LIMIT {lim}"));
        }
        if let Some(off) = self.offset {
            sql.push_str(&format!(" OFFSET {off}"));
        }
        (sql, params)
    }

    pub(crate) fn count_sql(&self) -> (String, Vec<SqlValue>) {
        let target = T::table_name();
        let through = self.through;
        let first_key = self.first_key;
        let second_key = self.second_key;

        let mut sql = format!(
            "SELECT COUNT(*) FROM {target} \
             INNER JOIN {through} ON {through}.id = {target}.{second_key} \
             WHERE {through}.{first_key} = $1"
        );
        let mut params: Vec<SqlValue> = vec![self.owner_id.clone()];

        for (col, val) in &self.extra_where {
            let ph = format!("${}", params.len() + 1);
            sql.push_str(&format!(" AND {target}.{col} = {ph}"));
            params.push(val.clone());
        }

        (sql, params)
    }
}

// ── async terminals (full bounds required) ────────────────────────────────────

impl<T> ThroughQuery<T>
where
    T: Model + for<'r> sqlx::FromRow<'r, PgRow> + Send + Unpin,
{
    fn current_pool() -> Result<PgPool, sqlx::Error> {
        pool::try_current_pool().ok_or_else(|| {
            sqlx::Error::Configuration(
                "no database pool in scope — add OrmLayer to your router or \
                 call pool::with_pool() in tests"
                    .to_string()
                    .into(),
            )
        })
    }

    /// Fetch all related rows.
    pub async fn get(self) -> Result<Vec<T>, sqlx::Error> {
        let pool = Self::current_pool()?;
        let (sql, params) = self.select_sql();
        sqlx_pg::fetch_all_as::<T>(&pool, &sql, params).await
    }

    /// Fetch the first related row, or `None`.
    pub async fn first(self) -> Result<Option<T>, sqlx::Error> {
        let pool = Self::current_pool()?;
        let (mut sql, params) = self.base_sql();
        sql.push_str(" LIMIT 1");
        sqlx_pg::fetch_optional_as::<T>(&pool, &sql, params).await
    }

    /// Return the count of related rows.
    pub async fn count(self) -> Result<i64, sqlx::Error> {
        let pool = Self::current_pool()?;
        let (sql, params) = self.count_sql();
        let row = sqlx_pg::build_query(&sql, params).fetch_one(&pool).await?;
        use sqlx::Row;
        row.try_get::<i64, _>(0)
    }

    /// Return `true` if at least one related row exists.
    pub async fn exists(self) -> Result<bool, sqlx::Error> {
        Ok(self.count().await? > 0)
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    struct Post;
    impl Model for Post {
        fn table_name() -> &'static str {
            "posts"
        }
        fn columns() -> &'static [&'static str] {
            &["id", "title"]
        }
    }

    struct Comment;
    impl Model for Comment {
        fn table_name() -> &'static str {
            "comments"
        }
        fn columns() -> &'static [&'static str] {
            &["id", "body"]
        }
    }

    #[test]
    fn select_sql_basic() {
        let q = ThroughQuery::<Post>::new(7i64, "users", "country_id", "user_id");
        let (sql, params) = q.select_sql();
        assert_eq!(
            sql,
            "SELECT posts.* FROM posts \
             INNER JOIN users ON users.id = posts.user_id \
             WHERE users.country_id = $1"
        );
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn select_sql_with_where_eq() {
        let q = ThroughQuery::<Post>::new(7i64, "users", "country_id", "user_id")
            .where_eq("published", true);
        let (sql, params) = q.select_sql();
        assert!(sql.contains("AND posts.published = $2"), "sql={sql}");
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn select_sql_with_limit_offset() {
        let q = ThroughQuery::<Post>::new(7i64, "users", "country_id", "user_id")
            .limit(10)
            .offset(20);
        let (sql, _) = q.select_sql();
        assert!(sql.contains("LIMIT 10"), "sql={sql}");
        assert!(sql.contains("OFFSET 20"), "sql={sql}");
    }

    #[test]
    fn select_sql_order_by_desc() {
        let q = ThroughQuery::<Post>::new(7i64, "users", "country_id", "user_id")
            .order_by_desc("created_at");
        let (sql, _) = q.select_sql();
        assert!(sql.contains("ORDER BY created_at DESC"), "sql={sql}");
    }

    #[test]
    fn count_sql_basic() {
        let q = ThroughQuery::<Post>::new(7i64, "users", "country_id", "user_id");
        let (sql, params) = q.count_sql();
        assert!(sql.starts_with("SELECT COUNT(*)"), "sql={sql}");
        assert!(sql.contains("INNER JOIN users"), "sql={sql}");
        assert!(sql.contains("WHERE users.country_id = $1"), "sql={sql}");
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn has_one_through_pattern() {
        // has_one_through uses the same query; caller calls .first()
        let q = ThroughQuery::<Comment>::new(1i64, "posts", "user_id", "post_id");
        let (base, params) = q.base_sql();
        let first_sql = format!("{base} LIMIT 1");
        assert!(first_sql.contains("LIMIT 1"), "sql={first_sql}");
        assert_eq!(params.len(), 1);
    }
}
