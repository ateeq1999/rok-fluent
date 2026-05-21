//! Bridge between Active Record `ModelQuery` and the typed DSL `SelectBuilder`.
//!
//! Enabled when both `active` and `query` features are active.

use crate::core::condition::{Condition, JoinOp, OrderDir as CoreOrderDir, SqlValue};
use crate::core::model::Model;
use crate::core::query::QueryBuilder;
use crate::dsl::column::{NullsOrder, OrderDir as DslOrderDir, OrderExpr};
use crate::dsl::expr::Expr;
use crate::dsl::SelectBuilder;

/// Convert a DSL [`Expr`] into a [`Condition`] compatible with [`QueryBuilder`].
///
/// Parameterised variants map directly; structural variants (`And`, `Or`) recurse into
/// groups; non-parameterisable variants (`Not`, `Case`, `AggCmp`, `ColEq`) fall back to
/// literal SQL via [`Condition::Raw`].
pub fn expr_to_condition(expr: Expr) -> Condition {
    match expr {
        Expr::Eq(col, val) => Condition::Eq(col, val),
        Expr::Ne(col, val) => Condition::Ne(col, val),
        Expr::Gt(col, val) => Condition::Gt(col, val),
        Expr::Gte(col, val) => Condition::Gte(col, val),
        Expr::Lt(col, val) => Condition::Lt(col, val),
        Expr::Lte(col, val) => Condition::Lte(col, val),
        Expr::Like(col, val) => Condition::Like(col, sql_value_to_str(val)),
        Expr::NotLike(col, val) => Condition::NotLike(col, sql_value_to_str(val)),
        Expr::ILike(col, val) => Condition::ILike(col, sql_value_to_str(val)),
        Expr::In(col, vals) => Condition::In(col, vals),
        Expr::NotIn(col, vals) => Condition::NotIn(col, vals),
        Expr::IsNull(col) => Condition::IsNull(col),
        Expr::IsNotNull(col) => Condition::IsNotNull(col),
        Expr::Between(col, lo, hi) => Condition::Between(col, lo, hi),
        Expr::NotBetween(col, lo, hi) => Condition::NotBetween(col, lo, hi),
        // AND-tree: render as a grouped sub-condition
        Expr::And(l, r) => Condition::Group(vec![
            (JoinOp::And, expr_to_condition(*l)),
            (JoinOp::And, expr_to_condition(*r)),
        ]),
        // OR-tree: render as a grouped sub-condition
        Expr::Or(l, r) => Condition::Group(vec![
            (JoinOp::And, expr_to_condition(*l)),
            (JoinOp::Or, expr_to_condition(*r)),
        ]),
        // Non-parameterisable: render with literal values so $N offsets are never wrong
        Expr::Not(inner) => Condition::Raw(format!("NOT ({})", expr_to_literal(&inner))),
        Expr::Raw(sql) => Condition::Raw(sql),
        Expr::Exists(sql) => Condition::Raw(format!("EXISTS ({sql})")),
        Expr::NotExists(sql) => Condition::Raw(format!("NOT EXISTS ({sql})")),
        Expr::InSubquery(col, sql) => Condition::Raw(format!("{col} IN ({sql})")),
        Expr::NotInSubquery(col, sql) => Condition::Raw(format!("{col} NOT IN ({sql})")),
        Expr::ColEq(a, b) => Condition::Raw(format!("{a} = {b}")),
        Expr::AggCmp(agg, op, val) => {
            Condition::Raw(format!("{agg} {op} {}", val.to_sql_literal()))
        }
        Expr::Case(case_expr) => {
            // CaseExpr has no direct Condition equivalent; render as raw literal SQL
            Condition::Raw(case_expr_to_literal(&case_expr))
        }
        // Exhaustive — Expr is #[non_exhaustive], so a wildcard handles future variants
        #[allow(unreachable_patterns)]
        _ => Condition::Raw("1=1".into()),
    }
}

