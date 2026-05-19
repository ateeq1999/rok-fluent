//! [`MysqlModel`] — ergonomic async CRUD for any [`Model`] + [`sqlx::FromRow`] type
//! backed by MySQL.

use rok_orm_core::{Model, QueryBuilder, SqlValue};
use sqlx::{mysql::MySqlRow, MySqlPool};

use crate::mysql_executor;

/// Blanket async CRUD extension for any type that implements [`Model`] and
/// [`sqlx::FromRow<MySqlRow>`].
pub trait MysqlModel: Model + for<'r> sqlx::FromRow<'r, MySqlRow> + Send + Unpin {
    /// Fetch every row from the model's table.
    fn all(
        pool: &MySqlPool,
    ) -> impl std::future::Future<Output = Result<Vec<Self>, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        mysql_executor::fetch_all(pool, Self::query())
    }

    /// Fetch rows matching a custom query.
    fn find_where(
        pool: &MySqlPool,
        builder: QueryBuilder<Self>,
    ) -> impl std::future::Future<Output = Result<Vec<Self>, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        mysql_executor::fetch_all(pool, builder)
    }

    /// Fetch a single row by primary key. Returns `None` if not found.
    fn find_by_pk(
        pool: &MySqlPool,
        id: impl Into<SqlValue> + Send,
    ) -> impl std::future::Future<Output = Result<Option<Self>, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        mysql_executor::fetch_optional(pool, Self::find(id))
    }

    /// Fetch by PK or return `Err(RowNotFound)`.
    fn find_or_fail(
        pool: &MySqlPool,
        id: impl Into<SqlValue> + Send,
    ) -> impl std::future::Future<Output = Result<Self, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        async move {
            match mysql_executor::fetch_optional(pool, Self::find(id)).await? {
                Some(row) => Ok(row),
                None => Err(sqlx::Error::RowNotFound),
            }
        }
    }

    /// Total row count.
    fn count(pool: &MySqlPool) -> impl std::future::Future<Output = Result<i64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        mysql_executor::count(pool, Self::query())
    }

    /// Row count matching a custom query.
    fn count_where(
        pool: &MySqlPool,
        builder: QueryBuilder<Self>,
    ) -> impl std::future::Future<Output = Result<i64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        mysql_executor::count(pool, builder)
    }

    /// Return the first row by primary key ascending.
    fn first(
        pool: &MySqlPool,
    ) -> impl std::future::Future<Output = Result<Option<Self>, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        mysql_executor::fetch_optional(pool, Self::query().limit(1))
    }

    /// Return the last row by primary key descending.
    fn last(
        pool: &MySqlPool,
    ) -> impl std::future::Future<Output = Result<Option<Self>, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        let pk = Self::primary_key();
        mysql_executor::fetch_optional(pool, Self::query().order_by_desc(pk).limit(1))
    }

    /// Build a filter query (`WHERE col = val`).
    fn filter(col: &str, val: impl Into<SqlValue>) -> QueryBuilder<Self>
    where
        Self: Sized,
    {
        Self::query().where_eq(col, val)
    }

    /// Build a query for a specific PK value.
    fn find_query(id: impl Into<SqlValue>) -> QueryBuilder<Self>
    where
        Self: Sized,
    {
        Self::find(id)
    }

    /// INSERT a new row and return the `LAST_INSERT_ID()`.
    fn create(
        pool: &MySqlPool,
        data: &[(&str, SqlValue)],
    ) -> impl std::future::Future<Output = Result<i64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        mysql_executor::insert(pool, Self::table_name(), data)
    }

    /// UPDATE by primary key, return affected rows.
    fn update_by_pk(
        pool: &MySqlPool,
        id: impl Into<SqlValue> + Send,
        data: &[(&str, SqlValue)],
    ) -> impl std::future::Future<Output = Result<u64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        mysql_executor::update(
            pool,
            Self::table_name(),
            Self::primary_key(),
            id.into(),
            data,
        )
    }

    /// DELETE by primary key, return affected rows.
    fn delete_by_pk(
        pool: &MySqlPool,
        id: impl Into<SqlValue> + Send,
    ) -> impl std::future::Future<Output = Result<u64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        mysql_executor::delete(pool, Self::table_name(), Self::primary_key(), id.into())
    }

    /// DELETE using this instance's PK value.
    fn delete_self(
        &self,
        pool: &MySqlPool,
    ) -> impl std::future::Future<Output = Result<u64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        let val = self.pk_value();
        mysql_executor::delete(pool, Self::table_name(), Self::primary_key(), val)
    }

    /// SUM(col) across the table.
    fn sum(
        pool: &MySqlPool,
        col: &str,
    ) -> impl std::future::Future<Output = Result<Option<f64>, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        let expr = format!("SUM(`{col}`)");
        async move { mysql_executor::aggregate(pool, Self::table_name(), &expr, "", vec![]).await }
    }

    /// AVG(col) across the table.
    fn avg(
        pool: &MySqlPool,
        col: &str,
    ) -> impl std::future::Future<Output = Result<Option<f64>, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        let expr = format!("AVG(`{col}`)");
        async move { mysql_executor::aggregate(pool, Self::table_name(), &expr, "", vec![]).await }
    }

    /// MAX(col) across the table.
    fn max(
        pool: &MySqlPool,
        col: &str,
    ) -> impl std::future::Future<Output = Result<Option<f64>, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        let expr = format!("MAX(`{col}`)");
        async move { mysql_executor::aggregate(pool, Self::table_name(), &expr, "", vec![]).await }
    }

    /// MIN(col) across the table.
    fn min(
        pool: &MySqlPool,
        col: &str,
    ) -> impl std::future::Future<Output = Result<Option<f64>, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        let expr = format!("MIN(`{col}`)");
        async move { mysql_executor::aggregate(pool, Self::table_name(), &expr, "", vec![]).await }
    }
}

impl<T> MysqlModel for T where T: Model + for<'r> sqlx::FromRow<'r, MySqlRow> + Send + Unpin {}
