//! [`Schema`] — entry point for migration DDL builders.

use crate::table::{AlterColumnBuilder, TableBuilder};

// ── CreateTable ───────────────────────────────────────────────────────────────

/// Builder for a `CREATE TABLE` statement.
pub struct CreateTableBuilder {
    table: String,
    if_not_exists: bool,
    tb: TableBuilder,
}

impl CreateTableBuilder {
    /// Generate the `CREATE TABLE` SQL string.
    pub fn to_sql(&self) -> String {
        let qual = if self.if_not_exists {
            "IF NOT EXISTS "
        } else {
            ""
        };
        let cols: Vec<String> = self.tb.columns.iter().map(|c| c.to_sql()).collect();
        let body = cols.join(",\n    ");
        let mut parts = vec![format!(
            "CREATE TABLE {qual}\"{table}\" (\n    {body}\n)",
            qual = qual,
            table = self.table,
            body = body,
        )];
        for idx in &self.tb.indices {
            parts.push(idx.clone());
        }
        parts.join(";\n")
    }
}

// ── AlterTableBuilder ─────────────────────────────────────────────────────────

/// Fluent builder for a sequence of `ALTER TABLE` statements.
///
/// Returned by [`Schema::alter_table`].  Each method appends one or more SQL
/// statements; call [`to_sql_statements`] to retrieve them as individual strings
/// or [`to_sql`] to get a single semicolon-joined string.
///
/// [`to_sql_statements`]: AlterTableBuilder::to_sql_statements
/// [`to_sql`]: AlterTableBuilder::to_sql
pub struct AlterTableBuilder {
    table: String,
    stmts: Vec<String>,
}

impl AlterTableBuilder {
    // ── column operations ─────────────────────────────────────────────────────

    /// `ALTER TABLE … ADD COLUMN …`
    ///
    /// The closure receives a [`TableBuilder`] so you can use the same fluent
    /// column helpers as in `Schema::create`.
    ///
    /// ```rust
    /// use rok_orm_migrate::Schema;
    ///
    /// let stmts = Schema::alter_table("users", |t| {
    ///     t.add_column(|c| { c.string("phone"); });
    /// }).to_sql_statements();
    ///
    /// assert_eq!(stmts[0], "ALTER TABLE \"users\" ADD COLUMN \"phone\" TEXT");
    /// ```
    pub fn add_column(&mut self, f: impl FnOnce(&mut TableBuilder)) -> &mut Self {
        let mut tb = TableBuilder::new();
        f(&mut tb);
        for col in &tb.columns {
            self.stmts.push(format!(
                "ALTER TABLE \"{}\" ADD COLUMN {}",
                self.table,
                col.to_sql()
            ));
        }
        self
    }

    /// `ALTER TABLE … DROP COLUMN "name"`
    pub fn drop_column(&mut self, name: impl Into<String>) -> &mut Self {
        self.stmts.push(format!(
            "ALTER TABLE \"{}\" DROP COLUMN \"{}\"",
            self.table,
            name.into()
        ));
        self
    }

    /// `ALTER TABLE … RENAME COLUMN "from" TO "to"`
    pub fn rename_column(&mut self, from: impl Into<String>, to: impl Into<String>) -> &mut Self {
        self.stmts.push(format!(
            "ALTER TABLE \"{}\" RENAME COLUMN \"{}\" TO \"{}\"",
            self.table,
            from.into(),
            to.into()
        ));
        self
    }