/// Convert a `QueryBuilder<M>` (post-scopes, post-soft-delete) into a [`SelectBuilder`].
///
/// Simple conditions (Eq, Ne, In, etc.) are converted to typed DSL [`Expr`]s.
/// Complex conditions (Subquery, FullText, Group) fall back to [`Expr::Raw`] via
/// literal SQL rendering.
pub fn model_query_into_select<M: Model>(builder: QueryBuilder<M>) -> SelectBuilder {
    let mut sel = SelectBuilder::new().from_table_name(builder.table_name_str());

    for (op, cond) in builder.conditions() {
        let expr = condition_to_expr(cond.clone());
        match op {
            JoinOp::And => sel = sel.where_(expr),
            JoinOp::Or => sel = sel.or_where(expr),
        }
    }

    for (col, dir) in builder.order_clauses() {
        let dsl_dir = match dir {
            CoreOrderDir::Asc => DslOrderDir::Asc,
            CoreOrderDir::Desc => DslOrderDir::Desc,
        };
        let ord = OrderExpr {
            col: col.clone(),
            dir: dsl_dir,
            nulls: NullsOrder::Default,
        };
        sel = sel.order_by(ord);
    }

    if let Some(n) = builder.limit_value() {
        sel = sel.limit(n as u64);
    }
    if let Some(n) = builder.offset_value() {
        sel = sel.offset(n as u64);
    }

    sel
}

// ── internal helpers ──────────────────────────────────────────────────────────

fn condition_to_expr(cond: Condition) -> Expr {
    match cond {
        Condition::Eq(col, val) => Expr::Eq(col, val),
        Condition::Ne(col, val) => Expr::Ne(col, val),
        Condition::Gt(col, val) => Expr::Gt(col, val),
        Condition::Gte(col, val) => Expr::Gte(col, val),
        Condition::Lt(col, val) => Expr::Lt(col, val),
        Condition::Lte(col, val) => Expr::Lte(col, val),
        Condition::Like(col, pat) => Expr::Like(col, SqlValue::Text(pat)),
        Condition::NotLike(col, pat) => Expr::NotLike(col, SqlValue::Text(pat)),
        Condition::ILike(col, pat) => Expr::ILike(col, SqlValue::Text(pat)),
        Condition::In(col, vals) => Expr::In(col, vals),
        Condition::NotIn(col, vals) => Expr::NotIn(col, vals),
        Condition::IsNull(col) => Expr::IsNull(col),
        Condition::IsNotNull(col) => Expr::IsNotNull(col),
        Condition::Between(col, lo, hi) => Expr::Between(col, lo, hi),
        Condition::NotBetween(col, lo, hi) => Expr::NotBetween(col, lo, hi),
        Condition::Raw(sql) => Expr::Raw(sql),
        Condition::Group(inner) => {
            let pairs: Vec<(JoinOp, Expr)> = inner
                .into_iter()
                .map(|(op, c)| (op, condition_to_expr(c)))
                .collect();
            group_to_expr(pairs)
        }
        // Complex conditions that have no direct DSL equivalent: fall back to literal SQL
        other => Expr::Raw(other.to_literal_sql()),
    }
}

fn group_to_expr(mut pairs: Vec<(JoinOp, Expr)>) -> Expr {
    if pairs.is_empty() {
        return Expr::Raw("1=1".into());
    }
    let (_, first) = pairs.remove(0);
    pairs.into_iter().fold(first, |acc, (op, expr)| match op {
        JoinOp::And => acc.and(expr),
        JoinOp::Or => acc.or(expr),
    })
}

fn sql_value_to_str(v: SqlValue) -> String {
    match v {
        SqlValue::Text(s) => s,
        other => other.to_sql_literal(),
    }
}

