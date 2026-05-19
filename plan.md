# rok-fluent Consolidation Plan

## Goal

Collapse five separate crates (`rok-orm`, `rok-orm-core`, `rok-orm-macros`, `rok-orm-factory`,
`rok-orm-migrate`) into a single published crate named **`rok-fluent`**. All source lives under
`src/`. Optional subsystems are gated by Cargo feature flags — the same pattern used by `tokio`
(runtime/macros/net/fs/…) and `serde` (derive/alloc/std/…).

## Constraint: Proc-Macro Crate

Rust requires `proc-macro = true` crates to be standalone. `rok-orm-macros` **cannot** be inlined
into `src/`. Strategy (identical to `tokio-macros` / `serde_derive`):

- Rename `rok-orm-macros` → **`rok-fluent-macros`** (internal implementation detail, lives at
  `rok-fluent-macros/`).
- Users never depend on it directly.
- `rok-fluent` re-exports its items when the `macros` feature is on.

All other crates dissolve completely into `src/`.

---

## Target Directory Layout

```
rok-fluent/                      ← repo root
├── Cargo.toml                   ← single lib crate (replaces placeholder binary)
├── Cargo.lock
├── rok-fluent-macros/           ← proc-macro impl (internal, not user-facing)
│   ├── Cargo.toml
│   └── src/lib.rs
├── src/
│   ├── lib.rs                   ← public API & feature-gated re-exports
│   │
│   ├── core/                    ← rok-orm-core absorbed here
│   │   ├── mod.rs
│   │   ├── condition.rs
│   │   ├── model.rs
│   │   ├── query.rs
│   │   ├── replica.rs           ← cfg(feature = "replica")
│   │   ├── schema_cache.rs
│   │   ├── tenant.rs            ← cfg(feature = "tenant")
│   │   ├── sqlx_pg.rs           ← cfg(feature = "postgres")
│   │   ├── sqlx_sqlite.rs       ← cfg(feature = "sqlite")
│   │   └── sqlx_mysql.rs        ← cfg(feature = "mysql")
│   │
│   ├── orm/                     ← rok-orm absorbed here
│   │   ├── mod.rs
│   │   ├── casts.rs
│   │   ├── collection.rs
│   │   ├── eager.rs
│   │   ├── hooks.rs
│   │   ├── model_query.rs
│   │   ├── morph.rs
│   │   ├── n1.rs
│   │   ├── orm_layer.rs         ← cfg(feature = "axum")
│   │   ├── pagination.rs
│   │   ├── resource.rs
│   │   ├── scopes.rs
│   │   ├── through.rs
│   │   ├── postgres/            ← cfg(feature = "postgres")
│   │   │   ├── mod.rs
│   │   │   ├── executor.rs
│   │   │   ├── model.rs         (was pg_model.rs)
│   │   │   ├── pool.rs
│   │   │   ├── query_log.rs
│   │   │   ├── transaction.rs
│   │   │   └── pivot_query.rs
│   │   ├── mysql/               ← cfg(feature = "mysql")
│   │   │   ├── mod.rs
│   │   │   ├── executor.rs
│   │   │   └── model.rs         (was mysql_model.rs)
│   │   └── sqlite/              ← cfg(feature = "sqlite")
│   │       ├── mod.rs
│   │       ├── executor.rs
│   │       └── model.rs         (was sqlite_model.rs)
│   │
│   ├── factory/                 ← rok-orm-factory absorbed here
│   │   ├── mod.rs               ← cfg(feature = "factory")
│   │   └── faker.rs
│   │
│   └── migrate/                 ← rok-orm-migrate absorbed here
│       ├── mod.rs               ← cfg(feature = "migrate")
│       ├── migration.rs
│       ├── runner.rs
│       ├── schema.rs
│       ├── source.rs
│       └── table.rs
│
└── tests/
    ├── integration.rs
    └── pg_integration.rs
```

---

## Feature Flag Taxonomy

Mirrors tokio's style: small orthogonal flags that compose, with "full" convenience bundles.

```toml
[features]
# ── Defaults ────────────────────────────────────────────────────────────────
default = ["macros"]

# ── Proc-macro derive support ───────────────────────────────────────────────
macros = ["dep:rok-fluent-macros"]

# ── Database backends (mutually-usable, pick one or more) ───────────────────
postgres = ["dep:sqlx", "sqlx/postgres", "dep:tokio", "tokio/rt", "dep:futures-core",
            "dep:futures", "dep:dashmap", "dep:once_cell"]
sqlite   = ["dep:sqlx", "sqlx/sqlite",  "dep:tokio", "tokio/rt"]
mysql    = ["dep:sqlx", "sqlx/mysql",   "dep:tokio", "tokio/rt"]

# ── Web / tower integration ──────────────────────────────────────────────────
axum     = ["dep:axum", "dep:tower", "postgres"]   # implies postgres

# ── Observability ────────────────────────────────────────────────────────────
tracing  = ["dep:tracing"]
metrics  = ["dep:metrics"]

# ── Multi-tenancy ────────────────────────────────────────────────────────────
tenant   = ["dep:tower", "dep:http", "dep:tokio", "tokio/rt"]

# ── Read-replica strategies ──────────────────────────────────────────────────
replica  = []                                        # no extra deps needed

# ── Test utilities ───────────────────────────────────────────────────────────
factory           = []
factory-postgres  = ["factory", "postgres"]

# ── Migrations ───────────────────────────────────────────────────────────────
migrate           = ["dep:async-trait", "dep:anyhow"]
migrate-postgres  = ["migrate", "postgres"]
migrate-sqlite    = ["migrate", "sqlite"]
migrate-mysql     = ["migrate", "mysql"]

# ── Convenience bundles (like tokio's "full") ─────────────────────────────────
full = [
  "macros", "postgres", "axum", "tracing", "metrics",
  "tenant", "replica", "factory-postgres",
  "migrate-postgres",
]
```

