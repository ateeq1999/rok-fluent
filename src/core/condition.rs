//! SQL value and condition types.

use std::fmt;

/// A typed SQL parameter value.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum SqlValue {
    Text(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
    Null,
}

impl SqlValue {
    pub fn to_sql_literal(&self) -> String {
        match self {
            Self::Text(s) => format!("'{}'", s.replace('\'', "''")),
            Self::Integer(n) => n.to_string(),
            Self::Float(f) => f.to_string(),
            Self::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
            Self::Null => "NULL".to_string(),
        }
    }
}

impl fmt::Display for SqlValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_sql_literal())
    }
}

impl From<&str> for SqlValue {
    fn from(s: &str) -> Self {
        Self::Text(s.to_string())
    }
}
impl From<String> for SqlValue {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}
impl From<i8> for SqlValue {
    fn from(n: i8) -> Self {
        Self::Integer(n as i64)
    }
}
impl From<i16> for SqlValue {
    fn from(n: i16) -> Self {
        Self::Integer(n as i64)
    }
}
impl From<i32> for SqlValue {
    fn from(n: i32) -> Self {
        Self::Integer(n as i64)
    }
}
impl From<i64> for SqlValue {
    fn from(n: i64) -> Self {
        Self::Integer(n)
    }
}
impl From<u32> for SqlValue {
    fn from(n: u32) -> Self {
        Self::Integer(n as i64)
    }
}
impl From<u64> for SqlValue {
    fn from(n: u64) -> Self {
        Self::Integer(n as i64)
    }
}
impl From<f32> for SqlValue {
    fn from(f: f32) -> Self {
        Self::Float(f as f64)
    }
}
impl From<f64> for SqlValue {
    fn from(f: f64) -> Self {
        Self::Float(f)
    }
}
impl From<bool> for SqlValue {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}
impl<T: Into<SqlValue>> From<Option<T>> for SqlValue {
    fn from(opt: Option<T>) -> Self {
        match opt {
            Some(v) => v.into(),
            None => Self::Null,
        }
    }
}

/// The logical operator used to join a condition to the preceding clause.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinOp {
    And,
    Or,
}

impl fmt::Display for JoinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::And => write!(f, "AND"),
            Self::Or => write!(f, "OR"),
        }
    }
}

/// A single WHERE clause condition.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Condition {
    Eq(String, SqlValue),
    Ne(String, SqlValue),
    Gt(String, SqlValue),
    Gte(String, SqlValue),
    Lt(String, SqlValue),
    Lte(String, SqlValue),
    Like(String, String),
    NotLike(String, String),
    ILike(String, String),
    IsNull(String),
    IsNotNull(String),
    In(String, Vec<SqlValue>),
    NotIn(String, Vec<SqlValue>),
    Between(String, SqlValue, SqlValue),
    NotBetween(String, SqlValue, SqlValue),
    Raw(String),
    /// Grouped sub-conditions. Renders as `(cond1 AND cond2 OR …)`.
    Group(Vec<(JoinOp, Condition)>),
    /// JSONB text extraction: `col->>'key' = $N` (Postgres) or `json_extract(col,'$.key') = ?` (SQLite).
    JsonGet(String, String, SqlValue),
    /// `[NOT] EXISTS (SELECT 1 FROM table WHERE fk_expr [AND inner_conds])`.
    Subquery {
        exists: bool,
        table: String,
        fk_expr: String,
        inner: Vec<(JoinOp, Condition)>,
    },
    /// Full-text search match.
    FullTextMatch {
        col: String,
        query: String,
        config: Option<String>,
    },
    /// Full-text search rank expression.
    FullTextRank {
        col: String,
        query: String,
        config: Option<String>,
        alias: String,
    },
}