/// Render an `Expr` to literal SQL by inlining bound values.
///
/// Used only for `Expr::Not(inner)` where no parameterised rendering is possible.
fn expr_to_literal(expr: &Expr) -> String {
    let (sql, params) = expr.to_sql_pg(1);
    // Substitute $N placeholders back from highest to lowest to avoid partial matches
    // (e.g. replacing $1 before $10 would corrupt $10 → '...'0).
    let mut result = sql;
    for i in (0..params.len()).rev() {
        result = result.replace(&format!("${}", i + 1), &params[i].to_sql_literal());
    }
    result
}

/// Render a [`CaseExpr`] to literal SQL by inlining bound values.
fn case_expr_to_literal(case_expr: &crate::dsl::expr::CaseExpr) -> String {
    let (sql, params) = case_expr.render(1, '$');
    let mut result = sql;
    for i in (0..params.len()).rev() {
        result = result.replace(&format!("${}", i + 1), &params[i].to_sql_literal());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::condition::SqlValue;
    use crate::core::model::Model;
    use crate::core::query::QueryBuilder;
    use crate::dsl::expr::Expr;

    // Minimal stub model for QueryBuilder tests.
    struct Stub;
    impl Model for Stub {
        fn table_name() -> &'static str {
            "posts"
        }
        fn columns() -> &'static [&'static str] {
            &["id", "user_id", "published"]
        }
    }

    #[test]
    fn expr_eq_roundtrip() {
        let expr = Expr::Eq("user_id".into(), SqlValue::Integer(42));
        let cond = expr_to_condition(expr);
        let (sql, params) = cond.to_param_sql(1);
        assert_eq!(sql, "user_id = $1");
        assert_eq!(params, vec![SqlValue::Integer(42)]);
    }

    #[test]
    fn expr_and_roundtrip() {
        let expr = Expr::Eq("user_id".into(), SqlValue::Integer(1))
            .and(Expr::Eq("published".into(), SqlValue::Bool(true)));
        let cond = expr_to_condition(expr);
        let (sql, params) = cond.to_param_sql(1);
        assert_eq!(sql, "(user_id = $1 AND published = $2)");
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn expr_or_roundtrip() {
        let expr = Expr::Eq("status".into(), SqlValue::Text("active".into()))
            .or(Expr::Eq("status".into(), SqlValue::Text("pending".into())));
        let cond = expr_to_condition(expr);
        let (sql, params) = cond.to_param_sql(1);
        assert_eq!(sql, "(status = $1 OR status = $2)");
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn expr_in_roundtrip() {
        let expr = Expr::In(
            "id".into(),
            vec![
                SqlValue::Integer(1),
                SqlValue::Integer(2),
                SqlValue::Integer(3),
            ],
        );
        let cond = expr_to_condition(expr);
        let (sql, params) = cond.to_param_sql(1);
        assert_eq!(sql, "id IN ($1, $2, $3)");
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn into_dsl_preserves_table_name() {
        let builder: QueryBuilder<Stub> = QueryBuilder::new("posts");
        let sel = model_query_into_select(builder);
        let (sql, _) = sel.to_sql_pg();
        assert!(sql.contains("FROM \"posts\""), "sql: {sql}");
    }

    #[test]
    fn into_dsl_preserves_eq_condition() {
        let builder: QueryBuilder<Stub> = QueryBuilder::new("posts").where_eq("user_id", 7_i64);
        let sel = model_query_into_select(builder);
        let (sql, params) = sel.to_sql_pg();
        assert!(sql.contains("WHERE"), "sql: {sql}");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], SqlValue::Integer(7));
    }

    #[test]
    fn into_dsl_preserves_limit_offset() {
        let builder: QueryBuilder<Stub> = QueryBuilder::new("posts").limit(10).offset(20);
        let sel = model_query_into_select(builder);
        let (sql, _) = sel.to_sql_pg();
        assert!(sql.contains("LIMIT 10"), "sql: {sql}");
        assert!(sql.contains("OFFSET 20"), "sql: {sql}");
    }
}