    /// `ALTER TABLE … ALTER COLUMN … TYPE …` (and optionally SET/DROP NOT NULL).
    ///
    /// The closure receives an [`AlterColumnBuilder`] — set `.text()`, `.nullable()`, etc.
    ///
    /// ```rust
    /// use rok_orm_migrate::Schema;
    ///
    /// let stmts = Schema::alter_table("users", |t| {
    ///     t.change_column("bio", |c| { c.text().nullable(); });
    /// }).to_sql_statements();
    ///
    /// assert!(stmts[0].contains("ALTER COLUMN \"bio\" TYPE TEXT"));
    /// assert!(stmts[1].contains("DROP NOT NULL"));
    /// ```
    pub fn change_column(
        &mut self,
        name: impl Into<String>,
        f: impl FnOnce(&mut AlterColumnBuilder),
    ) -> &mut Self {
        let name = name.into();
        let mut acb = AlterColumnBuilder {
            type_sql: None,
            nullable: None,
        };
        f(&mut acb);
        if let Some(type_sql) = acb.type_sql {
            self.stmts.push(format!(
                "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" TYPE {}",
                self.table, name, type_sql
            ));
        }
        match acb.nullable {
            Some(true) => self.stmts.push(format!(
                "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" DROP NOT NULL",
                self.table, name
            )),
            Some(false) => self.stmts.push(format!(
                "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" SET NOT NULL",
                self.table, name
            )),
            None => {}
        }
        self
    }

    // ── index operations ──────────────────────────────────────────────────────

    /// `CREATE INDEX IF NOT EXISTS "idx_{table}_{col}" ON "{table}"("{col}")`
    pub fn add_index(&mut self, col: impl Into<String>) -> &mut Self {
        let col = col.into();
        self.stmts.push(format!(
            "CREATE INDEX IF NOT EXISTS \"idx_{table}_{col}\" ON \"{table}\"(\"{col}\")",
            table = self.table,
            col = col,
        ));
        self
    }

    /// `CREATE UNIQUE INDEX IF NOT EXISTS "idx_{table}_{col}" ON "{table}"("{col}")`
    pub fn add_unique_index(&mut self, col: impl Into<String>) -> &mut Self {
        let col = col.into();
        self.stmts.push(format!(
            "CREATE UNIQUE INDEX IF NOT EXISTS \"idx_{table}_{col}\" ON \"{table}\"(\"{col}\")",
            table = self.table,
            col = col,
        ));
        self
    }

    /// `DROP INDEX IF EXISTS "name"`
    pub fn drop_index(&mut self, name: impl Into<String>) -> &mut Self {
        self.stmts
            .push(format!("DROP INDEX IF EXISTS \"{}\"", name.into()));
        self
    }

    // ── foreign key operations ────────────────────────────────────────────────

    /// ```sql
    /// ALTER TABLE "t" ADD CONSTRAINT "fk_t_col"
    ///   FOREIGN KEY ("col") REFERENCES "ref_table"("ref_col")
    /// ```
    pub fn add_foreign(
        &mut self,
        col: impl Into<String>,
        ref_table: impl Into<String>,
        ref_col: impl Into<String>,
    ) -> &mut Self {
        let col = col.into();
        let ref_table = ref_table.into();
        let ref_col = ref_col.into();
        let constraint = format!("fk_{}_{}", self.table, col);
        self.stmts.push(format!(
            "ALTER TABLE \"{}\" ADD CONSTRAINT \"{}\" \
             FOREIGN KEY (\"{}\") REFERENCES \"{}\"(\"{}\")",
            self.table, constraint, col, ref_table, ref_col
        ));
        self
    }

    /// `ALTER TABLE "t" DROP CONSTRAINT "constraint_name"`
    pub fn drop_foreign(&mut self, constraint_name: impl Into<String>) -> &mut Self {
        self.stmts.push(format!(
            "ALTER TABLE \"{}\" DROP CONSTRAINT \"{}\"",
            self.table,
            constraint_name.into()
        ));
        self
    }

    // ── output ────────────────────────────────────────────────────────────────

    /// Return each SQL statement as a separate `String`.
    pub fn to_sql_statements(&self) -> Vec<String> {
        self.stmts.clone()
    }

    /// Return all statements joined with `";\n"`.
    pub fn to_sql(&self) -> String {
        self.stmts.join(";\n")
    }
}

// ── DropTable ─────────────────────────────────────────────────────────────────

/// Builder for a `DROP TABLE` statement.
pub struct DropTableBuilder {
    table: String,
    if_exists: bool,
    cascade: bool,
}

impl DropTableBuilder {
    pub fn if_exists(mut self) -> Self {
        self.if_exists = true;
        self
    }