impl Condition {
    /// Render with positional `$N` placeholders (PostgreSQL dialect).
    pub fn to_param_sql(&self, offset: usize) -> (String, Vec<SqlValue>) {
        match self {
            Self::Eq(col, v) => (format!("{col} = ${offset}"), vec![v.clone()]),
            Self::Ne(col, v) => (format!("{col} != ${offset}"), vec![v.clone()]),
            Self::Gt(col, v) => (format!("{col} > ${offset}"), vec![v.clone()]),
            Self::Gte(col, v) => (format!("{col} >= ${offset}"), vec![v.clone()]),
            Self::Lt(col, v) => (format!("{col} < ${offset}"), vec![v.clone()]),
            Self::Lte(col, v) => (format!("{col} <= ${offset}"), vec![v.clone()]),
            Self::Like(col, pat) => (
                format!("{col} LIKE ${offset}"),
                vec![SqlValue::Text(pat.clone())],
            ),
            Self::NotLike(col, pat) => (
                format!("{col} NOT LIKE ${offset}"),
                vec![SqlValue::Text(pat.clone())],
            ),
            Self::ILike(col, pat) => (
                format!("{col} ILIKE ${offset}"),
                vec![SqlValue::Text(pat.clone())],
            ),
            Self::IsNull(col) => (format!("{col} IS NULL"), vec![]),
            Self::IsNotNull(col) => (format!("{col} IS NOT NULL"), vec![]),
            Self::In(col, vals) => {
                if vals.is_empty() {
                    return ("1=0".into(), vec![]);
                }
                let placeholders: Vec<String> = vals
                    .iter()
                    .enumerate()
                    .map(|(i, _)| format!("${}", offset + i))
                    .collect();
                (
                    format!("{col} IN ({})", placeholders.join(", ")),
                    vals.clone(),
                )
            }
            Self::NotIn(col, vals) => {
                if vals.is_empty() {
                    return ("1=1".into(), vec![]);
                }
                let placeholders: Vec<String> = vals
                    .iter()
                    .enumerate()
                    .map(|(i, _)| format!("${}", offset + i))
                    .collect();
                (
                    format!("{col} NOT IN ({})", placeholders.join(", ")),
                    vals.clone(),
                )
            }
            Self::Between(col, lo, hi) => (
                format!("{col} BETWEEN ${offset} AND ${}", offset + 1),
                vec![lo.clone(), hi.clone()],
            ),
            Self::NotBetween(col, lo, hi) => (
                format!("{col} NOT BETWEEN ${offset} AND ${}", offset + 1),
                vec![lo.clone(), hi.clone()],
            ),
            Self::Raw(sql) => (sql.clone(), vec![]),
            Self::Group(inner) => {
                let mut parts: Vec<String> = Vec::new();
                let mut group_params: Vec<SqlValue> = Vec::new();
                for (idx, (op, cond)) in inner.iter().enumerate() {
                    let (frag, ps) = cond.to_param_sql(offset + group_params.len());
                    group_params.extend(ps);
                    if idx > 0 {
                        parts.push(format!("{op} {frag}"));
                    } else {
                        parts.push(frag);
                    }
                }
                (format!("({})", parts.join(" ")), group_params)
            }
            Self::JsonGet(col, key, val) => (
                format!("{col}->>'{}' = ${offset}", key.replace('\'', "''")),
                vec![val.clone()],
            ),
            Self::Subquery {
                exists,
                table,
                fk_expr,
                inner,
            } => {
                let kw = if *exists { "EXISTS" } else { "NOT EXISTS" };
                let mut sub_params: Vec<SqlValue> = Vec::new();
                let mut parts: Vec<String> = Vec::new();
                for (i, (op, cond)) in inner.iter().enumerate() {
                    let (frag, ps) = cond.to_param_sql(offset + sub_params.len());
                    sub_params.extend(ps);
                    parts.push(if i == 0 {
                        format!("AND {frag}")
                    } else {
                        format!("{op} {frag}")
                    });
                }
                let extra = if parts.is_empty() {
                    String::new()
                } else {
                    format!(" {}", parts.join(" "))
                };
                (
                    format!("{kw} (SELECT 1 FROM {table} WHERE {fk_expr}{extra})"),
                    sub_params,
                )
            }
            Self::FullTextMatch { col, query, config } => {
                let cfg = config
                    .as_ref()
                    .map(|c| format!("'{}', ", c.replace('\'', "''")))
                    .unwrap_or_default();
                (
                    format!("{col} @@ to_tsquery({cfg}${offset})"),
                    vec![SqlValue::Text(query.clone())],
                )
            }
            Self::FullTextRank {
                col,
                query,
                config,
                alias,
            } => {
                let cfg = config
                    .as_ref()
                    .map(|c| format!("'{}', ", c.replace('\'', "''")))
                    .unwrap_or_default();
                (
                    format!("ts_rank(to_tsvector({col}), to_tsquery({cfg}${offset})) AS {alias}"),
                    vec![SqlValue::Text(query.clone())],
                )
            }
        }
    }

