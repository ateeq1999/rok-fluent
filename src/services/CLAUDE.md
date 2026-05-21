# src/services — Subsystem Rules

Service layer sitting above Active Record. All services require `feature = "active"` + `feature = "postgres"` unless noted.

## Stateless vs pool-owning

| Service | Pool ownership |
|---|---|
| `CrudService<M>` | Owns `PgPool` (clone at construction) |
| `BatchService<M>` | Stateless — pool passed per call |
| `FilterBuilder<M>` | No pool — builds conditions only |
| `SortBuilder<M>` | No pool — builds order clauses only |
| `SoftDeleteService<M>` | Stateless |
| `SearchService<M>` | Stateless |
| `AuditService<M>` | Stateless |

## SearchService turbofish trap

`effective_cols` is a generic free function — type inference cannot resolve `M` without a hint:

```rust
// WRONG — E0283 type inference failure
let cols = effective_cols(cols);

// RIGHT
let cols = effective_cols::<M>(cols);
```

## Error types

- All `pub` functions return `Result<T, sqlx::Error>` for database operations.
- Validation errors (e.g. `SortBuilder` unknown column) are silent no-ops, not errors — the caller should whitelist valid column names.

## copy_insert CSV rules

- `Null` → empty string (PostgreSQL interprets the NULL escape `\N` from `FORMAT CSV, NULL ''`)
- `Bool` → `t` / `f` (PostgreSQL CSV format)
- Strings with `"` → double the quote: `"` → `""`
- Arrays → `"{val1,val2}"` wrapped in quotes

## BatchService size guidance

- `< 100 rows` → `bulk_insert`
- `100+ rows, no RETURNING` → `copy_insert` (10–50× faster)
- `100+ rows, need RETURNING or param-limit safety` → `bulk_insert_chunked`