    pub fn cascade(mut self) -> Self {
        self.cascade = true;
        self
    }

    pub fn to_sql(&self) -> String {
        let qual = if self.if_exists { "IF EXISTS " } else { "" };
        let cas = if self.cascade { " CASCADE" } else { "" };
        format!("DROP TABLE {qual}\"{}\"{cas}", self.table)
    }
}

// ── Schema ────────────────────────────────────────────────────────────────────

/// Entry point for the migration DDL DSL.
///
/// # Example
///
/// ```rust
/// use rok_orm_migrate::Schema;
///
/// let sql = Schema::create("users", |t| {
///     t.id();
///     t.string("name").not_null();
///     t.string("email").not_null().unique();
///     t.timestamps();
/// }).to_sql();
///
/// assert!(sql.contains("CREATE TABLE"));
/// assert!(sql.contains("\"users\""));
/// assert!(sql.contains("\"name\" TEXT NOT NULL"));
/// ```
pub struct Schema;

impl Schema {
    // ── CREATE TABLE ──────────────────────────────────────────────────────────

    /// Build a `CREATE TABLE` statement.
    pub fn create(
        table: impl Into<String>,
        f: impl FnOnce(&mut TableBuilder),
    ) -> CreateTableBuilder {
        let mut tb = TableBuilder::new();
        f(&mut tb);
        CreateTableBuilder {
            table: table.into(),
            if_not_exists: false,
            tb,
        }
    }

    /// Build a `CREATE TABLE IF NOT EXISTS` statement.
    pub fn create_if_not_exists(
        table: impl Into<String>,
        f: impl FnOnce(&mut TableBuilder),
    ) -> CreateTableBuilder {
        let mut tb = TableBuilder::new();
        f(&mut tb);
        CreateTableBuilder {
            table: table.into(),
            if_not_exists: true,
            tb,
        }
    }

    // ── ALTER TABLE ───────────────────────────────────────────────────────────

    /// Build a sequence of `ALTER TABLE` statements.
    ///
    /// ```rust
    /// use rok_orm_migrate::Schema;
    ///
    /// let stmts = Schema::alter_table("users", |t| {
    ///     t.add_column(|c| { c.string("phone").nullable(); });
    ///     t.drop_column("legacy_token");
    ///     t.rename_column("fname", "first_name");
    /// }).to_sql_statements();
    ///
    /// assert_eq!(stmts.len(), 3);
    /// assert!(stmts[0].contains("ADD COLUMN"));
    /// assert!(stmts[1].contains("DROP COLUMN"));
    /// assert!(stmts[2].contains("RENAME COLUMN"));
    /// ```
    pub fn alter_table(
        table: impl Into<String>,
        f: impl FnOnce(&mut AlterTableBuilder),
    ) -> AlterTableBuilder {
        let mut b = AlterTableBuilder {
            table: table.into(),
            stmts: Vec::new(),
        };
        f(&mut b);
        b
    }

    /// Alias for [`alter_table`](Schema::alter_table) — kept for backwards compatibility.
    pub fn table(
        table: impl Into<String>,
        f: impl FnOnce(&mut AlterTableBuilder),
    ) -> AlterTableBuilder {
        Self::alter_table(table, f)
    }

    // ── RENAME / DROP TABLE ───────────────────────────────────────────────────

    /// `ALTER TABLE "from" RENAME TO "to"`
    pub fn rename_table(from: impl Into<String>, to: impl Into<String>) -> String {
        format!(
            "ALTER TABLE \"{}\" RENAME TO \"{}\"",
            from.into(),
            to.into()
        )
    }

    /// `DROP TABLE "name"`
    pub fn drop_table(name: impl Into<String>) -> String {
        format!("DROP TABLE \"{}\"", name.into())
    }

    /// `DROP TABLE IF EXISTS "name"`
    pub fn drop_table_if_exists(name: impl Into<String>) -> String {
        format!("DROP TABLE IF EXISTS \"{}\"", name.into())
    }

    /// Builder-style `DROP TABLE` (with optional `.if_exists()` / `.cascade()` modifiers).
    pub fn drop(table: impl Into<String>) -> DropTableBuilder {
        DropTableBuilder {
            table: table.into(),
            if_exists: false,
            cascade: false,
        }
    }

