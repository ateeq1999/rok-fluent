//! Table and column builder DSL.

// ── ColumnBuilder ─────────────────────────────────────────────────────────────

/// Fluent builder for a single SQL column definition.
#[derive(Debug, Clone)]
pub struct ColumnBuilder {
    name: String,
    type_sql: String,
    nullable: bool,
    default: Option<String>,
    unique: bool,
    primary_key: bool,
    references: Option<(String, String)>, // (table, column)
}

impl ColumnBuilder {
    pub(crate) fn new(name: impl Into<String>, type_sql: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            type_sql: type_sql.into(),
            nullable: true,
            default: None,
            unique: false,
            primary_key: false,
            references: None,
        }
    }

    /// Mark the column as `NOT NULL`.
    pub fn not_null(&mut self) -> &mut Self {
        self.nullable = false;
        self
    }

    /// Explicitly mark the column as nullable (the default — useful for documentation).
    pub fn nullable(&mut self) -> &mut Self {
        self.nullable = true;
        self
    }

    /// Set a default value (raw SQL expression, e.g. `"true"`, `"NOW()"`, `"'active'"`)
    pub fn default(&mut self, expr: impl Into<String>) -> &mut Self {
        self.default = Some(expr.into());
        self
    }

    /// Add a `UNIQUE` constraint.
    pub fn unique(&mut self) -> &mut Self {
        self.unique = true;
        self
    }

    /// Mark as `PRIMARY KEY`.
    pub fn primary_key(&mut self) -> &mut Self {
        self.primary_key = true;
        self
    }

    /// Add a `REFERENCES table(column)` foreign key constraint.
    pub fn references(&mut self, table: impl Into<String>, column: impl Into<String>) -> &mut Self {
        self.references = Some((table.into(), column.into()));
        self
    }

    /// Render the column definition to SQL.
    pub(crate) fn to_sql(&self) -> String {
        let mut parts = vec![format!("\"{}\" {}", self.name, self.type_sql)];
        if self.primary_key {
            parts.push("PRIMARY KEY".to_string());
        }
        if !self.nullable {
            parts.push("NOT NULL".to_string());
        }
        if self.unique {
            parts.push("UNIQUE".to_string());
        }
        if let Some(ref def) = self.default {
            parts.push(format!("DEFAULT {def}"));
        }
        if let Some((ref tbl, ref col)) = self.references {
            parts.push(format!("REFERENCES \"{tbl}\"(\"{col}\")"));
        }
        parts.join(" ")
    }
}

// ── TableBuilder ──────────────────────────────────────────────────────────────

/// Accumulates column definitions and index statements for a `CREATE TABLE`.
#[derive(Debug, Default)]
pub struct TableBuilder {
    pub(crate) columns: Vec<ColumnBuilder>,
    pub(crate) indices: Vec<String>,
}

