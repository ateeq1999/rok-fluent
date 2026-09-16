//! CRUD via the typed DSL builders — `db::insert_into`, `db::update`,
//! `db::delete_from`, upsert via `.on_conflict()`, and `.returning()`.
//!
//! Requires a live PostgreSQL database, unlike most other examples in this
//! folder:
//!
//! ```sh
//! DATABASE_URL=postgres://user:pass@localhost/rok_fluent_db \
//!   cargo run --example 02_crud --features postgres,query
//! ```
//!
//! If `DATABASE_URL` is not set, this connects to
//! `postgres://postgres:postgres@localhost:5432/rok_fluent_db`.
//!
//! # Deviation from the plan
//!
//! The plan asked for this example to default to SQLite. That is not
//! possible here: `db::insert_into` / `db::update` / `db::delete_from` and
//! their `.execute()` / `.fetch_one()` / `.returning()` terminals are only
//! implemented for PostgreSQL (`#[cfg(feature = "postgres")]` in
//! `src/dsl/insert.rs`, `update.rs`, `delete.rs`). There is currently no
//! SQLite executor for the typed DSL, so any example exercising these
//! builders end-to-end must run against PostgreSQL. See
//! `examples/01_quickstart.rs` for a zero-setup SQLite example using Active
//! Record instead.
//!
//! Also note: `README.md`'s upsert snippet (`.on_conflict(col).do_update([...])`)
//! does not match the real `InsertBuilder` API. There is no `.do_update()`
//! method — the real chain is `.on_conflict([cols])` followed by either
//! `.do_update_excluded([cols])` (`SET col = EXCLUDED.col`) or
//! `.do_update_values([(col, val), ...])` (`SET col = $N`), as used below.

use rok_fluent::dsl::db;

#[derive(Debug, sqlx::FromRow, rok_fluent::TableDerive)]
#[table(name = "products")]
pub struct Product {
    pub id: i64,
    pub sku: String,
    pub name: String,
    pub price_cents: i64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/rok_fluent_db".into());
    let pool = sqlx::PgPool::connect(&database_url).await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS products (
            id          BIGSERIAL PRIMARY KEY,
            sku         TEXT NOT NULL UNIQUE,
            name        TEXT NOT NULL,
            price_cents BIGINT NOT NULL
        )",
    )
    .execute(&pool)
    .await?;

    // ── Insert + RETURNING * ────────────────────────────────────────────────
    let created: Product = db::insert_into(Product::table())
        .values([("sku", "WIDGET-1"), ("name", "Widget")])
        .returning()
        .fetch_one::<Product>(&pool)
        .await?;
    println!("Inserted: {created:?}");

    // price_cents needs its own insert since `.values()` here takes `&str`
    // values; use `.execute()` with a plain UPDATE for non-string columns.
    db::update(Product::table())
        .set("price_cents", 1999_i64)
        .where_(Product::ID.eq(created.id))
        .execute(&pool)
        .await?;

    // ── Upsert — INSERT … ON CONFLICT (sku) DO UPDATE ──────────────────────
    let upserted: Product = db::insert_into(Product::table())
        .values_typed([
            (Product::SKU, "WIDGET-1".to_string()),
            (Product::NAME, "Widget (v2)".to_string()),
        ])
        .on_conflict([Product::SKU])
        .do_update_excluded([Product::NAME])
        .returning()
        .fetch_one::<Product>(&pool)
        .await?;
    println!("Upserted: {upserted:?}");

    // ── Update ──────────────────────────────────────────────────────────────
    let updated_rows = db::update(Product::table())
        .set_col(Product::PRICE_CENTS, 2499_i64)
        .where_(Product::SKU.eq("WIDGET-1"))
        .execute(&pool)
        .await?;
    println!("Rows updated: {updated_rows}");

    // ── Select ──────────────────────────────────────────────────────────────
    let all: Vec<Product> = db::select()
        .from(Product::table())
        .where_(Product::PRICE_CENTS.gt(0_i64))
        .order_by(Product::NAME.asc())
        .fetch_all(&pool)
        .await?;
    println!("All products: {all:?}");

    // ── Delete ──────────────────────────────────────────────────────────────
    let deleted_rows = db::delete_from(Product::table())
        .where_(Product::SKU.eq("WIDGET-1"))
        .execute(&pool)
        .await?;
    println!("Rows deleted: {deleted_rows}");

    Ok(())
}
