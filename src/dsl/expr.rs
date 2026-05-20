//! [`Expr`] — composable boolean expression tree for DSL `WHERE` / `HAVING` clauses.

use crate::core::condition::SqlValue;

/// A boolean predicate in the DSL query language.
///
/// Built via [`Column`](super::column::Column) methods (`.eq()`, `.gt()`, etc.)
/// and composed with [`and`](Expr::and) / [`or`](Expr::or).
///
/// ```rust,ignore
/// let expr = User::ID.eq(1_i64)
///     .and(User::EMAIL.like("%@example.com"));
/// ```
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Expr {
    /// `col = val`
    Eq(String, SqlValue),
    /// `col != val`
    Ne(String, SqlValue),
    /// `col > val`
    Gt(String, SqlValue),
    /// `col >= val`
    Gte(String, SqlValue),
    /// `col < val`
    Lt(String, SqlValue),
    /// `col <= val`
    Lte(String, SqlValue),
    /// `col LIKE pattern`
    Like(String, SqlValue),
    /// `col NOT LIKE pattern`
    NotLike(String, SqlValue),
    /// `col ILIKE pattern` — PostgreSQL case-insensitive LIKE
    ILike(String, SqlValue),
    /// `col IN (...)`
    In(String, Vec<SqlValue>),
    /// `col NOT IN (...)`
    NotIn(String, Vec<SqlValue>),
    /// `col IS NULL`
    IsNull(String),
    /// `col IS NOT NULL`
    IsNotNull(String),
    /// `col BETWEEN lo AND hi`
    Between(String, SqlValue, SqlValue),
    /// `col NOT BETWEEN lo AND hi`
    NotBetween(String, SqlValue, SqlValue),
    /// `left_col = right_col` — column-to-column equality for JOIN ON clauses
    ColEq(String, String),
    /// Aggregate comparison for HAVING: `AGG_SQL op val` (e.g. `COUNT("t"."id") > $N`)
    AggCmp(String, &'static str, SqlValue),
    /// `left AND right`
    And(Box<Expr>, Box<Expr>),
    /// `left OR right`
    Or(Box<Expr>, Box<Expr>),
    /// `NOT expr`
    Not(Box<Expr>),
    /// Raw SQL fragment (no parameter binding).
    Raw(String),
}

impl std::ops::Not for Expr {
    type Output = Expr;

    /// Negate with `NOT expr`.
    fn not(self) -> Expr {
        Expr::Not(Box::new(self))
    }
}

impl Expr {
    /// Combine with `AND`.
    pub fn and(self, other: Expr) -> Expr {
        Expr::And(Box::new(self), Box::new(other))
    }

    /// Combine with `OR`.
    pub fn or(self, other: Expr) -> Expr {
        Expr::Or(Box::new(self), Box::new(other))
    }

    /// Raw SQL escape hatch — inserted verbatim with no parameter binding.
    ///
    /// Use only when no typed alternative exists.
    pub fn raw(sql: impl Into<String>) -> Expr {
        Expr::Raw(sql.into())
    }

    /// Render the expression to a parameterised SQL fragment.
    ///
    /// `offset` is the 1-based index of the first `$N` placeholder to emit
    /// (PostgreSQL style).  Returns `(sql_fragment, bound_values)`.
    pub fn to_sql_pg(&self, offset: usize) -> (String, Vec<SqlValue>) {
        self.render(offset, '$')
    }

    /// Render using `?` placeholders (MySQL / SQLite).
    pub fn to_sql_qmark(&self, offset: usize) -> (String, Vec<SqlValue>) {
        self.render(offset, '?')
    }

    fn render(&self, mut offset: usize, ph: char) -> (String, Vec<SqlValue>) {
        match self {
            Expr::Eq(col, v) => self.binary_op(col, "=", v, offset, ph),
            Expr::Ne(col, v) => self.binary_op(col, "!=", v, offset, ph),
            Expr::Gt(col, v) => self.binary_op(col, ">", v, offset, ph),
            Expr::Gte(col, v) => self.binary_op(col, ">=", v, offset, ph),
            Expr::Lt(col, v) => self.binary_op(col, "<", v, offset, ph),
            Expr::Lte(col, v) => self.binary_op(col, "<=", v, offset, ph),
            Expr::Like(col, v) => self.binary_op(col, "LIKE", v, offset, ph),
            Expr::NotLike(col, v) => self.binary_op(col, "NOT LIKE", v, offset, ph),
            Expr::ILike(col, v) => self.binary_op(col, "ILIKE", v, offset, ph),

            Expr::In(col, vals) => {
                let phs: Vec<String> = vals
                    .iter()
                    .enumerate()
                    .map(|(i, _)| {
                        if ph == '?' {
                            "?".into()
                        } else {
                            format!("${}", offset + i)
                        }
                    })
                    .collect();
                (format!("{col} IN ({})", phs.join(", ")), vals.clone())
            }

            Expr::NotIn(col, vals) => {
                let phs: Vec<String> = vals
                    .iter()
                    .enumerate()
                    .map(|(i, _)| {
                        if ph == '?' {
                            "?".into()
                        } else {
                            format!("${}", offset + i)
                        }
                    })
                    .collect();
                (format!("{col} NOT IN ({})", phs.join(", ")), vals.clone())
            }

            Expr::IsNull(col) => (format!("{col} IS NULL"), vec![]),
            Expr::IsNotNull(col) => (format!("{col} IS NOT NULL"), vec![]),

            Expr::Between(col, lo, hi) => {
                let lo_ph = if ph == '?' {
                    "?".into()
                } else {
                    format!("${offset}")
                };
                let hi_ph = if ph == '?' {
                    "?".into()
                } else {
                    format!("${}", offset + 1)
                };
                (
                    format!("{col} BETWEEN {lo_ph} AND {hi_ph}"),
                    vec![lo.clone(), hi.clone()],
                )
            }

            Expr::NotBetween(col, lo, hi) => {
                let lo_ph = if ph == '?' {
                    "?".into()
                } else {
                    format!("${offset}")
                };
                let hi_ph = if ph == '?' {
                    "?".into()
                } else {
                    format!("${}", offset + 1)
                };
                (
                    format!("{col} NOT BETWEEN {lo_ph} AND {hi_ph}"),
                    vec![lo.clone(), hi.clone()],
                )
            }

            Expr::ColEq(left, right) => (format!("{left} = {right}"), vec![]),

            Expr::AggCmp(agg_sql, op, val) => {
                let placeholder = if ph == '?' {
                    "?".into()
                } else {
                    format!("${offset}")
                };
                (format!("{agg_sql} {op} {placeholder}"), vec![val.clone()])
            }

            Expr::And(l, r) => {
                let (ls, lp) = l.render(offset, ph);
                offset += lp.len();
                let (rs, rp) = r.render(offset, ph);
                let mut params = lp;
                params.extend(rp);
                (format!("({ls} AND {rs})"), params)
            }

            Expr::Or(l, r) => {
                let (ls, lp) = l.render(offset, ph);
                offset += lp.len();
                let (rs, rp) = r.render(offset, ph);
                let mut params = lp;
                params.extend(rp);
                (format!("({ls} OR {rs})"), params)
            }

            Expr::Not(inner) => {
                let (s, p) = inner.render(offset, ph);
                (format!("NOT ({s})"), p)
            }

            Expr::Raw(sql) => (sql.clone(), vec![]),
        }
    }

    fn binary_op(
        &self,
        col: &str,
        op: &str,
        val: &SqlValue,
        offset: usize,
        ph: char,
    ) -> (String, Vec<SqlValue>) {
        let placeholder = if ph == '?' {
            "?".to_string()
        } else {
            format!("${offset}")
        };
        (format!("{col} {op} {placeholder}"), vec![val.clone()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::condition::SqlValue;

    #[test]
    fn eq_pg() {
        let e = Expr::Eq("\"users\".\"id\"".into(), SqlValue::Integer(1));
        let (sql, params) = e.to_sql_pg(1);
        assert_eq!(sql, "\"users\".\"id\" = $1");
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn and_increments_offset() {
        let e = Expr::Eq("\"users\".\"id\"".into(), SqlValue::Integer(1)).and(Expr::Eq(
            "\"users\".\"active\"".into(),
            SqlValue::Bool(true),
        ));
        let (sql, params) = e.to_sql_pg(1);
        assert_eq!(sql, "(\"users\".\"id\" = $1 AND \"users\".\"active\" = $2)");
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn in_pg() {
        let e = Expr::In(
            "\"users\".\"id\"".into(),
            vec![
                SqlValue::Integer(1),
                SqlValue::Integer(2),
                SqlValue::Integer(3),
            ],
        );
        let (sql, _) = e.to_sql_pg(1);
        assert_eq!(sql, "\"users\".\"id\" IN ($1, $2, $3)");
    }

    #[test]
    fn is_null() {
        let e = Expr::IsNull("\"posts\".\"deleted_at\"".into());
        let (sql, params) = e.to_sql_pg(1);
        assert_eq!(sql, "\"posts\".\"deleted_at\" IS NULL");
        assert!(params.is_empty());
    }

    #[test]
    fn between_pg() {
        let e = Expr::Between(
            "\"users\".\"age\"".into(),
            SqlValue::Integer(18),
            SqlValue::Integer(65),
        );
        let (sql, params) = e.to_sql_pg(1);
        assert_eq!(sql, "\"users\".\"age\" BETWEEN $1 AND $2");
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn col_eq_no_params() {
        let e = Expr::ColEq("\"posts\".\"user_id\"".into(), "\"users\".\"id\"".into());
        let (sql, params) = e.to_sql_pg(1);
        assert_eq!(sql, "\"posts\".\"user_id\" = \"users\".\"id\"");
        assert!(params.is_empty());
    }

    #[test]
    fn raw_no_params() {
        let e = Expr::raw("score > 100");
        let (sql, params) = e.to_sql_pg(1);
        assert_eq!(sql, "score > 100");
        assert!(params.is_empty());
    }
}