    // ── ENUM ──────────────────────────────────────────────────────────────────

    /// `CREATE TYPE name AS ENUM ('v1', 'v2', …)`
    ///
    /// ```rust
    /// use rok_orm_migrate::Schema;
    ///
    /// let sql = Schema::create_enum("mood", &["happy", "sad", "neutral"]);
    /// assert_eq!(sql, "CREATE TYPE \"mood\" AS ENUM ('happy', 'sad', 'neutral')");
    /// ```
    pub fn create_enum(name: &str, variants: &[&str]) -> String {
        let vars: Vec<String> = variants.iter().map(|v| format!("'{}'", v)).collect();
        format!("CREATE TYPE \"{name}\" AS ENUM ({})", vars.join(", "))
    }

    /// `DROP TYPE IF EXISTS name`
    ///
    /// ```rust
    /// use rok_orm_migrate::Schema;
    ///
    /// let sql = Schema::drop_enum("mood");
    /// assert_eq!(sql, "DROP TYPE IF EXISTS \"mood\"");
    /// ```
    pub fn drop_enum(name: &str) -> String {
        format!("DROP TYPE IF EXISTS \"{name}\"")
    }

    /// `ALTER TYPE name ADD VALUE 'variant'`
    ///
    /// ```rust
    /// use rok_orm_migrate::Schema;
    ///
    /// let sql = Schema::alter_enum_add("mood", "ecstatic");
    /// assert_eq!(sql, "ALTER TYPE \"mood\" ADD VALUE 'ecstatic'");
    /// ```
    pub fn alter_enum_add(name: &str, variant: &str) -> String {
        format!("ALTER TYPE \"{name}\" ADD VALUE '{variant}'")
    }

    // ── RAW SQL ───────────────────────────────────────────────────────────────

    /// Return the raw SQL string unchanged — useful for one-off statements that
    /// don't fit a builder pattern (e.g. `ADD CONSTRAINT CHECK …`).
    pub fn raw(sql: impl Into<String>) -> String {
        sql.into()
    }
}

// ── SchemaExecutor ────────────────────────────────────────────────────────────

/// A live connection to a database that can execute DDL built by [`Schema`].
///
/// Passed to [`Migration::up`] and [`Migration::down`] by the runner.
/// All methods delegate SQL generation to the [`Schema`] static builders
/// and then execute each statement against the pool.
///
/// [`Migration::up`]: crate::migration::Migration::up
/// [`Migration::down`]: crate::migration::Migration::down
#[cfg(feature = "postgres")]
pub struct SchemaExecutor {
    pool: sqlx::PgPool,
}

#[cfg(feature = "postgres")]
impl SchemaExecutor {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    // ── CREATE TABLE ──────────────────────────────────────────────────────────

    /// `CREATE TABLE "name" (…)` — executes the full DDL including any indices.
    pub async fn create(
        &self,
        table: impl Into<String>,
        f: impl FnOnce(&mut crate::table::TableBuilder),
    ) -> anyhow::Result<()> {
        self.raw_execute(&Schema::create(table, f).to_sql()).await
    }

    /// `CREATE TABLE IF NOT EXISTS "name" (…)`
    pub async fn create_if_not_exists(
        &self,
        table: impl Into<String>,
        f: impl FnOnce(&mut crate::table::TableBuilder),
    ) -> anyhow::Result<()> {
        self.raw_execute(&Schema::create_if_not_exists(table, f).to_sql())
            .await
    }

    // ── ALTER TABLE ───────────────────────────────────────────────────────────

    /// Executes a sequence of `ALTER TABLE` statements built by the closure.
    pub async fn alter_table(
        &self,
        table: impl Into<String>,
        f: impl FnOnce(&mut AlterTableBuilder),
    ) -> anyhow::Result<()> {
        self.raw_execute(&Schema::alter_table(table, f).to_sql())
            .await
    }

    // ── RENAME / DROP TABLE ───────────────────────────────────────────────────

