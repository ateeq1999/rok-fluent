# rok-fluent — Project Rules

This file is the authoritative project system prompt for Claude Code. Every session in this
directory inherits these rules automatically.

## Project Context

**rok-fluent** is a single-crate async ORM for Rust targeting PostgreSQL, MySQL, and SQLite via
SQLx. The codebase is being consolidated from five separate crates into one (see `plan.md`). All
source lives under `src/`. Optional subsystems are gated by Cargo feature flags, following the
same pattern as `tokio` and `serde`.

Internal proc-macro crate: `rok-fluent-macros/` (never published as a standalone user dependency).

---

## Rust Code Standards

### Correctness
- **No `unwrap()` or `expect()`** in library code. Use `?` propagation or map errors explicitly.
- **No `todo!()` or `unimplemented!()`** in committed code — stubs block compilation with a proper
  `compile_error!()` if needed.
- **No `unsafe`** without a `// SAFETY:` comment explaining every invariant being upheld.
- **No integer casts with `as`** that can truncate silently — use `TryFrom`/`TryInto`.
- **All public `Result`-returning functions** must use a crate-level error type from `thiserror`,
  never `Box<dyn std::error::Error>` in public signatures.

### Ownership & Borrowing
- Accept `&str` not `String` in function parameters unless ownership is required.
- Accept `impl AsRef<str>` when a flexible string input is needed.
- Prefer **`Cow<'_, str>`** over cloning strings for values that are sometimes owned, sometimes borrowed.
- Return `impl Trait` from `pub fn` when the concrete type is an implementation detail.

### Async
- Use `async fn` not `impl Future<Output = …>` unless the return type needs to be named/stored.
- Never `.await` inside `Iterator::map` — collect first or use `futures::future::try_join_all`.
- Tag public async trait methods with `+ Send + Sync` bounds; prefer `async_trait` only where
  `async fn in trait` (RPITIT) cannot be used yet.

### Traits & Generics
- Add `#[must_use]` to all builder methods (they return `Self` and silently dropping them is a bug).
- Add `#[must_use]` to all `Result` and `Option` returning functions.
- Derive `Debug` on every public struct and enum.
- Do not derive `Default` unless the zero-value is semantically meaningful.

### Feature Gating
- Any item that depends on an optional dependency **must** be wrapped in
  `#[cfg(feature = "...")]`. Failing to gate causes compile errors for users who don't enable
  that feature.
- Use `#[cfg(any(feature = "postgres", feature = "sqlite", feature = "mysql"))]` when an item
  is needed by any database backend.
- Document every feature flag in `docs/features.md` and in a comment above the `[features]` table
  in `Cargo.toml`.

### Documentation
- Every `pub` struct, enum, trait, function, and type alias must have a `///` doc comment.
- Every module (`mod.rs` / `lib.rs`) must have a `//!` module-level doc comment.
- Code examples in doc comments must compile. Use `# use rok_fluent::*;` preambles to make them
  self-contained. Use `no_run` only for examples that require a live database.
- Never write comments that describe **what** the code does — only **why** (non-obvious
  invariants, workarounds, constraints).

### Style
- No `#[allow(dead_code)]`, `#[allow(unused_*)]`, or `#[allow(clippy::...)]` without a comment
  that names the specific reason.
- Imports: `std` → blank line → external crates → blank line → `crate::` / `super::`.
- No wildcard imports (`use foo::*`) in library code; only in `#[cfg(test)]` test modules.

---

## Quality Gates (must all pass before every commit)

```sh
# 1. Format
cargo fmt --all

# 2. Lint — zero warnings allowed
cargo clippy --workspace --all-features -- -D warnings

# 3. Test
cargo test --workspace --all-features

# 4. Feature matrix spot-check
cargo check --no-default-features
cargo check --features postgres
cargo check --features sqlite
cargo check --features mysql
cargo check --features "migrate-postgres,factory-postgres,axum,tracing,metrics,tenant"
```

If `cargo clippy` emits a warning that cannot be fixed (e.g., a known upstream issue), suppress it
at the call site with `#[allow(clippy::specific_lint)]` **and** a comment linking to the issue.

---

## Workflow (follow in this exact order after any code change)

### 1. Format
```sh
cargo fmt --all
```

### 2. Lint & fix
```sh
cargo clippy --workspace --all-features -- -D warnings
```
Fix every warning before proceeding.

### 3. Test
```sh
cargo test --workspace
```

### 4. Commit
Use [Conventional Commits](https://www.conventionalcommits.org/) format:

```
<type>(<scope>): <short summary in imperative mood>

[optional body: why, not what]

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
```

**Types:** `feat` | `fix` | `refactor` | `docs` | `test` | `chore` | `perf`

**Scopes:** `core` | `orm` | `postgres` | `mysql` | `sqlite` | `migrate` | `factory` | `macros`
| `axum` | `tenant` | `replica`

Examples:
```
feat(postgres): add connection retry with exponential backoff
fix(migrate): deduplicate migration version check
refactor(core): move sqlx adapters under core/sqlx/ submodule
docs(features): document tenant and replica feature flags
```

Stage only the relevant files (`git add -p`, never `git add .` blindly). Never commit
`.env` files, secrets, or `target/`.

### 5. Update docs
After **any** change to public API, feature flags, or behavior:

- API change → update or create `docs/api/<module>.md`
- Feature flag added/removed → update `docs/features.md`
- Breaking change → add entry to `docs/changelog.md`
- New guide-worthy behavior → add or update `docs/guides/<topic>.md`

Docs live in `docs/` and are always **Markdown**. Follow the structure in `docs/index.md`.

### 6. Publish (after version bump)
Publish in dependency order — macros crate first, then the main crate:

```sh
# 1. Bump version in Cargo.toml (both rok-fluent and rok-fluent-macros to same version)
# 2. Update docs/changelog.md with the release entry
# 3. Commit: chore: release v0.x.y
# 4. Tag: git tag -s v0.x.y -m "Release v0.x.y"

cargo publish -p rok-fluent-macros
# wait for crates.io to index (usually ~30 s)
cargo publish -p rok-fluent
```

Only publish from a **clean working tree** on the `main` branch. Run all quality gates first.
Never `--no-verify` and never force-push after a release tag.

---

## docs/ Structure

```
docs/
  index.md              project overview, feature table, quick links
  getting-started.md    installation, feature selection, first query
  features.md           every feature flag, what it enables, examples
  architecture.md       module map, dependency graph, design decisions
  changelog.md          version history and migration notes
  api/
    core.md             Model, QueryBuilder, Condition, Dialect, SqlValue
    orm.md              CRUD, pagination, scopes, hooks, eager loading, casts
    postgres.md         executor, pool, transactions, pivot queries
    mysql.md            MySQL executor and model
    sqlite.md           SQLite executor and model
    migrate.md          MigrationRunner, Schema builder, sources, table builder
    factory.md          Factory trait, FactoryBuilder<T>, Faker helpers
  guides/
    migrations.md       writing and running migrations step-by-step
    testing.md          using Factory and Faker in tests
    axum.md             OrmLayer middleware setup with Axum
    multi-tenancy.md    TenantLayer setup and per-request tenant context
```

---

## What NOT to do

- Do not create `*.md` files outside `docs/` unless the user explicitly asks (plan.md and todo.md
  are exceptions already approved).
- Do not add features, abstractions, or error-handling paths beyond what the task requires.
- Do not add comments that describe what the code does — names do that.
- Do not use `git add .` or `git add -A` — always stage selectively.
- Do not `cargo publish` without a version bump, clean tree, and passing quality gates.
- Do not modify `Cargo.lock` manually.
