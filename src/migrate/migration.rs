//! Migration traits — raw SQL (backward-compat) and async schema-builder.

// ── RawMigration ──────────────────────────────────────────────────────────────

/// A raw-SQL migration that returns SQL strings synchronously.
///
/// Used internally by [`EmbeddedMigration`], [`FileSource`], and the
/// `.raw_migration()` runner method.  Framework crates that ship their own
/// tables embed their SQL via this trait.
///
/// [`EmbeddedMigration`]: crate::migrate::EmbeddedMigration
/// [`FileSource`]: crate::migrate::FileSource
pub trait RawMigration: Send + Sync + 'static {
    /// A unique, sortable identifier for this migration.
    ///
    /// Convention: `"YYYY_MM_DD_HHMMSS_description"`.
    fn name(&self) -> &str;

    /// SQL to apply this migration (`CREATE TABLE …`, etc.).
    fn up(&self) -> String;

    /// SQL to rollback this migration (`DROP TABLE …`, etc.).
    fn down(&self) -> String;
}

// ── Migration (async schema-builder) ─────────────────────────────────────────

/// A schema-builder migration — async, receives a live [`SchemaExecutor`].
///
/// This is the preferred API for application migrations.  Use the typed DSL
/// in `up` to describe what to create and `down` to reverse it.
///
/// # Example
///
/// ```rust,ignore
/// # use rok_fluent::migrate::{Migration, MigrationRunner, SchemaExecutor};
/// use async_trait::async_trait;
///
/// pub struct CreateUsersTable;
///
/// #[async_trait]
/// impl Migration for CreateUsersTable {
///     fn name(&self) -> &str { "2026_05_18_000001_create_users_table" }
///
///     async fn up(&self, schema: &SchemaExecutor) -> anyhow::Result<()> {
///         schema.create("users", |t: &mut rok_fluent::migrate::TableBuilder| {
///             t.id();
///             t.string("email").not_null().unique();
///             t.string("name").not_null();
///             t.timestamps();
///         }).await
///     }
///
///     async fn down(&self, schema: &SchemaExecutor) -> anyhow::Result<()> {
///         schema.drop_table_if_exists("users").await
///     }
/// }
/// ```
///
/// [`SchemaExecutor`]: crate::migrate::schema::SchemaExecutor
#[cfg(feature = "postgres")]
#[async_trait::async_trait]
pub trait Migration: Send + Sync + 'static {
    /// A unique, sortable identifier for this migration.
    ///
    /// Convention: `"YYYY_MM_DD_HHMMSS_description"`.
    fn name(&self) -> &str;

    /// Apply this migration — called by the runner when the migration is pending.
    async fn up(&self, schema: &super::schema::SchemaExecutor) -> anyhow::Result<()>;

    /// Rollback this migration — called by the runner during `rok db:rollback`.
    async fn down(&self, schema: &super::schema::SchemaExecutor) -> anyhow::Result<()>;
}
