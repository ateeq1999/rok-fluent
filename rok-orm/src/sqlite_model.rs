//! [`SqliteModel`] — ergonomic async CRUD methods for any [`Model`] + [`sqlx::FromRow`] type,
//! backed by SQLite.
//!
//! All methods are provided as defaults; no manual implementation is required.
//!
//! # Example
//!
//! ```rust,ignore
//! use rok_orm::{Model, sqlite_model::SqliteModel, SqlValue};
//!
//! #[derive(Model, sqlx::FromRow)]
//! pub struct Note {
//!     pub id:    i64,
//!     pub title: String,
//!     pub body:  String,
//! }
//!
//! let pool = sqlx::SqlitePool::connect("sqlite::memory:").await?;
//!
//! let all: Vec<Note>     = Note::all(&pool).await?;
//! let one: Option<Note>  = Note::find_by_pk(&pool, 1i64).await?;
//! let n:   i64           = Note::count(&pool).await?;
//! Note::create(&pool, &[("title", "Hello".into()), ("body", "World".into())]).await?;
//! Note::update_by_pk(&pool, 1i64, &[("title", "Updated".into())]).await?;
//! Note::delete_by_pk(&pool, 1i64).await?;
//! ```

use rok_orm_core::{Model, QueryBuilder, SqlValue};
use sqlx::{sqlite::SqliteRow, SqlitePool};

use crate::sqlite_executor;

/// Blanket async CRUD extension for any type that implements [`Model`] and
/// [`sqlx::FromRow<SqliteRow>`].
pub trait SqliteModel: Model + for<'r> sqlx::FromRow<'r, SqliteRow> + Send + Unpin {
    /// Fetch every row from the model's table.
    fn all(
        pool: &SqlitePool,
    ) -> impl std::future::Future<Output = Result<Vec<Self>, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        sqlite_executor::fetch_all(pool, Self::query())
    }

    /// Fetch rows matching a custom query.
    fn find_where(
        pool: &SqlitePool,
        builder: QueryBuilder<Self>,
    ) -> impl std::future::Future<Output = Result<Vec<Self>, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        sqlite_executor::fetch_all(pool, builder)
    }

    /// Fetch a single row by primary key.  Returns `None` if not found.
    fn find_by_pk(
        pool: &SqlitePool,
        id: impl Into<SqlValue> + Send,
    ) -> impl std::future::Future<Output = Result<Option<Self>, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        sqlite_executor::fetch_optional(pool, Self::find(id))
    }

    /// Return the total number of rows.
    fn count(
        pool: &SqlitePool,
    ) -> impl std::future::Future<Output = Result<i64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        sqlite_executor::count(pool, Self::query())
    }

    /// Return the number of rows matching a custom query.
    fn count_where(
        pool: &SqlitePool,
        builder: QueryBuilder<Self>,
    ) -> impl std::future::Future<Output = Result<i64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        sqlite_executor::count(pool, builder)
    }

    /// Insert a row and return rows affected.
    fn create(
        pool: &SqlitePool,
        data: &[(&str, SqlValue)],
    ) -> impl std::future::Future<Output = Result<u64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        sqlite_executor::insert::<Self>(pool, Self::table_name(), data)
    }

    /// Update by primary key and return rows affected.
    fn update_by_pk(
        pool: &SqlitePool,
        id: impl Into<SqlValue> + Send,
        data: &[(&str, SqlValue)],
    ) -> impl std::future::Future<Output = Result<u64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        let builder = Self::find(id);
        sqlite_executor::update::<Self>(pool, builder, data)
    }

    /// Delete by primary key and return rows affected.
    fn delete_by_pk(
        pool: &SqlitePool,
        id: impl Into<SqlValue> + Send,
    ) -> impl std::future::Future<Output = Result<u64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        sqlite_executor::delete(pool, Self::find(id))
    }

    /// Delete rows matching a custom query and return rows affected.
    fn delete_where(
        pool: &SqlitePool,
        builder: QueryBuilder<Self>,
    ) -> impl std::future::Future<Output = Result<u64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        sqlite_executor::delete(pool, builder)
    }

    /// Update rows matching a custom query and return rows affected.
    fn update_where(
        pool: &SqlitePool,
        builder: QueryBuilder<Self>,
        data: &[(&str, SqlValue)],
    ) -> impl std::future::Future<Output = Result<u64, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        sqlite_executor::update::<Self>(pool, builder, data)
    }

    /// Insert a row and return it via `RETURNING *` (requires SQLite 3.35+).
    fn create_returning(
        pool: &SqlitePool,
        data: &[(&str, SqlValue)],
    ) -> impl std::future::Future<Output = Result<Self, sqlx::Error>> + Send
    where
        Self: Sized,
    {
        sqlite_executor::insert_returning::<Self>(pool, Self::table_name(), data)
    }
}

/// Blanket implementation — every `Model + FromRow<SqliteRow> + Send + Unpin`
/// gets `SqliteModel` for free.
impl<T> SqliteModel for T where T: Model + for<'r> sqlx::FromRow<'r, SqliteRow> + Send + Unpin {}
