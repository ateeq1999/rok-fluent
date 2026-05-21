# rok-fluent-macros — Subsystem Rules

Proc-macro crate. Never published as a standalone user dependency. Called internally by `#[derive(Model, Table, Resource, Seed)]` and the `query!` macro.

## Adding a `#[table(...)]` key

1. Add an arm in `attr.parse_nested_meta(|meta| { ... })` inside `expand_model` (field loop) or `expand_table_struct` (struct loop).
2. If the key marks a column but does **not** skip it from column generation (e.g. `searchable`), add a **no-op arm** in the `expand_table` path to avoid "unknown attribute" errors — do NOT set `skip = true`.
3. Relationship annotations (`has_one`, `has_many`, `belongs_to`, `many_through`) **do** set `skip = true` — they are never real columns.

## syn / quote patterns used here

```rust
// Parse a key=value attribute
if meta.path.is_ident("column") {
    let s: LitStr = meta.value()?.parse()?;
    col_override = Some(s.value());
}

// Parse a bare flag attribute (no value)
if meta.path.is_ident("searchable") {
    is_searchable = true;
}

// Generate a static slice
let n = items.len();
quote! {
    static ARR: [&str; #n] = [#(#items),*];
    &ARR
}
```

## Test pattern

```rust
#[test]
fn expand_searchable() {
    let input: DeriveInput = syn::parse_str(r#"
        #[derive(Model)]
        struct Post { #[table(searchable)] pub title: String }
    "#).unwrap();
    let tokens = expand_model(input).to_string();
    assert!(tokens.contains("searchable_columns"));
}
```

## Rules

- Return `syn::Error` (not `panic!`) for invalid attribute combinations.
- Every generated impl must pass `cargo expand` without extra imports — use fully-qualified paths in `quote!`.
- Do not emit `#[allow(...)]` in generated code.