    /// `ALTER TABLE "from" RENAME TO "to"`
    pub async fn rename_table(
        &self,
        from: impl Into<String>,
        to: impl Into<String>,
    ) -> anyhow::Result<()> {
        self.raw_execute(&Schema::rename_table(from, to)).await
    }

    /// `DROP TABLE "name"`
    pub async fn drop_table(&self, table: impl Into<String>) -> anyhow::Result<()> {
        self.raw_execute(&Schema::drop_table(table)).await
    }

    /// `DROP TABLE IF EXISTS "name"`
    pub async fn drop_table_if_exists(&self, table: impl Into<String>) -> anyhow::Result<()> {
        self.raw_execute(&Schema::drop_table_if_exists(table)).await
    }

    // ── ENUM ──────────────────────────────────────────────────────────────────

    /// `CREATE TYPE "name" AS ENUM ('v1', …)`
    pub async fn create_enum(&self, name: &str, variants: &[&str]) -> anyhow::Result<()> {
        self.raw_execute(&Schema::create_enum(name, variants)).await
    }

    /// `DROP TYPE IF EXISTS "name"`
    pub async fn drop_enum(&self, name: &str) -> anyhow::Result<()> {
        self.raw_execute(&Schema::drop_enum(name)).await
    }

    /// `ALTER TYPE "name" ADD VALUE 'variant'`
    pub async fn alter_enum_add(&self, name: &str, variant: &str) -> anyhow::Result<()> {
        self.raw_execute(&Schema::alter_enum_add(name, variant))
            .await
    }

    // ── RAW SQL ───────────────────────────────────────────────────────────────

    /// Execute arbitrary SQL — statements are split on `;` and run individually.
    pub async fn raw(&self, sql: impl Into<String>) -> anyhow::Result<()> {
        self.raw_execute(&sql.into()).await
    }