impl TableBuilder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    // ── Convenience helpers ──────────────────────────────────────────────

    /// `id BIGSERIAL PRIMARY KEY` — auto-increment 64-bit integer PK.
    pub fn id(&mut self) -> &mut ColumnBuilder {
        let mut col = ColumnBuilder::new("id", "BIGSERIAL");
        col.primary_key();
        col.not_null();
        self.columns.push(col);
        self.columns.last_mut().unwrap()
    }

    /// `uuid_id UUID PRIMARY KEY DEFAULT gen_random_uuid()`.
    pub fn uuid_id(&mut self) -> &mut ColumnBuilder {
        let mut col = ColumnBuilder::new("id", "UUID");
        col.primary_key();
        col.not_null();
        col.default("gen_random_uuid()");
        self.columns.push(col);
        self.columns.last_mut().unwrap()
    }

    /// `id TEXT PRIMARY KEY NOT NULL` — ULID primary key (application-generated).
    pub fn ulid_pk(&mut self) -> &mut ColumnBuilder {
        let mut col = ColumnBuilder::new("id", "TEXT");
        col.primary_key();
        col.not_null();
        self.columns.push(col);
        self.columns.last_mut().unwrap()
    }

    /// `id TEXT PRIMARY KEY NOT NULL` — CUID2 primary key (application-generated).
    pub fn cuid2_pk(&mut self) -> &mut ColumnBuilder {
        let mut col = ColumnBuilder::new("id", "TEXT");
        col.primary_key();
        col.not_null();
        self.columns.push(col);
        self.columns.last_mut().unwrap()
    }

    /// `id BIGINT PRIMARY KEY NOT NULL` — Snowflake ID primary key.
    pub fn snowflake_pk(&mut self) -> &mut ColumnBuilder {
        let mut col = ColumnBuilder::new("id", "BIGINT");
        col.primary_key();
        col.not_null();
        self.columns.push(col);
        self.columns.last_mut().unwrap()
    }

    /// `id VARCHAR(36) PRIMARY KEY NOT NULL` — UUID v7 primary key stored as string.
    pub fn uuid_v7_pk(&mut self) -> &mut ColumnBuilder {
        let mut col = ColumnBuilder::new("id", "VARCHAR(36)");
        col.primary_key();
        col.not_null();
        self.columns.push(col);
        self.columns.last_mut().unwrap()
    }

    /// `id TEXT PRIMARY KEY NOT NULL` — NanoID primary key (application-generated).
    pub fn nanoid_pk(&mut self) -> &mut ColumnBuilder {
        let mut col = ColumnBuilder::new("id", "TEXT");
        col.primary_key();
        col.not_null();
        self.columns.push(col);
        self.columns.last_mut().unwrap()
    }

    /// Add a `TEXT` column.
    pub fn string(&mut self, name: impl Into<String>) -> &mut ColumnBuilder {
        self.columns.push(ColumnBuilder::new(name, "TEXT"));
        self.columns.last_mut().unwrap()
    }

    /// Add a `VARCHAR(n)` column.
    pub fn varchar(&mut self, name: impl Into<String>, len: u32) -> &mut ColumnBuilder {
        self.columns
            .push(ColumnBuilder::new(name, format!("VARCHAR({len})")));
        self.columns.last_mut().unwrap()
    }

    /// Add a `BIGINT` column.
    pub fn big_integer(&mut self, name: impl Into<String>) -> &mut ColumnBuilder {
        self.columns.push(ColumnBuilder::new(name, "BIGINT"));
        self.columns.last_mut().unwrap()
    }

    /// Add an `INTEGER` column.
    pub fn integer(&mut self, name: impl Into<String>) -> &mut ColumnBuilder {
        self.columns.push(ColumnBuilder::new(name, "INTEGER"));
        self.columns.last_mut().unwrap()
    }

    /// Add a `SMALLINT` column.
    pub fn small_integer(&mut self, name: impl Into<String>) -> &mut ColumnBuilder {
        self.columns.push(ColumnBuilder::new(name, "SMALLINT"));
        self.columns.last_mut().unwrap()
    }

    /// Add a `BOOLEAN` column.
    pub fn boolean(&mut self, name: impl Into<String>) -> &mut ColumnBuilder {
        self.columns.push(ColumnBuilder::new(name, "BOOLEAN"));
        self.columns.last_mut().unwrap()
    }

    /// Add a `DOUBLE PRECISION` (f64) column.
    pub fn double(&mut self, name: impl Into<String>) -> &mut ColumnBuilder {
        self.columns
            .push(ColumnBuilder::new(name, "DOUBLE PRECISION"));
        self.columns.last_mut().unwrap()
    }

    /// Add a `DECIMAL(precision, scale)` column.
    pub fn decimal(
        &mut self,
        name: impl Into<String>,
        precision: u8,
        scale: u8,
    ) -> &mut ColumnBuilder {
        self.columns.push(ColumnBuilder::new(
            name,
            format!("DECIMAL({precision}, {scale})"),
        ));
        self.columns.last_mut().unwrap()
    }

    /// Add a `JSONB` column.
    pub fn json(&mut self, name: impl Into<String>) -> &mut ColumnBuilder {
        self.columns.push(ColumnBuilder::new(name, "JSONB"));
        self.columns.last_mut().unwrap()
    }

    /// Add a `TIMESTAMPTZ` column.
    pub fn timestamp(&mut self, name: impl Into<String>) -> &mut ColumnBuilder {
        self.columns.push(ColumnBuilder::new(name, "TIMESTAMPTZ"));
        self.columns.last_mut().unwrap()
    }

    /// Add a `DATE` column.
    pub fn date(&mut self, name: impl Into<String>) -> &mut ColumnBuilder {
        self.columns.push(ColumnBuilder::new(name, "DATE"));
        self.columns.last_mut().unwrap()
    }

    /// Add a `BYTEA` (binary) column.
    pub fn binary(&mut self, name: impl Into<String>) -> &mut ColumnBuilder {
        self.columns.push(ColumnBuilder::new(name, "BYTEA"));
        self.columns.last_mut().unwrap()
    }

    /// Add an enum column using a custom enum type.
    ///
    /// The enum type must already exist (create it via [`Schema::create_enum`]).
    ///
    /// ```rust
    /// use rok_orm_migrate::Schema;
    ///
    /// let sql = Schema::create("posts", |t| {
    ///     t.id();
    ///     t.enum_col("status", "post_status");
    /// }).to_sql();
    /// assert!(sql.contains("\"status\" post_status"));
    /// ```
    pub fn enum_col(&mut self, name: impl Into<String>, enum_name: &str) -> &mut ColumnBuilder {
        self.columns
            .push(ColumnBuilder::new(name, enum_name.to_string()));
        self.columns.last_mut().unwrap()
    }

    /// Add a raw column definition (type specified manually).
    pub fn column(
        &mut self,
        name: impl Into<String>,
        type_sql: impl Into<String>,
    ) -> &mut ColumnBuilder {
        self.columns.push(ColumnBuilder::new(name, type_sql));
        self.columns.last_mut().unwrap()
    }

    // ── Extended types (v3) ───────────────────────────────────────────────────

    /// `NUMERIC(19, 4)` — fixed-precision currency / monetary amount.
    pub fn money(&mut self, name: impl Into<String>) -> &mut ColumnBuilder {
        self.columns
            .push(ColumnBuilder::new(name, "NUMERIC(19, 4)"));
        self.columns.last_mut().unwrap()
    }

    /// `INET` — IPv4 or IPv6 address (PostgreSQL).
    pub fn ip_address(&mut self, name: impl Into<String>) -> &mut ColumnBuilder {
        self.columns.push(ColumnBuilder::new(name, "INET"));
        self.columns.last_mut().unwrap()
    }

    /// `MACADDR` — MAC address (PostgreSQL).
    pub fn mac_address(&mut self, name: impl Into<String>) -> &mut ColumnBuilder {
        self.columns.push(ColumnBuilder::new(name, "MACADDR"));
        self.columns.last_mut().unwrap()
    }

    /// `BYTEA` — binary blob, suitable for storing application-layer encrypted values.
    pub fn encrypted(&mut self, name: impl Into<String>) -> &mut ColumnBuilder {
        self.columns.push(ColumnBuilder::new(name, "BYTEA"));
        self.columns.last_mut().unwrap()
    }

    /// `vector(dims)` — pgvector embedding column (requires the pgvector extension).
    ///
    /// ```sql
    /// CREATE EXTENSION IF NOT EXISTS vector;
    /// ```
    pub fn vector(&mut self, name: impl Into<String>, dims: u32) -> &mut ColumnBuilder {
        self.columns
            .push(ColumnBuilder::new(name, format!("vector({dims})")));
        self.columns.last_mut().unwrap()
    }

    /// Add `BIGINT NOT NULL REFERENCES parent_table(id)` foreign key column.
    pub fn foreign_id(
        &mut self,
        name: impl Into<String>,
        references_table: impl Into<String>,
    ) -> &mut ColumnBuilder {
        let table = references_table.into();
        let mut col = ColumnBuilder::new(name, "BIGINT");
        col.not_null();
        col.references(&table, "id");
        self.columns.push(col);
        self.columns.last_mut().unwrap()
    }

    /// Add `created_at TIMESTAMPTZ` and `updated_at TIMESTAMPTZ` columns.
    pub fn timestamps(&mut self) {
        let mut c = ColumnBuilder::new("created_at", "TIMESTAMPTZ");
        c.not_null();
        c.default("NOW()");
        self.columns.push(c);
        let mut u = ColumnBuilder::new("updated_at", "TIMESTAMPTZ");
        u.not_null();
        u.default("NOW()");
        self.columns.push(u);
    }

    /// Add `deleted_at TIMESTAMPTZ NULL` (soft-delete column).
    pub fn soft_deletes(&mut self) {
        self.columns
            .push(ColumnBuilder::new("deleted_at", "TIMESTAMPTZ"));
    }

    /// Add a raw index statement (appended after column definitions).
    pub fn index(&mut self, index_sql: impl Into<String>) {
        self.indices.push(index_sql.into());
    }
}