    /// Render with anonymous `?` placeholders (SQLite / MySQL dialect).
    pub fn to_param_sql_sqlite(&self) -> (String, Vec<SqlValue>) {
        match self {
            Self::Eq(col, v) => (format!("{col} = ?"), vec![v.clone()]),
            Self::Ne(col, v) => (format!("{col} != ?"), vec![v.clone()]),
            Self::Gt(col, v) => (format!("{col} > ?"), vec![v.clone()]),
            Self::Gte(col, v) => (format!("{col} >= ?"), vec![v.clone()]),
            Self::Lt(col, v) => (format!("{col} < ?"), vec![v.clone()]),
            Self::Lte(col, v) => (format!("{col} <= ?"), vec![v.clone()]),
            Self::Like(col, pat) => (format!("{col} LIKE ?"), vec![SqlValue::Text(pat.clone())]),
            Self::NotLike(col, pat) => (
                format!("{col} NOT LIKE ?"),
                vec![SqlValue::Text(pat.clone())],
            ),
            Self::ILike(col, pat) => (format!("{col} ILIKE ?"), vec![SqlValue::Text(pat.clone())]),
            Self::IsNull(col) => (format!("{col} IS NULL"), vec![]),
            Self::IsNotNull(col) => (format!("{col} IS NOT NULL"), vec![]),
            Self::In(col, vals) => {
                if vals.is_empty() {
                    return ("1=0".into(), vec![]);
                }
                let ph = vals.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
                (format!("{col} IN ({ph})"), vals.clone())
            }
            Self::NotIn(col, vals) => {
                if vals.is_empty() {
                    return ("1=1".into(), vec![]);
                }
                let ph = vals.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
                (format!("{col} NOT IN ({ph})"), vals.clone())
            }
            Self::Between(col, lo, hi) => (
                format!("{col} BETWEEN ? AND ?"),
                vec![lo.clone(), hi.clone()],
            ),
            Self::NotBetween(col, lo, hi) => (
                format!("{col} NOT BETWEEN ? AND ?"),
                vec![lo.clone(), hi.clone()],
            ),
            Self::Raw(sql) => (sql.clone(), vec![]),
            Self::Group(inner) => {
                let mut parts: Vec<String> = Vec::new();
                let mut group_params: Vec<SqlValue> = Vec::new();
                for (idx, (op, cond)) in inner.iter().enumerate() {
                    let (frag, ps) = cond.to_param_sql_sqlite();
                    group_params.extend(ps);
                    if idx > 0 {
                        parts.push(format!("{op} {frag}"));
                    } else {
                        parts.push(frag);
                    }
                }
                (format!("({})", parts.join(" ")), group_params)
            }
            Self::JsonGet(col, key, val) => (
                format!("json_extract({col}, '$.{}') = ?", key.replace('\'', "''")),
                vec![val.clone()],
            ),
            Self::Subquery {
                exists,
                table,
                fk_expr,
                inner,
            } => {
                let kw = if *exists { "EXISTS" } else { "NOT EXISTS" };
                let mut sub_params: Vec<SqlValue> = Vec::new();
                let mut parts: Vec<String> = Vec::new();
                for (i, (op, cond)) in inner.iter().enumerate() {
                    let (frag, ps) = cond.to_param_sql_sqlite();
                    sub_params.extend(ps);
                    parts.push(if i == 0 {
                        format!("AND {frag}")
                    } else {
                        format!("{op} {frag}")
                    });
                }
                let extra = if parts.is_empty() {
                    String::new()
                } else {
                    format!(" {}", parts.join(" "))
                };
                (
                    format!("{kw} (SELECT 1 FROM {table} WHERE {fk_expr}{extra})"),
                    sub_params,
                )
            }
            Self::FullTextMatch {
                col,
                query,
                config: _,
            } => (
                format!("{col} MATCH ?"),
                vec![SqlValue::Text(query.clone())],
            ),
            Self::FullTextRank {
                col,
                query,
                config: _,
                alias,
            } => (
                format!("ts_rank(to_tsvector({col}), ?) AS {alias}"),
                vec![SqlValue::Text(query.clone())],
            ),
        }
    }

