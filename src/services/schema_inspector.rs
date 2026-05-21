//! [`SchemaInspector`] — query PostgreSQL `information_schema` for column,
//! index, and foreign-key metadata.

/// Metadata about a single database column.
#[derive(Debug, Clone)]
pub struct ColumnInfo {
    /// Column name.
    pub name: String,
    /// SQL data type name (e.g. `BIGINT`, `VARCHAR`, `TIMESTAMP WITH TIME ZONE`).
    pub data_type: String,
    /// Maximum character length (`VARCHAR(255)` → `255`); `None` for non-character types.
    pub max_length: Option<i32>,
    /// Whether the column is nullable.
    pub nullable: bool,
    /// Default expression, if any.
    pub default: Option<String>,
    /// Whether the column is part of the primary key.
    pub is_pk: bool,
}

/// Metadata about a database index.
#[derive(Debug, Clone)]
pub struct IndexInfo {
    /// Index name.
    pub name: String,
    /// Column names in index order.
    pub columns: Vec<String>,
    /// Whether the index enforces uniqueness.
    pub unique: bool,
    /// Whether this index is the primary key index.
    pub primary: bool,
}

/// Metadata about a foreign-key constraint.
#[derive(Debug, Clone)]
pub struct ForeignKeyInfo {
    /// Constraint name.
    pub constraint_name: String,
    /// Local column names participating in the FK.
    pub columns: Vec<String>,
    /// Referenced table name.
    pub foreign_table: String,
    /// Referenced column names.
    pub foreign_columns: Vec<String>,
}

/// Inspect PostgreSQL schema metadata via `information_schema`.
///
/// All methods query the live database and return structured metadata.
///
/// # Example
///
/// ```rust,ignore
/// use rok_fluent::services::SchemaInspector;
///
/// let cols = SchemaInspector::columns("users", &pool).await?;
/// for col in &cols {
///     println!("{} {} (pk={})", col.name, col.data_type, col.is_pk);
/// }
/// ```
pub struct SchemaInspector;

impl SchemaInspector {
    /// Return metadata for every column in `table`.
    pub async fn columns(table: &str, pool: &sqlx::PgPool) -> Result<Vec<ColumnInfo>, sqlx::Error> {
        let pk_cols = Self::pk_columns(table, pool).await?;

        #[derive(sqlx::FromRow)]
        struct RawCol {
            column_name: String,
            udt_name: String,
            character_maximum_length: Option<i32>,
            is_nullable: String,
            column_default: Option<String>,
        }

        let raw: Vec<RawCol> = sqlx::query_as(
            "SELECT column_name, udt_name, character_maximum_length, \
                    is_nullable, column_default \
             FROM information_schema.columns \
             WHERE table_schema = 'public' AND table_name = $1 \
             ORDER BY ordinal_position",
        )
        .bind(table)
        .fetch_all(pool)
        .await?;

        Ok(raw
            .into_iter()
            .map(|r| {
                let is_pk = pk_cols.contains(&r.column_name);
                ColumnInfo {
                    name: r.column_name,
                    data_type: pg_type_str(&r.udt_name, r.character_maximum_length),
                    max_length: r.character_maximum_length,
                    nullable: r.is_nullable == "YES",
                    default: r.column_default,
                    is_pk,
                }
            })
            .collect())
    }

    /// Return metadata for every index on `table`.
    pub async fn indexes(table: &str, pool: &sqlx::PgPool) -> Result<Vec<IndexInfo>, sqlx::Error> {
        #[derive(sqlx::FromRow)]
        struct RawIdx {
            indexname: String,
            indexdef: String,
        }

        let raw: Vec<RawIdx> = sqlx::query_as(
            "SELECT indexname, indexdef FROM pg_indexes \
             WHERE tablename = $1 AND schemaname = 'public' \
             ORDER BY indexname",
        )
        .bind(table)
        .fetch_all(pool)
        .await?;

        Ok(raw
            .into_iter()
            .map(|r| {
                let unique = r.indexdef.contains(" UNIQUE ");
                let primary = r.indexdef.contains(" PRIMARY KEY ");
                let cols = parse_index_columns(&r.indexdef);
                IndexInfo {
                    name: r.indexname,
                    columns: cols,
                    unique,
                    primary,
                }
            })
            .collect())
    }

