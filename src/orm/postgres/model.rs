//! [`PgModel`] — ergonomic async CRUD methods for any [`Model`] + [`sqlx::FromRow`] type.
//!
//! All methods are provided as defaults; no manual implementation is required.
//!
//! # Example
//!
//! ```rust,no_run
//! # use rok_fluent::orm::postgres::model::PgModel;
//! # use rok_fluent::core::model::Model;
//! # use rok_fluent::core::condition::SqlValue;
//! # #[derive(Debug, sqlx::FromRow)]
//! # pub struct User { pub id: i64, pub name: String }
//! # impl Model for User {
//! #     fn table_name() -> &'static str { "users" }
//! #     fn columns() -> &'static [&'static str] { &["id", "name"] }
//! # }
//! # async fn example(pool: sqlx::PgPool) -> Result<(), sqlx::Error> {
//! let all: Vec<User>    = User::all(&pool).await?;
//! let one: Option<User> = User::find_by_pk(&pool, 1i64).await?;
//! let n: i64            = User::count(&pool).await?;
//! User::create(&pool, &[("name", "Alice".into())]).await?;
//! User::update_by_pk(&pool, 1i64, &[("name", "Bob".into())]).await?;
//! User::delete_by_pk(&pool, 1i64).await?;
//! # Ok(())
//! # }
//! ```

use sqlx::{postgres::PgRow, PgPool};

use super::executor;
use crate::core::condition::SqlValue;
use crate::core::model::Model;
use crate::core::query::QueryBuilder;
use crate::core::sqlx::pg as sqlx_pg;
use crate::orm::model_query::ModelQuery;

