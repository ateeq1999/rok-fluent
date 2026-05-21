//! Benchmarks: SQL query build time for Active Record and DSL builders.
//!
//! Run: `cargo bench --bench query_build --all-features`

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use rok_fluent::core::condition::SqlValue;
use rok_fluent::core::query::Dialect;
use rok_fluent::dsl::db;
use rok_fluent::Model;

// ── Model for benchmarks ────────────────────────────────────────────────────

struct BenchModel;

impl Model for BenchModel {
    fn table_name() -> &'static str {
        "bench_items"
    }
    fn primary_key() -> &'static str {
        "id"
    }
    fn primary_keys() -> &'static [&'static str] {
        &["id"]
    }
    fn columns() -> &'static [&'static str] {
        &["id", "name", "value", "active", "category", "score"]
    }
    fn pk_value(&self) -> SqlValue {
        SqlValue::Null
    }
}

// ── DSL table ───────────────────────────────────────────────────────────────

struct BenchTable;

impl rok_fluent::dsl::Table for BenchTable {
    fn table_name() -> &'static str {
        "bench_items"
    }
}

// ── Criterion group ─────────────────────────────────────────────────────────

fn bench_active_record_simple(c: &mut Criterion) {
    c.bench_function("ar_simple_select", |b| {
        b.iter(|| {
            let q = BenchModel::query()
                .where_eq(black_box("active"), black_box(true))
                .limit(black_box(25));
            let (_sql, _params) = q.to_sql_with_dialect(Dialect::Postgres);
            black_box((_sql, _params))
        })
    });
}

fn bench_active_record_complex(c: &mut Criterion) {
    c.bench_function("ar_complex_query", |b| {
        b.iter(|| {
            let q = BenchModel::query()
                .where_eq("active", true)
                .where_gt("score", 100_i64)
                .where_like("name", "%test%")
                .where_in(
                    "category",
                    vec![
                        SqlValue::from("a"),
                        SqlValue::from("b"),
                        SqlValue::from("c"),
                    ],
                )
                .order_by_desc("score")
                .order_by("name")
                .limit(50)
                .offset(10)
                .distinct();
            let (_sql, _params) = q.to_sql_with_dialect(Dialect::Postgres);
            black_box((_sql, _params))
        })
    });
}

fn bench_active_record_bare(c: &mut Criterion) {
    c.bench_function("ar_bare_query", |b| {
        b.iter(|| {
            let q = BenchModel::query();
            let (_sql, _params) = q.to_sql_with_dialect(Dialect::Postgres);
            black_box((_sql, _params))
        })
    });
}

fn bench_dsl_select(c: &mut Criterion) {
    c.bench_function("dsl_select", |b| {
        b.iter(|| {
            let sb = db::select()
                .from_table_name("bench_items")
                .columns(["\"id\"", "\"name\"", "\"score\""])
                .limit(black_box(25));
            let (_sql, _params) = sb.to_sql_pg();
            black_box((_sql, _params))
        })
    });
}

fn bench_dsl_insert(c: &mut Criterion) {
    c.bench_function("dsl_insert", |b| {
        b.iter(|| {
            let ib = db::insert_into(BenchTable).values([("name", "test"), ("value", "42")]);
            let (_sql, _params) = ib.to_sql_pg();
            black_box((_sql, _params))
        })
    });
}

fn bench_dsl_update(c: &mut Criterion) {
    c.bench_function("dsl_update", |b| {
        b.iter(|| {
            let ub = db::update(BenchTable).set("name", "updated");
            let (_sql, _params) = ub.to_sql_pg();
            black_box((_sql, _params))
        })
    });
}

fn bench_dsl_delete(c: &mut Criterion) {
    c.bench_function("dsl_delete", |b| {
        b.iter(|| {
            let db_ = db::delete_from(BenchTable);
            let (_sql, _params) = db_.to_sql_pg();
            black_box((_sql, _params))
        })
    });
}

criterion_group!(
    benches,
    bench_active_record_simple,
    bench_active_record_complex,
    bench_active_record_bare,
    bench_dsl_select,
    bench_dsl_insert,
    bench_dsl_update,
    bench_dsl_delete,
);
criterion_main!(benches);