    /// Execute raw SQL — used internally by the `RawMigrationAdapter`.
    pub(crate) async fn raw_execute(&self, sql: &str) -> anyhow::Result<()> {
        for stmt in sql.split(';') {
            let stmt = stmt.trim();
            if !stmt.is_empty() {
                sqlx::query(stmt).execute(&self.pool).await?;
            }
        }
        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── CREATE TABLE ──────────────────────────────────────────────────────────

    #[test]
    fn create_table_basic() {
        let sql = Schema::create("users", |t| {
            t.id();
            t.string("name").not_null();
            t.string("email").not_null().unique();
            t.timestamps();
        })
        .to_sql();

        assert!(sql.contains("CREATE TABLE \"users\""));
        assert!(sql.contains("\"id\" BIGSERIAL PRIMARY KEY NOT NULL"));
        assert!(sql.contains("\"name\" TEXT NOT NULL"));
        assert!(sql.contains("\"email\" TEXT NOT NULL UNIQUE"));
        assert!(sql.contains("\"created_at\" TIMESTAMPTZ"));
    }

    #[test]
    fn create_if_not_exists() {
        let sql = Schema::create_if_not_exists("posts", |t| {
            t.id();
            t.big_integer("user_id").not_null();
            t.string("title").not_null();
            t.soft_deletes();
        })
        .to_sql();

        assert!(sql.contains("IF NOT EXISTS"));
        assert!(sql.contains("\"deleted_at\" TIMESTAMPTZ"));
    }

    // ── DROP TABLE ────────────────────────────────────────────────────────────

    #[test]
    fn drop_builder() {
        let sql = Schema::drop("users").if_exists().cascade().to_sql();
        assert_eq!(sql, "DROP TABLE IF EXISTS \"users\" CASCADE");
    }

    #[test]
    fn drop_table_string() {
        assert_eq!(Schema::drop_table("users"), "DROP TABLE \"users\"");
        assert_eq!(
            Schema::drop_table_if_exists("users"),
            "DROP TABLE IF EXISTS \"users\""
        );
    }

    // ── RENAME TABLE ──────────────────────────────────────────────────────────

    #[test]
    fn rename_table() {
        let sql = Schema::rename_table("posts", "articles");
        assert_eq!(sql, "ALTER TABLE \"posts\" RENAME TO \"articles\"");
    }

    // ── RAW SQL ───────────────────────────────────────────────────────────────

    // ── ENUM ──────────────────────────────────────────────────────────────────

    #[test]
    fn create_enum_basic() {
        let sql = Schema::create_enum("mood", &["happy", "sad", "neutral"]);
        assert_eq!(
            sql,
            "CREATE TYPE \"mood\" AS ENUM ('happy', 'sad', 'neutral')"
        );
    }

    #[test]
    fn create_enum_single_variant() {
        let sql = Schema::create_enum("status", &["active"]);
        assert_eq!(sql, "CREATE TYPE \"status\" AS ENUM ('active')");
    }

    #[test]
    fn drop_enum_basic() {
        let sql = Schema::drop_enum("mood");
        assert_eq!(sql, "DROP TYPE IF EXISTS \"mood\"");
    }

    #[test]
    fn alter_enum_add_basic() {
        let sql = Schema::alter_enum_add("mood", "ecstatic");
        assert_eq!(sql, "ALTER TYPE \"mood\" ADD VALUE 'ecstatic'");
    }

    #[test]
    fn raw_passthrough() {
        let sql = Schema::raw("ALTER TABLE users ADD CONSTRAINT chk_age CHECK (age >= 0)");
        assert_eq!(
            sql,
            "ALTER TABLE users ADD CONSTRAINT chk_age CHECK (age >= 0)"
        );
    }

    // ── ADD COLUMN ────────────────────────────────────────────────────────────

    #[test]
    fn alter_add_column() {
        let stmts = Schema::alter_table("users", |t| {
            t.add_column(|c| {
                c.string("phone");
            });
        })
        .to_sql_statements();
        assert_eq!(stmts.len(), 1);
        assert_eq!(stmts[0], "ALTER TABLE \"users\" ADD COLUMN \"phone\" TEXT");
    }

    #[test]
    fn alter_add_column_not_null_default() {
        let stmts = Schema::alter_table("users", |t| {
            t.add_column(|c| {
                c.boolean("email_verified").not_null().default("FALSE");
            });
        })
        .to_sql_statements();
        assert_eq!(stmts.len(), 1);
        assert!(
            stmts[0].contains("ADD COLUMN \"email_verified\" BOOLEAN NOT NULL DEFAULT FALSE"),
            "sql={}",
            stmts[0]
        );
    }

    #[test]
    fn alter_add_column_nullable_explicit() {
        let stmts = Schema::alter_table("users", |t| {
            t.add_column(|c| {
                c.string("bio").nullable();
            });
        })
        .to_sql_statements();
        assert_eq!(stmts[0], "ALTER TABLE \"users\" ADD COLUMN \"bio\" TEXT");
    }

    // ── DROP COLUMN ───────────────────────────────────────────────────────────

    #[test]
    fn alter_drop_column() {
        let stmts = Schema::alter_table("users", |t| {
            t.drop_column("legacy_token");
        })
        .to_sql_statements();
        assert_eq!(
            stmts[0],
            "ALTER TABLE \"users\" DROP COLUMN \"legacy_token\""
        );
    }

    // ── RENAME COLUMN ─────────────────────────────────────────────────────────

    #[test]
    fn alter_rename_column() {
        let stmts = Schema::alter_table("users", |t| {
            t.rename_column("fname", "first_name");
        })
        .to_sql_statements();
        assert_eq!(
            stmts[0],
            "ALTER TABLE \"users\" RENAME COLUMN \"fname\" TO \"first_name\""
        );
    }

    // ── CHANGE COLUMN ─────────────────────────────────────────────────────────

    #[test]
    fn alter_change_column_type_and_nullable() {
        let stmts = Schema::alter_table("users", |t| {
            t.change_column("bio", |c| {
                c.text().nullable();
            });
        })
        .to_sql_statements();
        assert_eq!(stmts.len(), 2);
        assert_eq!(
            stmts[0],
            "ALTER TABLE \"users\" ALTER COLUMN \"bio\" TYPE TEXT"
        );
        assert_eq!(
            stmts[1],
            "ALTER TABLE \"users\" ALTER COLUMN \"bio\" DROP NOT NULL"
        );
    }

    #[test]
    fn alter_change_column_type_not_null() {
        let stmts = Schema::alter_table("users", |t| {
            t.change_column("score", |c| {
                c.integer().not_null();
            });
        })
        .to_sql_statements();
        assert_eq!(stmts.len(), 2);
        assert!(stmts[0].contains("TYPE INTEGER"), "sql={}", stmts[0]);
        assert!(stmts[1].contains("SET NOT NULL"), "sql={}", stmts[1]);
    }

    #[test]
    fn alter_change_column_type_only() {
        let stmts = Schema::alter_table("users", |t| {
            t.change_column("meta", |c| {
                c.json();
            });
        })
        .to_sql_statements();
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("TYPE JSONB"), "sql={}", stmts[0]);
    }