/// Blanket async CRUD extension for any type that implements [`Model`] and
/// [`sqlx::FromRow`].
///
/// Implemented automatically for every such type — no `impl` block needed.
pub trait PgModel: Model + for<'r> sqlx::FromRow<'r, PgRow> + Send + Unpin + 'static {
    /// Fetch every row from the model's table.
    fn all(
        pool: &PgPool,
    ) -> impl std::future::Future<Output = Result<Vec<Self>, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        executor::fetch_all(pool, Self::query())
    }

    /// Fetch rows matching a custom query.
    fn find_where(
        pool: &PgPool,
        builder: QueryBuilder<Self>,
    ) -> impl std::future::Future<Output = Result<Vec<Self>, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        executor::fetch_all(pool, builder)
    }

    /// Fetch a single row by primary key.  Returns `None` if not found.
    fn find_by_pk(
        pool: &PgPool,
        id: impl Into<SqlValue> + Send,
    ) -> impl std::future::Future<Output = Result<Option<Self>, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        executor::fetch_optional(pool, Self::find(id))
    }

    /// Return the total number of rows in the model's table.
    fn count(pool: &PgPool) -> impl std::future::Future<Output = Result<i64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        executor::count(pool, Self::query())
    }

    /// Return the number of rows matching a custom query.
    fn count_where(
        pool: &PgPool,
        builder: QueryBuilder<Self>,
    ) -> impl std::future::Future<Output = Result<i64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        executor::count(pool, builder)
    }

    /// Insert a row using the given column-value pairs and return rows affected.
    fn create(
        pool: &PgPool,
        data: &[(&str, SqlValue)],
    ) -> impl std::future::Future<Output = Result<u64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        executor::insert::<Self>(pool, Self::table_name(), data)
    }

    /// Update the row with the given primary key and return rows affected.
    fn update_by_pk(
        pool: &PgPool,
        id: impl Into<SqlValue> + Send,
        data: &[(&str, SqlValue)],
    ) -> impl std::future::Future<Output = Result<u64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        let builder = Self::find(id);
        executor::update::<Self>(pool, builder, data)
    }

    /// Delete the row with the given primary key and return rows affected.
    fn delete_by_pk(
        pool: &PgPool,
        id: impl Into<SqlValue> + Send,
    ) -> impl std::future::Future<Output = Result<u64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        executor::delete(pool, Self::find(id))
    }

    /// Delete rows matching a custom query and return rows affected.
    fn delete_where(
        pool: &PgPool,
        builder: QueryBuilder<Self>,
    ) -> impl std::future::Future<Output = Result<u64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        executor::delete(pool, builder)
    }

    /// Update rows matching a custom query and return rows affected.
    ///
    /// ```rust,no_run
    /// # use rok_fluent::orm::postgres::model::PgModel;
    /// # use rok_fluent::core::model::Model;
    /// # #[derive(Debug, sqlx::FromRow)]
    /// # pub struct User { pub id: i64, pub role: String, pub active: bool }
    /// # impl Model for User {
    /// #     fn table_name() -> &'static str { "users" }
    /// #     fn columns() -> &'static [&'static str] { &["id", "role", "active"] }
    /// # }
    /// # async fn example(pool: sqlx::PgPool) -> Result<(), sqlx::Error> {
    /// User::update_where(
    ///     &pool,
    ///     User::query().where_eq("role", "guest"),
    ///     &[("active", false.into())],
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    fn update_where(
        pool: &PgPool,
        builder: QueryBuilder<Self>,
        data: &[(&str, SqlValue)],
    ) -> impl std::future::Future<Output = Result<u64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        executor::update::<Self>(pool, builder, data)
    }

    /// Insert multiple rows in one statement and return rows affected.
    ///
    /// All rows must have the same column keys in the same order.
    fn bulk_create(
        pool: &PgPool,
        rows: &[Vec<(&str, SqlValue)>],
    ) -> impl std::future::Future<Output = Result<u64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        executor::bulk_insert::<Self>(pool, Self::table_name(), rows)
    }

    /// Insert multiple rows with `ON CONFLICT … DO UPDATE` in a single statement.
    ///
    /// All rows must have the same column keys in the same order.  Rows that
    /// match the `conflict_cols` constraint are updated with all non-conflict
    /// column values; rows that don't match are inserted.
    fn bulk_upsert(
        pool: &PgPool,
        rows: &[Vec<(&str, SqlValue)>],
        conflict_cols: &[&str],
    ) -> impl std::future::Future<Output = Result<u64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        executor::bulk_upsert::<Self>(pool, rows, conflict_cols)
    }

    /// Insert rows in chunks of `chunk_size`, returning total rows affected.
    ///
    /// Useful when the row count would exceed PostgreSQL's parameter limit (~65k).
    fn bulk_insert_chunked(
        pool: &PgPool,
        rows: &[Vec<(&str, SqlValue)>],
        chunk_size: usize,
    ) -> impl std::future::Future<Output = Result<u64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        let pool = pool.clone();
        let owned: Vec<Vec<(String, SqlValue)>> = rows
            .iter()
            .map(|r| r.iter().map(|(k, v)| (k.to_string(), v.clone())).collect())
            .collect();
        async move {
            let mut total = 0u64;
            for chunk in owned.chunks(chunk_size) {
                let data: Vec<Vec<(&str, SqlValue)>> = chunk
                    .iter()
                    .map(|r| r.iter().map(|(k, v)| (k.as_str(), v.clone())).collect())
                    .collect();
                total += executor::bulk_insert::<Self>(&pool, Self::table_name(), &data).await?;
            }
            Ok(total)
        }
    }

    /// Insert a row and return the full inserted row via `RETURNING *`.
    ///
    /// Ideal for getting back generated IDs or server-side default columns.
    fn create_returning(
        pool: &PgPool,
        data: &[(&str, SqlValue)],
    ) -> impl std::future::Future<Output = Result<Self, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        executor::insert_returning::<Self>(pool, Self::table_name(), data)
    }

    // ── pool-free fluent entry points ─────────────────────────────────────

    /// Start a pool-free fluent query filtered by `col = val`.
    ///
    /// Requires [`OrmLayer`](crate::orm::orm_layer::OrmLayer) on the router
    /// (or [`pool::with_pool`](crate::orm::postgres::pool::with_pool) in tests).
    fn filter(col: &str, val: impl Into<SqlValue>) -> ModelQuery<Self>
    where
        Self: Sized + Sync + 'static,
    {
        ModelQuery::new(Self::query().where_eq(col, val))
    }

    /// Start a pool-free fluent query returning all rows (soft-delete aware).
    fn all_query() -> ModelQuery<Self>
    where
        Self: Sized + Sync + 'static,
    {
        ModelQuery::new(Self::query())
    }

    /// Start a pool-free fluent query for a single row by primary key.
    fn find_query(id: impl Into<SqlValue>) -> ModelQuery<Self>
    where
        Self: Sized + Sync + 'static,
    {
        ModelQuery::new(Self::find(id))
    }

    // ── soft-delete operations ────────────────────────────────────────────

    /// Fetch a single row by primary key — returns an error if not found.
    ///
    /// Unlike `find_by_pk` which returns `Option<Self>`, this method returns
    /// `Err(sqlx::Error::RowNotFound)` when no row matches.
    fn find_or_fail(
        pool: &PgPool,
        id: impl Into<SqlValue> + Send,
    ) -> impl std::future::Future<Output = Result<Self, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        let pool = pool.clone();
        let builder = Self::find(id);
        async move {
            executor::fetch_optional(&pool, builder)
                .await?
                .ok_or(sqlx::Error::RowNotFound)
        }
    }

    /// Delete this model instance from the database by its primary key.
    fn delete_self(
        &self,
        pool: &PgPool,
    ) -> impl std::future::Future<Output = Result<u64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        let pk = self.pk_value();
        let pool = pool.clone();
        async move { executor::delete::<Self>(&pool, Self::find(pk)).await }
    }

    /// Insert multiple rows and return all inserted rows via `RETURNING *`.
    ///
    /// All rows must have the same column keys in the same order.
    fn insert_many(
        pool: &PgPool,
        rows: &[Vec<(&str, SqlValue)>],
    ) -> impl std::future::Future<Output = Result<Vec<Self>, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        executor::bulk_insert_returning::<Self>(pool, Self::table_name(), rows)
    }

    /// Upsert a row (`INSERT … ON CONFLICT DO UPDATE`) and return the affected row.
    fn upsert_returning(
        pool: &PgPool,
        data: &[(&str, SqlValue)],
        conflict_cols: &[&str],
    ) -> impl std::future::Future<Output = Result<Self, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        executor::upsert_returning::<Self>(pool, data, conflict_cols)
    }

    /// Compute the `SUM` of `col` over all rows matching this model's table.
    ///
    /// Returns `None` when no rows exist or all values are `NULL`.
    fn sum(
        pool: &PgPool,
        col: &str,
    ) -> impl std::future::Future<Output = Result<Option<f64>, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        let agg = format!("SUM({col})");
        let pool = pool.clone();
        let builder = Self::query();
        async move { executor::aggregate(&pool, builder, &agg).await }
    }

    /// Compute the `AVG` of `col` over all rows matching this model's table.
    fn avg(
        pool: &PgPool,
        col: &str,
    ) -> impl std::future::Future<Output = Result<Option<f64>, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        let agg = format!("AVG({col})");
        let pool = pool.clone();
        let builder = Self::query();
        async move { executor::aggregate(&pool, builder, &agg).await }
    }

    /// Return the `MAX` value of `col` over all rows matching this model's table.
    fn max(
        pool: &PgPool,
        col: &str,
    ) -> impl std::future::Future<Output = Result<Option<f64>, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        let agg = format!("MAX({col})");
        let pool = pool.clone();
        let builder = Self::query();
        async move { executor::aggregate(&pool, builder, &agg).await }
    }

    /// Return the `MIN` value of `col` over all rows matching this model's table.
    fn min(
        pool: &PgPool,
        col: &str,
    ) -> impl std::future::Future<Output = Result<Option<f64>, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        let agg = format!("MIN({col})");
        let pool = pool.clone();
        let builder = Self::query();
        async move { executor::aggregate(&pool, builder, &agg).await }
    }

    // ── soft-delete operations ────────────────────────────────────────────────

    /// Soft-delete the row with the given primary key (`UPDATE SET deleted_at = NOW()`).
    fn soft_delete_by_pk(
        pool: &PgPool,
        id: impl Into<SqlValue> + Send,
    ) -> impl std::future::Future<Output = Result<u64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        let col = Self::soft_delete_column().unwrap_or("deleted_at");
        executor::soft_delete::<Self>(pool, Self::find(id), col)
    }

    /// Restore a soft-deleted row by primary key (`UPDATE SET deleted_at = NULL`).
    fn restore_by_pk(
        pool: &PgPool,
        id: impl Into<SqlValue> + Send,
    ) -> impl std::future::Future<Output = Result<u64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        let col = Self::soft_delete_column().unwrap_or("deleted_at");
        let builder = Self::query().where_eq(Self::primary_key(), id);
        executor::restore::<Self>(pool, builder, col)
    }

    /// Update `updated_at = NOW()` for the row with the given primary key (touch).
    fn touch_by_pk(
        pool: &PgPool,
        id: impl Into<SqlValue> + Send,
    ) -> impl std::future::Future<Output = Result<u64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        executor::touch::<Self>(pool, Self::find(id))
    }

    /// Soft-delete rows matching a custom query.
    fn soft_delete_where(
        pool: &PgPool,
        builder: QueryBuilder<Self>,
    ) -> impl std::future::Future<Output = Result<u64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        let col = Self::soft_delete_column().unwrap_or("deleted_at");
        executor::soft_delete::<Self>(pool, builder, col)
    }

    // ── upsert helpers ────────────────────────────────────────────────────────

    /// Find the first row matching `find` attrs, or INSERT one combining `find` + `create` attrs.
    ///
    /// Uses a transaction with `SELECT … FOR UPDATE` to prevent duplicate inserts.
    fn first_or_create(
        pool: &PgPool,
        find: &[(&str, SqlValue)],
        create: &[(&str, SqlValue)],
    ) -> impl std::future::Future<Output = Result<Self, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        let pool = pool.clone();
        let find_owned: Vec<(String, SqlValue)> = find
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        let create_owned: Vec<(String, SqlValue)> = create
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        async move { do_first_or_create::<Self>(&pool, find_owned, create_owned).await }
    }

    // ── LISTEN / NOTIFY ───────────────────────────────────────────────────────

    /// Subscribe to PostgreSQL `NOTIFY` events on this model's channel.
    ///
    /// The channel name defaults to the model's table name.  Useful for
    /// reactive patterns where a trigger calls `pg_notify(table_name, payload)`.
    fn subscribe(
        pool: &PgPool,
    ) -> impl std::future::Future<Output = Result<sqlx::postgres::PgListener, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        let pool = pool.clone();
        let channel = Self::table_name();
        async move {
            let mut listener = sqlx::postgres::PgListener::connect_with(&pool).await?;
            listener.listen(channel).await?;
            Ok(listener)
        }
    }

    /// Subscribe to a custom PostgreSQL channel (not tied to this model's table).
    fn subscribe_channel(
        pool: &PgPool,
        channel: &str,
    ) -> impl std::future::Future<Output = Result<sqlx::postgres::PgListener, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        let pool = pool.clone();
        let channel = channel.to_string();
        async move {
            let mut listener = sqlx::postgres::PgListener::connect_with(&pool).await?;
            listener.listen(&channel).await?;
            Ok(listener)
        }
    }

    /// Find the first row matching `find` attrs and UPDATE it with `update` attrs, or INSERT
    /// a new row combining both.
    ///
    /// Uses a transaction with `SELECT … FOR UPDATE` to prevent races.
    fn update_or_create(
        pool: &PgPool,
        find: &[(&str, SqlValue)],
        update: &[(&str, SqlValue)],
    ) -> impl std::future::Future<Output = Result<Self, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        let pool = pool.clone();
        let find_owned: Vec<(String, SqlValue)> = find
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        let update_owned: Vec<(String, SqlValue)> = update
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        async move { do_update_or_create::<Self>(&pool, find_owned, update_owned).await }
    }
}

