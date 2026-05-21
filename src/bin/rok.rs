//! `rok` — rok-fluent database CLI.
//!
//! Build: `cargo build --features cli --bin rok`
//!
//! Reads `DATABASE_URL` from the environment.
//!
//! ```text
//! rok db migrate   [--dir migrations]
//! rok db rollback  [--dir migrations]
//! rok db status    [--dir migrations]
//! rok db make <name> [--dir migrations]
//! rok db seed
//! rok db schema dump
//! rok db schema diff [--dir migrations]
//! ```

use std::fs;
use std::path::Path;

use clap::{Parser, Subcommand};
use rok_fluent::migrate::{FileSource, MigrationRunner};
use rok_fluent::services::SchemaInspector;

// ── CLI shape ─────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "rok", about = "rok-fluent database tooling", version)]
struct Cli {
    #[command(subcommand)]
    command: TopCommand,
}

#[derive(Subcommand)]
enum TopCommand {
    /// Database commands: migrate, rollback, status, make, seed, schema
    Db {
        #[command(subcommand)]
        subcommand: DbCommand,
    },
}

#[derive(Subcommand)]
enum DbCommand {
    /// Run all pending migrations
    Migrate {
        #[arg(long, default_value = "migrations")]
        dir: String,
    },
    /// Roll back the last batch of migrations
    Rollback {
        #[arg(long, default_value = "migrations")]
        dir: String,
    },
    /// Show migration status (Applied / Pending)
    Status {
        #[arg(long, default_value = "migrations")]
        dir: String,
    },
    /// Create a new timestamped migration file
    Make {
        /// Descriptive name, e.g. "create_users_table"
        name: String,
        #[arg(long, default_value = "migrations")]
        dir: String,
    },
    /// Print guidance for running seeders
    Seed,
    /// Schema inspection commands
    Schema {
        #[command(subcommand)]
        subcommand: SchemaCommand,
    },
}

#[derive(Subcommand)]
enum SchemaCommand {
    /// Dump the live database schema as approximate CREATE TABLE DDL
    Dump,
    /// Compare migration files on disk to the applied _migrations table
    Diff {
        #[arg(long, default_value = "migrations")]
        dir: String,
    },
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() -> anyhow::Result<()> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build runtime: {e}"))?
        .block_on(run())
}

async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        TopCommand::Db { subcommand } => handle_db(subcommand).await,
    }
}

