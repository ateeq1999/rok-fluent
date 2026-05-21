#![no_main]

use libfuzzer_sys::fuzz_target;
use rok_fluent::core::condition::SqlValue;
use rok_fluent::core::query::Dialect;
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

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    // Active Record QueryBuilder with qmark dialect (SQLite / MySQL)
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

    let (_sql, _params) = qb.to_sql_with_dialect(Dialect::Sqlite);
    drop((_sql, _params));
});
