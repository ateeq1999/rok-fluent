# rok-fluent

Async ORM for Rust built on SQLx. Single crate, multi-database, two query styles, feature-gated.

```toml
[dependencies]
rok-fluent = { version = "0.4", features = ["active", "query", "postgres"] }
```

## Quick Links

| Topic | Document |
|-------|----------|
| Install & first query | [Getting Started](getting-started.md) |
| All feature flags | [Features](features.md) |
| Typed DSL API | [docs/api/dsl.md](api/dsl.md) |
| Active Record / ORM API | [docs/api/orm.md](api/orm.md) |
| Core traits API | [docs/api/core.md](api/core.md) |
| PostgreSQL | [docs/api/postgres.md](api/postgres.md) |
| MySQL | [docs/api/mysql.md](api/mysql.md) |
| SQLite | [docs/api/sqlite.md](api/sqlite.md) |
| Migrations | [docs/api/migrate.md](api/migrate.md) |
| Test factories | [docs/api/factory.md](api/factory.md) |
| Module map & design | [Architecture](architecture.md) |
| Version history | [Changelog](changelog.md) |
| Writing migrations | [docs/guides/migrations.md](guides/migrations.md) |
| Testing with factories | [docs/guides/testing.md](guides/testing.md) |
| Axum integration | [docs/guides/axum.md](guides/axum.md) |
| Multi-tenancy | [docs/guides/multi-tenancy.md](guides/multi-tenancy.md) |
| Debugging queries | [docs/guides/debugging.md](guides/debugging.md) |
| Transactions & savepoints | [docs/guides/transactions.md](guides/transactions.md) *(planned)* |
| Row-level & advisory locking | [docs/guides/locking.md](guides/locking.md) *(planned)* |
| Performance tuning | [docs/guides/performance.md](guides/performance.md) |

## Feature Matrix

| Feature flag | What it enables | Extra deps |
|---|---|---|
| `default` | `macros` | — |
| `macros` | `#[derive(Model, Table, Resource, Seed)]`, `query!` | `rok-fluent-macros` |
| **`active`** | **Active Record** — `ModelQuery`, `PgModel`, `CrudService`, `FilterBuilder`, `SortBuilder`, `BatchService`, `SoftDeleteService`*, `SearchService`*, `AuditService`*, `TransactionService`*, `LockService`* | — |
| **`query`** | **Typed DSL** — `SelectBuilder` with JOINs/CTEs/aggregates/set-ops, `InsertBuilder`, `UpdateBuilder`, `DeleteBuilder`, `Column<T,V>`, `Expr`, `AggExpr`, `FnExpr`, `CaseExpr`, `Loaded<T>` | — |
| `postgres` | PostgreSQL executor, pool, transactions | `sqlx/postgres`, `tokio`, `dashmap` |
| `sqlite` | SQLite executor | `sqlx/sqlite`, `tokio` |
| `mysql` | MySQL executor | `sqlx/mysql`, `tokio` |
| `axum` | `OrmLayer` Tower middleware | `axum`, `tower` (implies `postgres`) |
| `tracing` | OpenTelemetry query spans + `QueryLog`* | `tracing` |
| `metrics` | Prometheus metrics | `metrics` |
| `tenant` | `TenantLayer` multi-tenant context | `tower`, `http`, `tokio` |
| `replica` | Read-replica routing | — |
| `cli`* | `rok db` CLI tool | `clap` |
| `factory` | `Factory` trait, `FactoryBuilder<T>`, `Faker` | — |
| `factory-postgres` | DB-backed factory creation | `postgres` |
| `migrate` | `MigrationRunner`, `Schema` builder | `async-trait`, `anyhow` |
| `migrate-postgres` | PostgreSQL migration runner | `migrate` + `postgres` |
| `migrate-sqlite` | SQLite migration runner | `migrate` + `sqlite` |
| `migrate-mysql` | MySQL migration runner | `migrate` + `mysql` |
| `full` | Everything above | all |

*\* = planned, not yet released*

## Query Styles

### Typed DSL (`query` feature) — OOP-natural, fully type-checked

```rust
let users: Vec<User> = db::select()
    .from(User::table())
    .where_(User::EMAIL.like("%@example.com").and(User::ID.gt(0_i64)))
    .order_by(User::NAME.asc())
    .limit(25)
    .fetch_all(&pool).await?;
```

### Active Record (`active` feature) — model-centric, expressive scopes

```rust
let users: Vec<User> = User::query()
    .where_like("email", "%@example.com")
    .order_by("name")
    .limit(25)
    .get().await?;
```

## Supported Databases

| Database | Minimum version | Feature flag |
|---|---|---|
| PostgreSQL | 13 | `postgres` |
| MySQL | 8.0 | `mysql` |
| SQLite | 3.35 | `sqlite` |

## License

MIT