/// Blanket implementation — every `Model + FromRow + Send + Unpin` gets
/// `PgModel` for free.
impl<T> PgModel for T where T: Model + for<'r> sqlx::FromRow<'r, PgRow> + Send + Unpin + 'static {}

// ── transaction helpers ───────────────────────────────────────────────────────

async fn do_first_or_create<T>(
    pool: &PgPool,
    find: Vec<(String, SqlValue)>,
    create: Vec<(String, SqlValue)>,
) -> Result<T, sqlx::Error>
where
    T: Model + for<'r> sqlx::FromRow<'r, PgRow> + Send + Unpin,
{
    let mut tx = pool.begin().await?;

    let mut qb: QueryBuilder<T> = QueryBuilder::new(T::table_name());
    for (col, val) in &find {
        qb = qb.where_eq(col.as_str(), val.clone());
    }
    let (base_sql, params) = qb.limit(1).to_sql();
    let sql_locked = format!("{base_sql} FOR UPDATE");

    let existing = sqlx_pg::build_query_as::<T>(&sql_locked, params)
        .fetch_optional(&mut *tx)
        .await?;

    if let Some(row) = existing {
        tx.commit().await?;
        return Ok(row);
    }

    let all_data: Vec<(&str, SqlValue)> = find
        .iter()
        .chain(create.iter())
        .map(|(k, v)| (k.as_str(), v.clone()))
        .collect();

    let (insert_sql, insert_params) = QueryBuilder::<T>::insert_sql(T::table_name(), &all_data);
    let sql_returning = format!("{insert_sql} RETURNING *");

    let new_row = sqlx_pg::build_query_as::<T>(&sql_returning, insert_params)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(new_row)
}