    /// Return every foreign-key constraint referencing or originating from
    /// `table`.
    pub async fn foreign_keys(
        table: &str,
        pool: &sqlx::PgPool,
    ) -> Result<Vec<ForeignKeyInfo>, sqlx::Error> {
        #[derive(sqlx::FromRow)]
        struct RawFk {
            constraint_name: String,
            column_name: String,
            foreign_table_name: String,
            foreign_column_name: String,
        }

        let raw: Vec<RawFk> = sqlx::query_as(
            "SELECT tc.constraint_name, \
                    kcu.column_name, \
                    ccu.table_name AS foreign_table_name, \
                    ccu.column_name AS foreign_column_name \
             FROM information_schema.table_constraints tc \
             JOIN information_schema.key_column_usage kcu \
               ON tc.constraint_name = kcu.constraint_name \
             JOIN information_schema.constraint_column_usage ccu \
               ON tc.constraint_name = ccu.constraint_name \
             WHERE tc.table_schema = 'public' \
               AND tc.table_name = $1 \
               AND tc.constraint_type = 'FOREIGN KEY' \
             ORDER BY tc.constraint_name, kcu.ordinal_position",
        )
        .bind(table)
        .fetch_all(pool)
        .await?;

        // Group by constraint_name
        let mut result: Vec<ForeignKeyInfo> = Vec::new();
        let mut current: Option<ForeignKeyInfo> = None;
        for r in raw {
            match &mut current {
                None => {
                    current = Some(ForeignKeyInfo {
                        constraint_name: r.constraint_name,
                        columns: vec![r.column_name],
                        foreign_table: r.foreign_table_name,
                        foreign_columns: vec![r.foreign_column_name],
                    });
                }
                Some(ref mut fk) if fk.constraint_name == r.constraint_name => {
                    fk.columns.push(r.column_name);
                    fk.foreign_columns.push(r.foreign_column_name);
                }
                _ => {
                    result.push(current.take().unwrap());
                    current = Some(ForeignKeyInfo {
                        constraint_name: r.constraint_name,
                        columns: vec![r.column_name],
                        foreign_table: r.foreign_table_name,
                        foreign_columns: vec![r.foreign_column_name],
                    });
                }
            }
        }
        if let Some(fk) = current {
            result.push(fk);
        }
        Ok(result)
    }

    // ── helpers ────────────────────────────────────────────────────────────

    async fn pk_columns(table: &str, pool: &sqlx::PgPool) -> Result<Vec<String>, sqlx::Error> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT kcu.column_name \
             FROM information_schema.table_constraints tc \
             JOIN information_schema.key_column_usage kcu \
               ON tc.constraint_name = kcu.constraint_name \
               AND tc.table_schema = kcu.table_schema \
             WHERE tc.table_schema = 'public' AND tc.table_name = $1 \
               AND tc.constraint_type = 'PRIMARY KEY' \
             ORDER BY kcu.ordinal_position",
        )
        .bind(table)
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().map(|(n,)| n).collect())
    }
}

fn pg_type_str(udt_name: &str, char_max: Option<i32>) -> String {
    match udt_name {
        "int8" | "bigserial" => "BIGINT".into(),
        "int4" | "serial" => "INTEGER".into(),
        "int2" | "smallserial" => "SMALLINT".into(),
        "float8" => "DOUBLE PRECISION".into(),
        "float4" => "REAL".into(),
        "bool" => "BOOLEAN".into(),
        "text" => "TEXT".into(),
        "varchar" => char_max.map_or("VARCHAR".into(), |n| format!("VARCHAR({n})")),
        "bpchar" => char_max.map_or("CHAR".into(), |n| format!("CHAR({n})")),
        "uuid" => "UUID".into(),
        "jsonb" => "JSONB".into(),
        "json" => "JSON".into(),
        "timestamptz" => "TIMESTAMP WITH TIME ZONE".into(),
        "timestamp" => "TIMESTAMP".into(),
        "date" => "DATE".into(),
        "time" => "TIME".into(),
        "bytea" => "BYTEA".into(),
        "numeric" => "NUMERIC".into(),
        other => other.to_uppercase(),
    }
}

/// Extract column names from a `pg_indexes.indexdef` string.
///
/// Example input: `"CREATE UNIQUE INDEX users_email_idx ON public.users USING btree (email)"`
/// Returns: `["email"]`
fn parse_index_columns(indexdef: &str) -> Vec<String> {
    let paren = match indexdef.find('(') {
        Some(p) => p,
        None => return Vec::new(),
    };
    let content = &indexdef[paren + 1..];
    let end = match content.find(')') {
        Some(e) => e,
        None => return Vec::new(),
    };
    let inner = &content[..end];
    inner
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_owned())
        .collect()
}
