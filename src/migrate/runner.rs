//! [`MigrationRunner`] — execute migrations from one or more sources.

use super::{
    migration::{Migration, RawMigration},
    schema::SchemaExecutor,
    source::MigrationSource,
};

// ── RawMigrationAdapter ───────────────────────────────────────────────────────

/// Bridge: wraps a [`RawMigration`] (sync SQL-string) into the async
/// [`Migration`] interface so the runner can treat both types uniformly.
struct RawMigrationAdapter(Box<dyn RawMigration>);

#[async_trait::async_trait]
impl Migration for RawMigrationAdapter {
    fn name(&self) -> &str {
        self.0.name()
    }

    async fn up(&self, schema: &SchemaExecutor) -> anyhow::Result<()> {
        schema.raw_execute(&self.0.up()).await
    }

    async fn down(&self, schema: &SchemaExecutor) -> anyhow::Result<()> {
        schema.raw_execute(&self.0.down()).await
    }
}

// ── MigrationRunner ───────────────────────────────────────────────────────────

/// Runs migrations from multiple sources against a PostgreSQL database.
///
/// Sources are applied in registration order.  A `_migrations` tracking table
/// records which migrations have already run so reruns are safe.
///
/// # Example
///
/// ```rust,no_run
/// use rok_fluent::migrate::{MigrationRunner, FileSource};
///
/// # async fn example(pool: sqlx::PgPool) -> anyhow::Result<()> {
/// MigrationRunner::new(pool)
///     .source(FileSource::new("./migrations"))
///     .run()
///     .await?;
/// # Ok(())
/// # }
/// ```
pub struct MigrationRunner {
    pool: sqlx::PgPool,
    migrations: Vec<Box<dyn Migration>>,
}

impl MigrationRunner {
    /// Create a new runner backed by the given pool.
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self {
            pool,
            migrations: Vec::new(),
        }
    }

    /// Register a [`MigrationSource`] (embedded SQL or file-based SQL).
    ///
    /// Each raw migration from the source is wrapped in `RawMigrationAdapter`
    /// so it can run alongside schema-builder migrations.
    pub fn source(mut self, s: impl MigrationSource + 'static) -> Self {
        for m in s.migrations() {
            self.migrations.push(Box::new(RawMigrationAdapter(m)));
        }
        self
    }

    /// Register a single schema-builder [`Migration`] inline.
    pub fn migration(mut self, m: impl Migration + 'static) -> Self {
        self.migrations.push(Box::new(m));
        self
    }

    /// Register multiple schema-builder [`Migration`]s from a `Vec` or iterator.
    pub fn migrations(mut self, ms: impl IntoIterator<Item = Box<dyn Migration>>) -> Self {
        self.migrations.extend(ms);
        self
    }

    /// Register a raw-SQL [`RawMigration`] inline (backward-compatible).
    ///
    /// Prefer `.migration()` with the schema-builder trait for new code.
    pub fn raw_migration(mut self, m: impl RawMigration + 'static) -> Self {
        self.migrations
            .push(Box::new(RawMigrationAdapter(Box::new(m))));
        self
    }

    // ── private helpers ───────────────────────────────────────────────────────

    async fn ensure_table(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS _migrations (
                id     BIGSERIAL    PRIMARY KEY,
                name   TEXT         NOT NULL UNIQUE,
                batch  INTEGER      NOT NULL DEFAULT 1,
                run_at TIMESTAMPTZ  NOT NULL DEFAULT NOW()
            )",
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn applied(&self) -> Result<Vec<String>, sqlx::Error> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT name FROM _migrations ORDER BY id")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|(n,)| n).collect())
    }

    async fn current_batch(&self) -> Result<i32, sqlx::Error> {
        let row: Option<(i32,)> = sqlx::query_as("SELECT COALESCE(MAX(batch), 0) FROM _migrations")
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|(b,)| b).unwrap_or(0))
    }

    // ── public API ────────────────────────────────────────────────────────────

    /// Detect schema version gaps: migrations that would run out of sequence.
    ///
    /// Returns a list of human-readable warning strings (empty = no gaps).
    pub async fn gap_warnings(&self) -> anyhow::Result<Vec<String>> {
        self.ensure_table().await?;
        let applied = self.applied().await?;
        if applied.is_empty() {
            return Ok(Vec::new());
        }
        let last_applied = applied.last().expect("non-empty").clone();
        let mut warnings = Vec::new();
        for m in &self.migrations {
            let name = m.name();
            if !applied.contains(&name.to_string()) && name < last_applied.as_str() {
                warnings.push(format!(
                    "Gap detected: pending migration '{name}' sorts before last applied '{last_applied}'"
                ));
            }
        }
        let registered: Vec<&str> = self.migrations.iter().map(|m| m.name()).collect();
        for name in &applied {
            if !registered.contains(&name.as_str()) {
                warnings.push(format!(
                    "Orphan detected: applied migration '{name}' is no longer registered"
                ));
            }
        }
        Ok(warnings)
    }

    /// Apply all pending migrations.
    pub async fn run(&self) -> anyhow::Result<()> {
        self.ensure_table().await?;
        for w in self.gap_warnings().await? {
            eprintln!("rok-fluent WARNING: {w}");
        }
        let schema = SchemaExecutor::new(self.pool.clone());
        let applied = self.applied().await?;
        let batch = self.current_batch().await? + 1;

        for m in &self.migrations {
            let name = m.name().to_string();
            if applied.contains(&name) {
                continue;
            }
            m.up(&schema).await?;
            sqlx::query("INSERT INTO _migrations (name, batch) VALUES ($1, $2)")
                .bind(&name)
                .bind(batch)
                .execute(&self.pool)
                .await?;
            println!("Migrated:  {name}");
        }
        Ok(())
    }

    /// Rollback the last batch of migrations.
    pub async fn rollback(&self) -> anyhow::Result<()> {
        self.ensure_table().await?;
        let schema = SchemaExecutor::new(self.pool.clone());
        let batch = self.current_batch().await?;
        if batch == 0 {
            println!("Nothing to rollback.");
            return Ok(());
        }

        let names: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM _migrations WHERE batch = $1 ORDER BY id DESC")
                .bind(batch)
                .fetch_all(&self.pool)
                .await?;

        for (name,) in names {
            if let Some(m) = self.migrations.iter().find(|m| m.name() == name) {
                m.down(&schema).await?;
                sqlx::query("DELETE FROM _migrations WHERE name = $1")
                    .bind(&name)
                    .execute(&self.pool)
                    .await?;
                println!("Rolled back: {name}");
            }
        }
        Ok(())
    }

    /// Print the status of every migration (Applied / Pending).
    pub async fn status(&self) -> anyhow::Result<()> {
        self.ensure_table().await?;
        let applied = self.applied().await?;
        for m in &self.migrations {
            let status = if applied.contains(&m.name().to_string()) {
                "Applied "
            } else {
                "Pending "
            };
            println!("[{status}] {}", m.name());
        }
        Ok(())
    }
}
