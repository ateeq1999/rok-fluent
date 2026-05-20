//! [`MigrationSource`] — abstraction over where migrations come from.
//!
//! Two built-in sources:
//!
//! - [`EmbeddedMigrations`] — SQL strings compiled into the binary via
//!   `include_str!`.  Framework crates (rok-auth, rok-lock, …) use this to
//!   ship their own table definitions alongside their code.
//!
//! - [`FileSource`] — reads all `*.sql` files from a directory at startup.
//!   Application developers point this at their `migrations/` folder.

use super::migration::RawMigration;

// ── MigrationSource trait ─────────────────────────────────────────────────────

/// A provider of ordered raw-SQL migrations.
///
/// Implement this trait to create custom migration sources.  The built-in
/// implementations ([`EmbeddedMigrations`], [`FileSource`]) cover the common
/// cases.
pub trait MigrationSource: Send + Sync {
    /// Return all migrations from this source in application order.
    fn migrations(&self) -> Vec<Box<dyn RawMigration>>;
}

// ── EmbeddedMigration ─────────────────────────────────────────────────────────

/// A single migration whose SQL is embedded at compile time.
///
/// # Example
///
/// ```rust,no_run
/// use rok_fluent::migrate::EmbeddedMigration;
///
/// const M: EmbeddedMigration = EmbeddedMigration {
///     name: "001_create_users",
///     up:   "CREATE TABLE users (id BIGSERIAL PRIMARY KEY);",
///     down: "DROP TABLE IF EXISTS users;",
/// };
/// ```
#[derive(Clone, Copy)]
pub struct EmbeddedMigration {
    /// Unique, sortable name.  Convention: `"NNN_description"`.
    pub name: &'static str,
    /// SQL to apply the migration (`CREATE TABLE …`).
    pub up: &'static str,
    /// SQL to revert the migration (`DROP TABLE …`).  May be empty.
    pub down: &'static str,
}

impl RawMigration for EmbeddedMigration {
    fn name(&self) -> &str {
        self.name
    }
    fn up(&self) -> String {
        self.up.to_string()
    }
    fn down(&self) -> String {
        self.down.to_string()
    }
}

// ── EmbeddedMigrations ────────────────────────────────────────────────────────

/// An ordered, compile-time set of [`EmbeddedMigration`]s.
///
/// Framework crates expose a `pub fn migrations() -> EmbeddedMigrations`
/// function so callers can register their migrations with the runner.
pub struct EmbeddedMigrations {
    inner: &'static [EmbeddedMigration],
}

impl EmbeddedMigrations {
    /// Construct from a static slice.
    pub const fn new(inner: &'static [EmbeddedMigration]) -> Self {
        Self { inner }
    }
}

impl MigrationSource for EmbeddedMigrations {
    fn migrations(&self) -> Vec<Box<dyn RawMigration>> {
        self.inner
            .iter()
            .map(|m| Box::new(*m) as Box<dyn RawMigration>)
            .collect()
    }
}

// ── FileSource ────────────────────────────────────────────────────────────────

/// A [`MigrationSource`] that reads `*.sql` files from a directory at startup.
///
/// Files are sorted alphabetically, so timestamp-prefixed names
/// (`20260502_create_posts.sql`) apply in the correct order.
/// Files whose names contain `.down.` are ignored.
///
/// # Example
///
/// ```rust,ignore
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
pub struct FileSource {
    dir: std::path::PathBuf,
}

impl FileSource {
    /// Create a new `FileSource` pointing at the given directory.
    pub fn new(dir: impl Into<std::path::PathBuf>) -> Self {
        Self { dir: dir.into() }
    }
}

struct FileMigration {
    name: String,
    up: String,
    down: String,
}

impl RawMigration for FileMigration {
    fn name(&self) -> &str {
        &self.name
    }
    fn up(&self) -> String {
        self.up.clone()
    }
    fn down(&self) -> String {
        self.down.clone()
    }
}

fn parse_up_down(content: &str) -> (String, String) {
    if let Some(up_pos) = content.find("-- up\n") {
        let after_up = &content[up_pos + 6..];
        if let Some(down_pos) = after_up.find("-- down\n") {
            return (
                after_up[..down_pos].trim().to_string(),
                after_up[down_pos + 8..].trim().to_string(),
            );
        }
        return (after_up.trim().to_string(), String::new());
    }
    (content.trim().to_string(), String::new())
}

impl MigrationSource for FileSource {
    fn migrations(&self) -> Vec<Box<dyn RawMigration>> {
        let mut entries = match std::fs::read_dir(&self.dir) {
            Ok(rd) => rd
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let p = e.path();
                    p.extension().and_then(|s| s.to_str()) == Some("sql")
                        && !p
                            .file_name()
                            .and_then(|s| s.to_str())
                            .map(|s| s.contains(".down."))
                            .unwrap_or(false)
                })
                .collect::<Vec<_>>(),
            Err(_) => return Vec::new(),
        };

        entries.sort_by_key(|e| e.file_name());

        entries
            .into_iter()
            .filter_map(|e| {
                let path = e.path();
                let name = path.file_stem()?.to_str()?.to_string();
                let content = std::fs::read_to_string(&path).ok()?;
                let (up, down) = parse_up_down(&content);
                Some(Box::new(FileMigration { name, up, down }) as Box<dyn RawMigration>)
            })
            .collect()
    }
}