    pub fn to_literal_sql(&self) -> String {
        match self {
            Self::Eq(col, v) => format!("{col} = {v}"),
            Self::Ne(col, v) => format!("{col} != {v}"),
            Self::Gt(col, v) => format!("{col} > {v}"),
            Self::Gte(col, v) => format!("{col} >= {v}"),
            Self::Lt(col, v) => format!("{col} < {v}"),
            Self::Lte(col, v) => format!("{col} <= {v}"),
            Self::Like(col, p) => format!("{col} LIKE '{p}'"),
            Self::NotLike(col, p) => format!("{col} NOT LIKE '{p}'"),
            Self::ILike(col, p) => format!("{col} ILIKE '{p}'"),
            Self::IsNull(col) => format!("{col} IS NULL"),
            Self::IsNotNull(col) => format!("{col} IS NOT NULL"),
            Self::In(col, vals) => {
                if vals.is_empty() {
                    return "1=0".into();
                }
                let lits: Vec<String> = vals.iter().map(|v| v.to_sql_literal()).collect();
                format!("{col} IN ({})", lits.join(", "))
            }
            Self::NotIn(col, vals) => {
                if vals.is_empty() {
                    return "1=1".into();
                }
                let lits: Vec<String> = vals.iter().map(|v| v.to_sql_literal()).collect();
                format!("{col} NOT IN ({})", lits.join(", "))
            }
            Self::Between(col, lo, hi) => format!("{col} BETWEEN {lo} AND {hi}"),
            Self::NotBetween(col, lo, hi) => format!("{col} NOT BETWEEN {lo} AND {hi}"),
            Self::Raw(sql) => sql.clone(),
            Self::Group(inner) => {
                let parts: Vec<String> = inner
                    .iter()
                    .enumerate()
                    .map(|(idx, (op, cond))| {
                        let frag = cond.to_literal_sql();
                        if idx > 0 {
                            format!("{op} {frag}")
                        } else {
                            frag
                        }
                    })
                    .collect();
                format!("({})", parts.join(" "))
            }
            Self::JsonGet(col, key, val) => {
                format!("{col}->>'{}' = {val}", key.replace('\'', "''"))
            }
            Self::Subquery {
                exists,
                table,
                fk_expr,
                inner,
            } => {
                let kw = if *exists { "EXISTS" } else { "NOT EXISTS" };
                let parts: Vec<String> = inner
                    .iter()
                    .enumerate()
                    .map(|(i, (op, cond))| {
                        let frag = cond.to_literal_sql();
                        if i == 0 {
                            format!("AND {frag}")
                        } else {
                            format!("{op} {frag}")
                        }
                    })
                    .collect();
                let extra = if parts.is_empty() {
                    String::new()
                } else {
                    format!(" {}", parts.join(" "))
                };
                format!("{kw} (SELECT 1 FROM {table} WHERE {fk_expr}{extra})")
            }
            Self::FullTextMatch { col, query, config } => {
                let cfg = config
                    .as_ref()
                    .map(|c| format!("'{}', ", c.replace('\'', "''")))
                    .unwrap_or_default();
                format!("{col} @@ to_tsquery({cfg}'{}')", query.replace('\'', "''"))
            }
            Self::FullTextRank {
                col,
                query,
                config,
                alias,
            } => {
                let cfg = config
                    .as_ref()
                    .map(|c| format!("'{}', ", c.replace('\'', "''")))
                    .unwrap_or_default();
                format!(
                    "ts_rank(to_tsvector({col}), to_tsquery({cfg}'{}')) AS {alias}",
                    query.replace('\'', "''")
                )
            }
        }
    }
}

/// Sort direction for ORDER BY clauses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderDir {
    Asc,
    Desc,
}

impl fmt::Display for OrderDir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Asc => write!(f, "ASC"),
            Self::Desc => write!(f, "DESC"),
        }
    }
}