async fn do_update_or_create<T>(
    pool: &PgPool,
    find: Vec<(String, SqlValue)>,
    update: Vec<(String, SqlValue)>,
) -> Result<T, sqlx::Error>
where
    T: Model + for<'r> sqlx::FromRow<'r, PgRow> + Send + Unpin,
{
    let mut tx = pool.begin().await?;

    let mut qb: QueryBuilder<T> = QueryBuilder::new(T::table_name());
    for (col, val) in &find {
        qb = qb.where_eq(col.as_str(), val.clone());
    }
    let (base_sql, params) = qb.limit(1).to_sql();
    let sql_locked = format!("{base_sql} FOR UPDATE");

    let existing = sqlx_pg::build_query_as::<T>(&sql_locked, params)
        .fetch_optional(&mut *tx)
        .await?;

    if let Some(existing_row) = existing {
        let pk_val = T::pk_value(&existing_row);
        let update_data: Vec<(&str, SqlValue)> = update
            .iter()
            .map(|(k, v)| (k.as_str(), v.clone()))
            .collect();
        let (update_sql, update_params) = QueryBuilder::<T>::new(T::table_name())
            .where_eq(T::primary_key(), pk_val.clone())
            .to_update_sql(&update_data);
        sqlx_pg::build_query(&update_sql, update_params)
            .execute(&mut *tx)
            .await?;

        let (select_sql, select_params) = QueryBuilder::<T>::new(T::table_name())
            .where_eq(T::primary_key(), pk_val)
            .limit(1)
            .to_sql();
        let updated_row = sqlx_pg::build_query_as::<T>(&select_sql, select_params)
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?;
        return Ok(updated_row);
    }

    let all_data: Vec<(&str, SqlValue)> = find
        .iter()
        .chain(update.iter())
        .map(|(k, v)| (k.as_str(), v.clone()))
        .collect();

    let (insert_sql, insert_params) = QueryBuilder::<T>::insert_sql(T::table_name(), &all_data);
    let sql_returning = format!("{insert_sql} RETURNING *");

    let new_row = sqlx_pg::build_query_as::<T>(&sql_returning, insert_params)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(new_row)
}