async fn handle_db(cmd: DbCommand) -> anyhow::Result<()> {
    match cmd {
        DbCommand::Migrate { dir } => {
            let pool = connect().await?;
            make_runner(pool, &dir).run().await?;
            println!("All migrations complete.");
        }
        DbCommand::Rollback { dir } => {
            let pool = connect().await?;
            make_runner(pool, &dir).rollback().await?;
        }
        DbCommand::Status { dir } => {
            let pool = connect().await?;
            make_runner(pool, &dir).status().await?;
        }
        DbCommand::Make { name, dir } => {
            make_migration(&name, &dir)?;
        }
        DbCommand::Seed => {
            eprintln!(
                "rok db seed: seeders must be registered in code.\n\
                 Use MigrationRunner::migration() to add typed Rust migrations/seeders.\n\
                 See docs/guides/migrations.md for the pattern."
            );
            std::process::exit(1);
        }
        DbCommand::Schema { subcommand } => match subcommand {
            SchemaCommand::Dump => {
                let pool = connect().await?;
                schema_dump(&pool).await?;
            }
            SchemaCommand::Diff { dir } => {
                let pool = connect().await?;
                schema_diff(&pool, &dir).await?;
            }
        },
    }
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn connect() -> anyhow::Result<sqlx::PgPool> {
    let url = std::env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL environment variable is not set"))?;
    sqlx::PgPool::connect(&url)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect: {e}"))
}

fn make_runner(pool: sqlx::PgPool, dir: &str) -> MigrationRunner {
    MigrationRunner::new(pool).source(FileSource::new(dir))
}

fn make_migration(name: &str, dir: &str) -> anyhow::Result<()> {
    let ts = chrono::Utc::now().format("%Y%m%d%H%M%S");
    let filename = format!("{ts}_{name}.sql");
    let path = Path::new(dir).join(&filename);
    fs::create_dir_all(dir).map_err(|e| anyhow::anyhow!("Cannot create directory '{dir}': {e}"))?;
    fs::write(
        &path,
        format!("-- up\n\n-- {name}\n\n\n-- down\n\n-- DROP TABLE IF EXISTS ...;\n"),
    )
    .map_err(|e| anyhow::anyhow!("Cannot write '{filename}': {e}"))?;
    println!("Created: {}", path.display());
    Ok(())
}

// ── Schema dump ───────────────────────────────────────────────────────────────

async fn schema_dump(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    let tables: Vec<(String,)> = sqlx::query_as(
        "SELECT table_name FROM information_schema.tables \
         WHERE table_schema = 'public' AND table_type = 'BASE TABLE' \
         AND table_name NOT IN ('_migrations') ORDER BY table_name",
    )
    .fetch_all(pool)
    .await?;

    if tables.is_empty() {
        println!("-- No user tables found in schema 'public'.");
        return Ok(());
    }

    println!("-- rok db schema dump  (approximate DDL — not for round-trip use)");

    for (table,) in &tables {
        let cols = SchemaInspector::columns(table, pool).await?;

        let pk_cols: Vec<&str> = cols
            .iter()
            .filter(|c| c.is_pk)
            .map(|c| c.name.as_str())
            .collect();

        println!("\nCREATE TABLE \"{table}\" (");
        let n = cols.len();
        for (i, col) in cols.iter().enumerate() {
            let pk_s = if pk_cols.len() == 1 && pk_cols.contains(&col.name.as_str()) {
                " PRIMARY KEY"
            } else {
                ""
            };
            let null_s = if col.nullable { "" } else { " NOT NULL" };
            let def_s = col
                .default
                .as_deref()
                .map(|d| format!(" DEFAULT {d}"))
                .unwrap_or_default();
            let comma = if i + 1 < n || pk_cols.len() > 1 {
                ","
            } else {
                ""
            };
            println!(
                "    \"{}\" {}{}{}{}{}",
                col.name, col.data_type, null_s, def_s, pk_s, comma
            );
        }
        if pk_cols.len() > 1 {
            let pks = pk_cols
                .iter()
                .map(|c| format!("\"{c}\""))
                .collect::<Vec<_>>()
                .join(", ");
            println!("    PRIMARY KEY ({pks})");
        }
        println!(");");
    }
    Ok(())
}

// ── Schema diff ───────────────────────────────────────────────────────────────

async fn schema_diff(pool: &sqlx::PgPool, dir: &str) -> anyhow::Result<()> {
    // Applied migrations — gracefully handle missing _migrations table
    let applied: Vec<String> =
        sqlx::query_as::<_, (String,)>("SELECT name FROM _migrations ORDER BY id")
            .fetch_all(pool)
            .await
            .map(|rows| rows.into_iter().map(|(n,)| n).collect())
            .unwrap_or_default();

    // Migration files on disk
    let dir_path = Path::new(dir);
    if !dir_path.exists() {
        eprintln!("Migration directory '{dir}' does not exist — no files to compare.");
        return Ok(());
    }

    let mut files: Vec<String> = fs::read_dir(dir_path)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "sql").unwrap_or(false))
        .filter_map(|e| {
            e.path()
                .file_stem()
                .and_then(|s| s.to_str())
                .map(str::to_owned)
        })
        .collect();
    files.sort();

    println!("{:<10} Migration", "Status");
    println!("{}", "-".repeat(60));

    for name in &files {
        let status = if applied.contains(name) {
            "Applied"
        } else {
            "Pending"
        };
        println!("{status:<10} {name}");
    }

    // Orphans: in DB but no matching file
    for name in &applied {
        if !files.contains(name) {
            println!("{:<10} {name}", "Orphan");
        }
    }

    Ok(())
}
