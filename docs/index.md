# rok-fluent

Eloquent-inspired async ORM for Rust. Single crate, multi-database, feature-gated.

```toml
[dependencies]
rok-fluent = { version = "0.4", features = ["postgres", "macros"] }
```

## Quick Links

| Topic | Document |
|-------|----------|
| Install & first query | [Getting Started](getting-started.md) |
| All feature flags | [Features](features.md) |
| Module map & design | [Architecture](architecture.md) |
| Version history | [Changelog](changelog.md) |
| Core traits API | [docs/api/core.md](api/core.md) |
| ORM runtime API | [docs/api/orm.md](api/orm.md) |
| PostgreSQL | [docs/api/postgres.md](api/postgres.md) |
| MySQL | [docs/api/mysql.md](api/mysql.md) |
| SQLite | [docs/api/sqlite.md](api/sqlite.md) |
| Migrations | [docs/api/migrate.md](api/migrate.md) |
| Test factories | [docs/api/factory.md](api/factory.md) |
| Writing migrations | [docs/guides/migrations.md](guides/migrations.md) |
| Testing with factories | [docs/guides/testing.md](guides/testing.md) |
| Axum integration | [docs/guides/axum.md](guides/axum.md) |
| Multi-tenancy | [docs/guides/multi-tenancy.md](guides/multi-tenancy.md) |

## Feature Matrix

| Feature flag | What it enables | Extra deps |
|---|---|---|
| `default` | `macros` | — |
| `macros` | `#[derive(Model, Resource, Seed)]` | `rok-fluent-macros` |
| `postgres` | PostgreSQL executor, pool, transactions | `sqlx/postgres`, `tokio`, `dashmap` |
| `sqlite` | SQLite executor | `sqlx/sqlite`, `tokio` |
| `mysql` | MySQL executor | `sqlx/mysql`, `tokio` |
| `axum` | `OrmLayer` Tower middleware | `axum`, `tower` (implies `postgres`) |
| `tracing` | OpenTelemetry query spans | `tracing` |
| `metrics` | Prometheus metrics | `metrics` |
| `tenant` | `TenantLayer` multi-tenant context | `tower`, `http`, `tokio` |
| `replica` | Read-replica routing | — |
| `factory` | `Factory` trait, `FactoryBuilder<T>`, `Faker` | — |
| `factory-postgres` | DB-backed factory creation | `postgres` |
| `migrate` | `MigrationRunner`, `Schema` builder | `async-trait`, `anyhow` |
| `migrate-postgres` | PostgreSQL migration runner | `migrate` + `postgres` |
| `migrate-sqlite` | SQLite migration runner | `migrate` + `sqlite` |
| `migrate-mysql` | MySQL migration runner | `migrate` + `mysql` |
| `full` | Everything above | all |

## Supported Databases

| Database | Minimum version | Feature flag |
|---|---|---|
| PostgreSQL | 13 | `postgres` |
| MySQL | 8.0 | `mysql` |
| SQLite | 3.35 | `sqlite` |

## License

MIT OR Apache-2.0
