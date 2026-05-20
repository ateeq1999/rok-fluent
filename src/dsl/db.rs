//! Entry-point functions for the Drizzle-style query DSL.
//!
//! Import everything with `use rok_fluent::dsl::db;` then build queries with
//! `db::select()`, `db::insert_into(table)`, `db::update(table)`,
//! `db::delete_from(table)`.

use super::{
    delete::DeleteBuilder, insert::InsertBuilder, select::SelectBuilder, table::Table,
    update::UpdateBuilder,
};

/// Start a `SELECT` query.
///
/// ```rust,ignore
/// use rok_fluent::dsl::db;
///
/// let users = db::select()
///     .from(users::table)
///     .where_(users::active.eq(true))
///     .order_by(users::created_at.desc())
///     .limit(20)
///     .fetch_all::<User>(&pool)
///     .await?;
/// ```
pub fn select() -> SelectBuilder {
    SelectBuilder::new()
}

/// Start an `INSERT INTO table` query.
///
/// ```rust,ignore
/// use rok_fluent::dsl::db;
///
/// let user = db::insert_into(users::table)
///     .values([("name", "Alice"), ("email", "alice@x.com")])
///     .returning()
///     .fetch_one::<User>(&pool)
///     .await?;
/// ```
pub fn insert_into<T: Table>(table: T) -> InsertBuilder {
    InsertBuilder::new(table)
}

/// Start an `UPDATE table` query.
///
/// ```rust,ignore
/// use rok_fluent::dsl::db;
///
/// db::update(users::table)
///     .set("name", "Bob")
///     .where_(users::id.eq(1_i64))
///     .execute(&pool)
///     .await?;
/// ```
pub fn update<T: Table>(table: T) -> UpdateBuilder {
    UpdateBuilder::new(table)
}

/// Start a `DELETE FROM table` query.
///
/// ```rust,ignore
/// use rok_fluent::dsl::db;
///
/// db::delete_from(users::table)
///     .where_(users::deleted_at.is_not_null())
///     .execute(&pool)
///     .await?;
/// ```
pub fn delete_from<T: Table>(table: T) -> DeleteBuilder {
    DeleteBuilder::new(table)
}
