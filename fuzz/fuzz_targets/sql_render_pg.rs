#![no_main]

use libfuzzer_sys::fuzz_target;
use rok_fluent::core::condition::SqlValue;
use rok_fluent::core::query::Dialect;
use rok_fluent::dsl::db;
use rok_fluent::Model;

struct FuzzModel;

impl Model for FuzzModel {
    fn table_name() -> &'static str {
        "fuzz"
    }
    fn primary_key() -> &'static str {
        "id"
    }
    fn primary_keys() -> &'static [&'static str] {
        &["id"]
    }
    fn columns() -> &'static [&'static str] {
        &["id", "name", "value", "active"]
    }
    fn pk_value(&self) -> SqlValue {
        SqlValue::Null
    }
}

struct FuzzTable;
impl rok_fluent::dsl::Table for FuzzTable {
    fn table_name() -> &'static str {
        "fuzz"
    }
}

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    // ── Active Record QueryBuilder ───────────────────────────────────────
    let mut qb = FuzzModel::query();
    let cols = ["id", "name", "value", "active"];
    let col = cols[(data[0] as usize) % cols.len()];
    let val: SqlValue = 42_i64.into();

    match data.get(1).copied().unwrap_or(0) % 12 {
        0 => qb = qb.where_eq(col, val),
        1 => qb = qb.where_ne(col, val),
        2 => qb = qb.where_gt(col, val),
        3 => qb = qb.where_lt(col, val),
        4 => qb = qb.where_like(col, "%test%"),
        5 => qb = qb.where_null(col),
        6 => qb = qb.where_not_null(col),
        7 => qb = qb.order_by(col),
        8 => qb = qb.order_by_desc(col),
        9 => qb = qb.limit(10),
        10 => qb = qb.offset(5),
        11 => qb = qb.distinct(),
        _ => {}
    }
    if data.len() > 2 && data[2] % 2 == 0 {
        qb = qb.select(&["id", "name"]);
    }

    let (_sql, _params) = qb.to_sql_with_dialect(Dialect::Postgres);
    drop((_sql, _params));

    // ── DSL SelectBuilder ────────────────────────────────────────────────
    let mut sb = db::select().from_table_name("fuzz");
    match data.get(1).copied().unwrap_or(0) % 6 {
        0 => sb = sb.distinct(),
        1 => sb = sb.limit(10),
        2 => sb = sb.offset(5),
        3 => sb = sb.columns(["id", "name"]),
        _ => {}
    }
    let (_sql, _params) = sb.to_sql_pg();
    drop((_sql, _params));

    // ── DSL InsertBuilder ────────────────────────────────────────────────
    if data.len() > 3 && data[2] % 2 == 0 {
        let ib = db::insert_into(FuzzTable).values([("name", "test")]);
        let (_sql, _params) = ib.to_sql_pg();
        drop((_sql, _params));
    }

    // ── DSL UpdateBuilder ────────────────────────────────────────────────
    let mut ub = db::update(FuzzTable);
    if data.len() > 2 && data[1] % 2 == 0 {
        ub = ub.set("name", "updated");
    }
    let (_sql, _params) = ub.to_sql_pg();
    drop((_sql, _params));

    // ── DSL DeleteBuilder ────────────────────────────────────────────────
    let db_ = db::delete_from(FuzzTable);
    let (_sql, _params) = db_.to_sql_pg();
    drop((_sql, _params));
});