### Mapping: old crate features → new flags

| Old | New |
|-----|-----|
| `rok-orm/default` | `rok-fluent/default` (= macros) |
| `rok-orm/macros` | `rok-fluent/macros` |
| `rok-orm/postgres` | `rok-fluent/postgres` |
| `rok-orm/sqlite` | `rok-fluent/sqlite` |
| `rok-orm/mysql` | `rok-fluent/mysql` |
| `rok-orm/axum` | `rok-fluent/axum` |
| `rok-orm/tracing` | `rok-fluent/tracing` |
| `rok-orm/metrics` | `rok-fluent/metrics` |
| `rok-orm-core/sqlx-postgres` | `rok-fluent/postgres` |
| `rok-orm-core/sqlx-sqlite` | `rok-fluent/sqlite` |
| `rok-orm-core/sqlx-mysql` | `rok-fluent/mysql` |
| `rok-orm-core/tenant` | `rok-fluent/tenant` |
| `rok-orm-factory/postgres` | `rok-fluent/factory-postgres` |
| `rok-orm-migrate/postgres` | `rok-fluent/migrate-postgres` |
| `rok-orm-migrate/sqlite` | `rok-fluent/migrate-sqlite` |
| `rok-orm-migrate/mysql` | `rok-fluent/migrate-mysql` |

---

## Public API Surface in `src/lib.rs`

```rust
// Always available — core traits
pub use crate::core::{Condition, Dialect, Join, JoinOp, Model, OrderDir, QueryBuilder, SqlValue};

// Always available — ORM runtime
pub use crate::orm::{
    collection, eager, hooks, morph, n1, pagination, scopes, through, casts,
};

// Macros feature
#[cfg(feature = "macros")]
pub use rok_fluent_macros::{query, Model, Resource, Seed};

// Database executors
#[cfg(feature = "postgres")] pub use crate::orm::postgres;
#[cfg(feature = "mysql")]    pub use crate::orm::mysql;
#[cfg(feature = "sqlite")]   pub use crate::orm::sqlite;

// Optional subsystems
#[cfg(feature = "axum")]    pub use crate::orm::orm_layer;
#[cfg(feature = "tenant")]  pub use crate::core::tenant;
#[cfg(feature = "replica")] pub use crate::core::replica;
#[cfg(feature = "factory")] pub use crate::factory;
#[cfg(feature = "migrate")] pub use crate::migrate;
```

---

## Cargo.toml Design

```toml
[package]
name    = "rok-fluent"
version = "0.4.0"           # major bump — consolidation is a breaking change
edition = "2021"
description = "Eloquent-inspired async ORM for Rust (PostgreSQL, MySQL, SQLite)"
# ... license, repo, homepage, keywords, categories

[lib]
name = "rok_fluent"
path = "src/lib.rs"

[features]
# (see Feature Flag Taxonomy above)

[dependencies]
# always-on
thiserror  = "2"
serde      = { version = "1", features = ["derive"] }
serde_json = "1"
base64     = "0.22"
chrono     = { version = "0.4", features = ["serde"] }
uuid       = { version = "1", features = ["v4", "serde"] }
once_cell  = "1"
rand       = "0.8"

# feature-gated
rok-fluent-macros = { version = "0.4", path = "rok-fluent-macros", optional = true }
sqlx        = { version = "0.8", optional = true, default-features = false,
                features = ["macros", "chrono", "uuid", "json"] }
tokio       = { version = "1", optional = true, default-features = false }
futures-core = { version = "0.3", optional = true }
futures      = { version = "0.3", optional = true }
dashmap      = { version = "6", optional = true }
axum         = { version = "0.8", optional = true }
tower        = { version = "0.5", optional = true }
http         = { version = "1",   optional = true }
tracing      = { version = "0.1", optional = true }
metrics      = { version = "0.24", optional = true }
async-trait  = { version = "0.1", optional = true }
anyhow       = { version = "1",   optional = true }

[dev-dependencies]
tokio = { version = "1", features = ["full"] }
```

---

## Version Strategy

- Bump to **`0.4.0`** — the consolidation is a breaking structural change.
- `rok-fluent-macros` bumps in lockstep (always same version as `rok-fluent`).
- Old crates (`rok-orm`, `rok-orm-core`, etc.) publish a final **`0.3.x`** tombstone release
  pointing users to `rok-fluent = "0.4"`.

---

## Migration Path for Existing Users

```toml
# Before
[dependencies]
rok-orm      = { version = "0.3", features = ["postgres", "macros"] }
rok-orm-migrate = { version = "0.3", features = ["postgres"] }

# After
[dependencies]
rok-fluent = { version = "0.4", features = ["postgres", "macros", "migrate-postgres"] }
```

Rust `use` paths change from `rok_orm::` / `rok_orm_core::` to `rok_fluent::`.
A `#[deprecated]` re-export shim can ease the transition if needed.

---

## Risks & Trade-offs

| Risk | Mitigation |
|------|-----------|
| Compile times increase even when only core is needed | Feature flags ensure zero-cost opt-in; `default = ["macros"]` stays minimal |
| Proc-macro crate still separate | Users never see it; it is not published independently |
| `migrate` pulls `anyhow` into the tree | `anyhow` is optional; only activated with migrate features |
| Breaking rename of `rok_orm::` → `rok_fluent::` | Tombstone releases + semver bump signals intent |
| Tests live only under `rok-orm/tests/` today | Move to `tests/` at repo root; keep as integration tests |