// ── AlterColumnBuilder ────────────────────────────────────────────────────────

/// Closure argument for `AlterTableBuilder::change_column`.
///
/// Specify the new column type and/or nullability; unset fields are left unchanged.
pub struct AlterColumnBuilder {
    pub(crate) type_sql: Option<String>,
    pub(crate) nullable: Option<bool>,
}

impl AlterColumnBuilder {
    pub fn text(&mut self) -> &mut Self {
        self.type_sql = Some("TEXT".into());
        self
    }
    pub fn string(&mut self) -> &mut Self {
        self.type_sql = Some("TEXT".into());
        self
    }
    pub fn big_integer(&mut self) -> &mut Self {
        self.type_sql = Some("BIGINT".into());
        self
    }
    pub fn integer(&mut self) -> &mut Self {
        self.type_sql = Some("INTEGER".into());
        self
    }
    pub fn small_integer(&mut self) -> &mut Self {
        self.type_sql = Some("SMALLINT".into());
        self
    }
    pub fn boolean(&mut self) -> &mut Self {
        self.type_sql = Some("BOOLEAN".into());
        self
    }
    pub fn double(&mut self) -> &mut Self {
        self.type_sql = Some("DOUBLE PRECISION".into());
        self
    }
    pub fn json(&mut self) -> &mut Self {
        self.type_sql = Some("JSONB".into());
        self
    }
    pub fn timestamp(&mut self) -> &mut Self {
        self.type_sql = Some("TIMESTAMPTZ".into());
        self
    }
    pub fn uuid(&mut self) -> &mut Self {
        self.type_sql = Some("UUID".into());
        self
    }
    pub fn decimal(&mut self, p: u8, s: u8) -> &mut Self {
        self.type_sql = Some(format!("DECIMAL({p}, {s})"));
        self
    }
    /// Set a raw SQL type string.
    pub fn type_sql(&mut self, sql: impl Into<String>) -> &mut Self {
        self.type_sql = Some(sql.into());
        self
    }
    pub fn nullable(&mut self) -> &mut Self {
        self.nullable = Some(true);
        self
    }
    pub fn not_null(&mut self) -> &mut Self {
        self.nullable = Some(false);
        self
    }
}