    // ── ADD INDEX ─────────────────────────────────────────────────────────────

    #[test]
    fn alter_add_index() {
        let stmts = Schema::alter_table("users", |t| {
            t.add_index("email");
        })
        .to_sql_statements();
        assert_eq!(
            stmts[0],
            "CREATE INDEX IF NOT EXISTS \"idx_users_email\" ON \"users\"(\"email\")"
        );
    }

    #[test]
    fn alter_add_unique_index() {
        let stmts = Schema::alter_table("users", |t| {
            t.add_unique_index("phone");
        })
        .to_sql_statements();
        assert_eq!(
            stmts[0],
            "CREATE UNIQUE INDEX IF NOT EXISTS \"idx_users_phone\" ON \"users\"(\"phone\")"
        );
    }

    #[test]
    fn alter_drop_index() {
        let stmts = Schema::alter_table("users", |t| {
            t.drop_index("idx_users_legacy");
        })
        .to_sql_statements();
        assert_eq!(stmts[0], "DROP INDEX IF EXISTS \"idx_users_legacy\"");
    }

    // ── FOREIGN KEY ───────────────────────────────────────────────────────────

    #[test]
    fn alter_add_foreign() {
        let stmts = Schema::alter_table("users", |t| {
            t.add_foreign("org_id", "organisations", "id");
        })
        .to_sql_statements();
        assert_eq!(
            stmts[0],
            "ALTER TABLE \"users\" ADD CONSTRAINT \"fk_users_org_id\" \
             FOREIGN KEY (\"org_id\") REFERENCES \"organisations\"(\"id\")"
        );
    }

    #[test]
    fn alter_drop_foreign() {
        let stmts = Schema::alter_table("users", |t| {
            t.drop_foreign("fk_users_org_id");
        })
        .to_sql_statements();
        assert_eq!(
            stmts[0],
            "ALTER TABLE \"users\" DROP CONSTRAINT \"fk_users_org_id\""
        );
    }

    // ── COMBINED ─────────────────────────────────────────────────────────────

    #[test]
    fn alter_combined_operations() {
        let stmts = Schema::alter_table("users", |t| {
            t.add_column(|c| {
                c.string("phone").nullable();
            });
            t.drop_column("legacy_token");
            t.rename_column("fname", "first_name");
            t.add_index("email");
            t.add_foreign("org_id", "organisations", "id");
        })
        .to_sql_statements();

        assert_eq!(stmts.len(), 5);
        assert!(stmts[0].contains("ADD COLUMN"));
        assert!(stmts[1].contains("DROP COLUMN"));
        assert!(stmts[2].contains("RENAME COLUMN"));
        assert!(stmts[3].contains("CREATE INDEX"));
        assert!(stmts[4].contains("ADD CONSTRAINT"));
    }

    #[test]
    fn to_sql_joins_with_semicolons() {
        let sql = Schema::alter_table("users", |t| {
            t.drop_column("a");
            t.drop_column("b");
        })
        .to_sql();
        assert_eq!(
            sql,
            "ALTER TABLE \"users\" DROP COLUMN \"a\";\nALTER TABLE \"users\" DROP COLUMN \"b\""
        );
    }
}
