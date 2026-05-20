//! Procedural macros for rok-fluent.
//!
//! | Macro | Kind | Description |
//! |---|---|---|
//! | `#[derive(Model)]` | derive | Implement the `Model` trait for a struct |
//! | `#[derive(Table)]` | derive | Generate a typed DSL module (requires `query` feature) |
//! | `#[derive(Resource)]` | derive | Generate `to_resource()` for API serialization |
//! | `#[derive(Seed)]` | derive | Generate `seed(pool, n)` scaffolding |
//! | `query!` | function-like | Shorthand for building a `QueryBuilder` |

use heck::ToSnakeCase;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, LitStr};

#[proc_macro_derive(Model, attributes(rok_orm, model, cast))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_model(input).unwrap_or_else(|e| e.to_compile_error().into())
}

fn expand_model(input: DeriveInput) -> syn::Result<TokenStream> {
    let struct_name = &input.ident;

    let mut custom_table: Option<String> = None;
    let mut struct_pk: Option<String> = None;
    let mut typed_queries: bool = false;
    let mut struct_pks: Vec<String> = Vec::new();
    let mut soft_delete: bool = false;
    let mut timestamps: bool = false;
    let mut _id_type: Option<String> = None;
    let mut fillable_fields: Vec<String> = Vec::new();
    let mut guarded_fields: Vec<String> = Vec::new();

    for attr in &input.attrs {
        let is_rok_orm = attr.path().is_ident("rok_orm");
        let is_model = attr.path().is_ident("model");
        if !is_rok_orm && !is_model {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("table") {
                let value = meta.value()?;
                let s: LitStr = value.parse()?;
                custom_table = Some(s.value());
                Ok(())
            } else if meta.path.is_ident("primary_key") || meta.path.is_ident("pk") {
                // `pk` is the short form available on the `#[model(...)]` namespace
                let value = meta.value()?;
                let s: LitStr = value.parse()?;
                struct_pk = Some(s.value());
                Ok(())
            } else if meta.path.is_ident("primary_keys") {
                let value = meta.value()?;
                let s: LitStr = value.parse()?;
                for pk in s.value().split(',') {
                    let trimmed = pk.trim().to_string();
                    if !trimmed.is_empty() && !struct_pks.contains(&trimmed) {
                        struct_pks.push(trimmed);
                    }
                }
                Ok(())
            } else if meta.path.is_ident("id") {
                let value = meta.value()?;
                let s: LitStr = value.parse()?;
                _id_type = Some(s.value());
                Ok(())
            } else if meta.path.is_ident("soft_delete") {
                soft_delete = true;
                Ok(())
            } else if meta.path.is_ident("timestamps") {
                timestamps = true;
                Ok(())
            } else if meta.path.is_ident("fillable") {
                let value = meta.value()?;
                let s: LitStr = value.parse()?;
                for f in s.value().split(',') {
                    let trimmed = f.trim().to_string();
                    if !trimmed.is_empty() {
                        fillable_fields.push(trimmed);
                    }
                }
                Ok(())
            } else if meta.path.is_ident("guarded") {
                let value = meta.value()?;
                let s: LitStr = value.parse()?;
                for f in s.value().split(',') {
                    let trimmed = f.trim().to_string();
                    if !trimmed.is_empty() {
                        guarded_fields.push(trimmed);
                    }
                }
                Ok(())
            } else if meta.path.is_ident("typed_queries") {
                typed_queries = true;
                Ok(())
            } else if meta.path.is_ident("hidden")
                || meta.path.is_ident("computed")
                || meta.path.is_ident("scopes")
                || meta.path.is_ident("tenant_scoped")
            {
                if meta.input.peek(syn::Token![=]) {
                    let value = meta.value()?;
                    let _: LitStr = value.parse()?;
                }
                Ok(())
            } else if is_model {
                Err(meta.error(
                    "unknown #[model(...)] struct attribute.\n\
                    Fix: supported attrs are table, pk, timestamps, soft_delete, \
                    fillable, guarded",
                ))
            } else {
                Err(meta.error(
                    "unknown #[rok_orm(...)] struct attribute.\n\
                    Fix: supported attrs are table, primary_key, primary_keys, id, \
                    soft_delete, timestamps, hidden, computed, fillable, guarded, scopes, \
                    tenant_scoped, typed_queries",
                ))
            }
        })?;
    }

    let table =
        custom_table.unwrap_or_else(|| format!("{}s", struct_name.to_string().to_snake_case()));

    let struct_pk_clone = struct_pk.clone();

    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(f) => &f.named,
            _ => {
                return Err(syn::Error::new(
                    Span::call_site(),
                    "rok-fluent: #[derive(Model)] only supports structs with named fields\n  \
                     fix: change `struct Foo(T)` or `struct Foo;` to `struct Foo { field: T }`",
                ))
            }
        },
        _ => {
            return Err(syn::Error::new(
                Span::call_site(),
                "rok-fluent: #[derive(Model)] only supports structs\n  \
                 fix: apply this derive to a struct, not an enum or union",
            ))
        }
    };

    let mut column_names: Vec<String> = Vec::new();
    let mut field_pk: Option<String> = None;
    let mut field_pks: Vec<String> = Vec::new();
    let mut field_pk_rust_idents: Vec<String> = Vec::new();
    let mut field_pk_rust_ident: Option<String> = None;
    let mut cast_pairs: Vec<(String, String)> = Vec::new();
    let mut index_hints: Vec<(String, String)> = Vec::new();
    let mut typed_query_fields: Vec<(String, String)> = Vec::new();

    for field in fields.iter() {
        let field_ident = match &field.ident {
            Some(id) => id.to_string(),
            None => continue,
        };

        let mut skip = false;
        let mut col_override: Option<String> = None;
        let mut is_pk = false;
        let mut cast_type: Option<String> = None;
        let mut index_kind: Option<String> = None;

        for attr in &field.attrs {
            if attr.path().is_ident("rok_orm") || attr.path().is_ident("model") {
                let is_field_model = attr.path().is_ident("model");
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("skip") {
                        skip = true;
                        Ok(())
                    } else if meta.path.is_ident("primary_key") || meta.path.is_ident("pk") {
                        // `pk` is the short form available on `#[model(...)]`
                        is_pk = true;
                        Ok(())
                    } else if meta.path.is_ident("column") {
                        let value = meta.value()?;
                        let s: LitStr = value.parse()?;
                        col_override = Some(s.value());
                        Ok(())
                    } else if meta.path.is_ident("index") {
                        index_kind = Some("index".to_string());
                        Ok(())
                    } else if meta.path.is_ident("unique_index") {
                        index_kind = Some("unique".to_string());
                        Ok(())
                    } else if meta.path.is_ident("full_text_index") {
                        index_kind = Some("full_text".to_string());
                        Ok(())
                    } else if meta.path.is_ident("hidden") {
                        Ok(())
                    } else if is_field_model {
                        Err(meta.error(
                            "unknown #[model(...)] field attribute.\n\
                            Fix: supported attrs are skip, pk, column",
                        ))
                    } else {
                        Err(meta.error(
                            "unknown #[rok_orm(...)] field attribute.\n\
                            Fix: supported attrs are skip, primary_key, column, hidden, \
                            index, unique_index, full_text_index",
                        ))
                    }
                })?;
            } else if attr.path().is_ident("cast") {
                attr.parse_nested_meta(|meta| {
                    let name = meta
                        .path
                        .get_ident()
                        .map(|i| i.to_string())
                        .unwrap_or_default();
                    match name.as_str() {
                        "json" | "encrypted" | "enum" | "csv" | "timestamp" | "array" => {
                            cast_type = Some(name);
                            Ok(())
                        }
                        other => Err(meta.error(format!(
                            "unknown cast `{other}`.\n\
                             Fix: use one of #[cast(json)], #[cast(encrypted)], \
                             #[cast(enum)], #[cast(csv)], #[cast(timestamp)], #[cast(array)]"
                        ))),
                    }
                })?;
            }
        }

        if is_pk {
            let col_name = col_override.clone().unwrap_or(field_ident.clone());
            if field_pk.is_none() {
                field_pk = Some(col_name.clone());
                field_pk_rust_ident = Some(field_ident.clone());
            }
            if !field_pks.contains(&col_name) {
                field_pks.push(col_name);
                field_pk_rust_idents.push(field_ident.clone());
            }
        }

        let col_name = col_override.clone().unwrap_or(field_ident.clone());

        if let Some(ct) = cast_type {
            cast_pairs.push((col_name.clone(), ct));
        }

        if let Some(kind) = index_kind {
            index_hints.push((col_name.clone(), kind));
        }

        if !skip {
            if typed_queries {
                typed_query_fields.push((field_ident.clone(), col_name.clone()));
            }
            column_names.push(col_name);
        }
    }

    let pk = field_pk
        .clone()
        .or_else(|| struct_pks.first().cloned())
        .or(struct_pk.clone())
        .unwrap_or_else(|| "id".to_string());

    let primary_keys: Vec<String> = if !field_pks.is_empty() {
        field_pks.clone()
    } else if !struct_pks.is_empty() {
        struct_pks.clone()
    } else if let Some(ref spk) = struct_pk {
        vec![spk.clone()]
    } else {
        vec!["id".to_string()]
    };

    let pk_rust_idents: Vec<syn::Ident> = if !field_pk_rust_idents.is_empty() {
        field_pk_rust_idents
            .iter()
            .map(|s| syn::Ident::new(s, Span::call_site()))
            .collect()
    } else {
        vec![]
    };

    let pk_rust_ident_str =
        field_pk_rust_ident.unwrap_or_else(|| struct_pk_clone.unwrap_or_else(|| "id".to_string()));
    let pk_rust_ident = syn::Ident::new(&pk_rust_ident_str, Span::call_site());

    let columns_len = column_names.len();
    let primary_keys_len = primary_keys.len();

    let soft_delete_impl = if soft_delete {
        quote! {
            fn soft_delete_column() -> Option<&'static str> {
                Some("deleted_at")
            }
        }
    } else {
        quote! {}
    };

    let timestamps_impl = if timestamps {
        quote! {
            fn timestamp_columns() -> Option<(&'static str, &'static str)> {
                Some(("created_at", "updated_at"))
            }
        }
    } else {
        quote! {}
    };

    let pk_values_impl = if !pk_rust_idents.is_empty() {
        quote! {
            fn pk_values(&self) -> Vec<::rok_fluent::SqlValue> {
                vec![
                    #(self.#pk_rust_idents.clone().into()),*
                ]
            }
        }
    } else {
        quote! {}
    };

    let cast_pair_tokens: Vec<_> = cast_pairs
        .iter()
        .map(|(col, ct)| quote! { (#col, #ct) })
        .collect();

    let index_hints_len = index_hints.len();
    let index_hint_tokens: Vec<_> = index_hints
        .iter()
        .map(|(col, kind)| quote! { (#col, #kind) })
        .collect();

    let typed_query_consts: Vec<_> = if typed_queries {
        typed_query_fields
            .iter()
            .map(|(rust_name, col_name)| {
                let const_name = syn::Ident::new(
                    &format!("COL_{}", rust_name.to_uppercase()),
                    Span::call_site(),
                );
                quote! {
                    pub const #const_name: &'static str = #col_name;
                }
            })
            .collect()
    } else {
        vec![]
    };

    let fillable_len = fillable_fields.len();
    let guarded_len = guarded_fields.len();
    let fillable_impl = quote! {
        pub fn fillable_columns() -> &'static [&'static str] {
            static FILLABLE: [&str; #fillable_len] = [#(#fillable_fields),*];
            &FILLABLE
        }

        pub fn guarded_columns() -> &'static [&'static str] {
            static GUARDED: [&str; #guarded_len] = [#(#guarded_fields),*];
            &GUARDED
        }

        pub fn fill(
            data: &[(&str, ::rok_fluent::SqlValue)],
        ) -> ::std::vec::Vec<(&'static str, ::rok_fluent::SqlValue)> {
            let fillable = Self::fillable_columns();
            let guarded  = Self::guarded_columns();
            let all_cols = Self::columns();

            data.iter()
                .filter_map(|(key, val)| {
                    if guarded.contains(key) {
                        return None;
                    }
                    if !fillable.is_empty() && !fillable.contains(key) {
                        return None;
                    }
                    all_cols
                        .iter()
                        .find(|c| *c == key)
                        .map(|c| (*c, val.clone()))
                })
                .collect()
        }
    };

    let expanded = quote! {
        impl ::rok_fluent::Model for #struct_name {
            fn table_name() -> &'static str {
                #table
            }

            fn primary_key() -> &'static str {
                #pk
            }

            fn primary_keys() -> &'static [&'static str] {
                static PKS: [&str; #primary_keys_len] = [#(#primary_keys),*];
                &PKS
            }

            fn columns() -> &'static [&'static str] {
                static COLS: [&str; #columns_len] = [#(#column_names),*];
                &COLS
            }

            fn pk_value(&self) -> ::rok_fluent::SqlValue {
                self.#pk_rust_ident.clone().into()
            }

            #pk_values_impl
            #soft_delete_impl
            #timestamps_impl
        }

        impl #struct_name {
            pub fn cast_fields() -> &'static [(&'static str, &'static str)] {
                &[#(#cast_pair_tokens),*]
            }

            pub fn index_hints() -> &'static [(&'static str, &'static str)] {
                static HINTS: [(&str, &str); #index_hints_len] = [#(#index_hint_tokens),*];
                &HINTS
            }

            #(#typed_query_consts)*

            #fillable_impl
        }
    };

    Ok(expanded.into())
}

// ── Resource derive ───────────────────────────────────────────────────────────

#[proc_macro_derive(Resource, attributes(resource, rok_orm, cast))]
pub fn derive_resource(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_resource(input).unwrap_or_else(|e| e.to_compile_error().into())
}

fn expand_resource(input: DeriveInput) -> syn::Result<TokenStream> {
    let struct_name = &input.ident;

    let mut struct_hidden: Vec<String> = Vec::new();
    let mut struct_computed: Vec<String> = Vec::new();

    for attr in &input.attrs {
        if !attr.path().is_ident("rok_orm") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("hidden") {
                let value = meta.value()?;
                let s: LitStr = value.parse()?;
                for name in s.value().split(',') {
                    let n = name.trim().to_string();
                    if !n.is_empty() {
                        struct_hidden.push(n);
                    }
                }
                Ok(())
            } else if meta.path.is_ident("computed") {
                let value = meta.value()?;
                let s: LitStr = value.parse()?;
                for name in s.value().split(',') {
                    let n = name.trim().to_string();
                    if !n.is_empty() {
                        struct_computed.push(n);
                    }
                }
                Ok(())
            } else {
                if meta.input.peek(syn::Token![=]) {
                    let value = meta.value()?;
                    let _: LitStr = value.parse()?;
                }
                Ok(())
            }
        })?;
    }

    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(f) => &f.named,
            _ => {
                return Err(syn::Error::new(
                    Span::call_site(),
                    "#[derive(Resource)] only supports named fields",
                ))
            }
        },
        _ => {
            return Err(syn::Error::new(
                Span::call_site(),
                "#[derive(Resource)] only supports structs",
            ))
        }
    };

    let mut push_stmts: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut auth_stmts: Vec<proc_macro2::TokenStream> = Vec::new();

    for field in fields.iter() {
        let field_ident = match &field.ident {
            Some(id) => id.clone(),
            None => continue,
        };

        let mut skip = false;
        let mut rename: Option<String> = None;
        let mut cast_type: Option<String> = None;
        let mut when_loaded = false;
        let mut when_auth: Option<String> = None;

        for attr in &field.attrs {
            if attr.path().is_ident("resource") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("skip") {
                        skip = true;
                        Ok(())
                    } else if meta.path.is_ident("rename") {
                        let value = meta.value()?;
                        let s: LitStr = value.parse()?;
                        rename = Some(s.value());
                        Ok(())
                    } else if meta.path.is_ident("when_loaded") {
                        when_loaded = true;
                        Ok(())
                    } else if meta.path.is_ident("when_auth") {
                        let value = meta.value()?;
                        let s: LitStr = value.parse()?;
                        when_auth = Some(s.value());
                        Ok(())
                    } else {
                        Err(meta.error(
                            "unknown resource attribute.\n\
                             Fix: expected `skip`, `rename = \"key\"`, `when_loaded`, \
                             or `when_auth = \"scope\"`",
                        ))
                    }
                })?;
            } else if attr.path().is_ident("rok_orm") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("hidden") {
                        skip = true;
                        Ok(())
                    } else {
                        if meta.input.peek(syn::Token![=]) {
                            let value = meta.value()?;
                            let _: LitStr = value.parse()?;
                        }
                        Ok(())
                    }
                })?;
            } else if attr.path().is_ident("cast") {
                attr.parse_nested_meta(|meta| {
                    cast_type = Some(
                        meta.path
                            .get_ident()
                            .map(|i| i.to_string())
                            .unwrap_or_default(),
                    );
                    Ok(())
                })?;
            }
        }

        let field_name = field_ident.to_string();
        if struct_hidden.contains(&field_name) {
            skip = true;
        }

        if !skip {
            let key = rename.unwrap_or(field_name);
            let value_expr = match cast_type.as_deref() {
                Some("encrypted") => quote! {
                    ::serde_json::Value::String("[ENCRYPTED]".to_string())
                },
                Some("timestamp") => quote! {
                    ::serde_json::Value::Number(
                        ::serde_json::Number::from(self.#field_ident.timestamp())
                    )
                },
                _ => quote! {
                    ::rok_fluent::orm::resource::field_to_json(&self.#field_ident)
                },
            };

            if when_loaded {
                push_stmts.push(quote! {
                    if let ::std::option::Option::Some(ref __wl) = self.#field_ident {
                        __e.push((#key, ::rok_fluent::orm::resource::field_to_json(__wl)));
                    }
                });
            } else if let Some(ref scope) = when_auth {
                auth_stmts.push(quote! {
                    if __auth_check(#scope) {
                        __e.push((#key, #value_expr));
                    }
                });
            } else {
                push_stmts.push(quote! {
                    __e.push((#key, #value_expr));
                });
            }
        }
    }

    let computed_stmts: Vec<proc_macro2::TokenStream> = struct_computed
        .iter()
        .map(|method_name| {
            let method_ident = syn::Ident::new(method_name, Span::call_site());
            quote! {
                __e.push((#method_name, ::rok_fluent::orm::resource::field_to_json(&self.#method_ident())));
            }
        })
        .collect();

    let has_auth_fields = !auth_stmts.is_empty();
    let auth_method = if has_auth_fields {
        quote! {
            pub fn to_resource_with_auth(
                &self,
                __auth_check: impl Fn(&str) -> bool,
            ) -> ::rok_fluent::orm::resource::ResourceValue {
                let mut __e: ::std::vec::Vec<(&'static str, ::serde_json::Value)> =
                    ::std::vec::Vec::new();
                #(#push_stmts)*
                #(#auth_stmts)*
                #(#computed_stmts)*
                ::rok_fluent::orm::resource::build_resource(__e)
            }
        }
    } else {
        quote! {}
    };

    let expanded = quote! {
        impl #struct_name {
            pub fn to_resource(&self) -> ::rok_fluent::orm::resource::ResourceValue {
                let mut __e: ::std::vec::Vec<(&'static str, ::serde_json::Value)> =
                    ::std::vec::Vec::new();
                #(#push_stmts)*
                #(#computed_stmts)*
                ::rok_fluent::orm::resource::build_resource(__e)
            }

            #auth_method
        }
    };

    Ok(expanded.into())
}

// ── query! macro ──────────────────────────────────────────────────────────────

use syn::{
    parse::{Parse, ParseStream},
    Expr, Ident, Token, Type,
};

struct QueryClause {
    name: Ident,
    args: Vec<Expr>,
}

impl Parse for QueryClause {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        let mut args = Vec::new();
        while !input.is_empty() && !input.peek(Token![,]) {
            args.push(input.parse::<Expr>()?);
            if input.peek(Token![,]) {
                break;
            }
        }
        Ok(QueryClause { name, args })
    }
}

struct QueryMacroInput {
    model: Type,
    clauses: Vec<QueryClause>,
}

impl Parse for QueryMacroInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let model: Type = input.parse()?;
        let mut clauses = Vec::new();
        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
            clauses.push(input.parse::<QueryClause>()?);
        }
        Ok(QueryMacroInput { model, clauses })
    }
}

/// Shorthand `QueryBuilder` constructor.
///
/// ```rust,ignore
/// use rok_fluent::query;
///
/// let q = query!(User,
///     where_eq "active" true,
///     order_by_desc "created_at",
///     limit 10,
/// );
/// ```
#[proc_macro]
pub fn query(input: TokenStream) -> TokenStream {
    let QueryMacroInput { model, clauses } = parse_macro_input!(input as QueryMacroInput);

    let mut chain = quote! { <#model as ::rok_fluent::Model>::query() };

    for clause in clauses {
        let name_str = clause.name.to_string();
        let args = &clause.args;

        let call = match (name_str.as_str(), args.len()) {
            ("where_eq", 2) => {
                let (c, v) = (&args[0], &args[1]);
                quote! { .where_eq(#c, #v) }
            }
            ("where_ne", 2) => {
                let (c, v) = (&args[0], &args[1]);
                quote! { .where_ne(#c, #v) }
            }
            ("where_gt", 2) => {
                let (c, v) = (&args[0], &args[1]);
                quote! { .where_gt(#c, #v) }
            }
            ("where_gte", 2) => {
                let (c, v) = (&args[0], &args[1]);
                quote! { .where_gte(#c, #v) }
            }
            ("where_lt", 2) => {
                let (c, v) = (&args[0], &args[1]);
                quote! { .where_lt(#c, #v) }
            }
            ("where_lte", 2) => {
                let (c, v) = (&args[0], &args[1]);
                quote! { .where_lte(#c, #v) }
            }
            ("where_like", 2) => {
                let (c, v) = (&args[0], &args[1]);
                quote! { .where_like(#c, #v) }
            }
            ("where_not_like", 2) => {
                let (c, v) = (&args[0], &args[1]);
                quote! { .where_not_like(#c, #v) }
            }
            ("where_null", 1) => {
                let c = &args[0];
                quote! { .where_null(#c) }
            }
            ("where_not_null", 1) => {
                let c = &args[0];
                quote! { .where_not_null(#c) }
            }
            ("or_where_eq", 2) => {
                let (c, v) = (&args[0], &args[1]);
                quote! { .or_where_eq(#c, #v) }
            }
            ("or_where_ne", 2) => {
                let (c, v) = (&args[0], &args[1]);
                quote! { .or_where_ne(#c, #v) }
            }
            ("order_by", 1) => {
                let c = &args[0];
                quote! { .order_by(#c) }
            }
            ("order_by_desc", 1) => {
                let c = &args[0];
                quote! { .order_by_desc(#c) }
            }
            ("limit", 1) => {
                let n = &args[0];
                quote! { .limit(#n) }
            }
            ("offset", 1) => {
                let n = &args[0];
                quote! { .offset(#n) }
            }
            ("select", _) => {
                quote! { .select(&[#(#args),*]) }
            }
            ("distinct", 0) => quote! { .distinct() },
            ("where_in", n) if n >= 2 => {
                let c = &args[0];
                let vals = &args[1..];
                quote! { .where_in(#c, vec![#(::rok_fluent::SqlValue::from(#vals)),*]) }
            }
            ("where_not_in", n) if n >= 2 => {
                let c = &args[0];
                let vals = &args[1..];
                quote! { .where_not_in(#c, vec![#(::rok_fluent::SqlValue::from(#vals)),*]) }
            }
            ("where_between", 3) => {
                let (c, lo, hi) = (&args[0], &args[1], &args[2]);
                quote! { .where_between(#c, #lo, #hi) }
            }
            ("where_json_contains", 2) => {
                let (c, json) = (&args[0], &args[1]);
                quote! { .where_json_contains(#c, #json) }
            }
            ("where_raw", 1) => {
                let sql = &args[0];
                quote! { .where_raw(#sql) }
            }
            (name, n) => {
                return syn::Error::new(
                    clause.name.span(),
                    format!("unknown query! clause `{name}` with {n} arg(s)"),
                )
                .to_compile_error()
                .into();
            }
        };

        chain = quote! { #chain #call };
    }

    chain.into()
}

// ── #[derive(Seed)] ───────────────────────────────────────────────────────────

#[proc_macro_derive(Seed)]
pub fn derive_seed(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;

    let expanded = quote! {
        impl #struct_name {
            pub async fn seed(pool: &::sqlx::PgPool, n: usize) -> ::std::result::Result<u64, ::sqlx::Error>
            where
                Self: ::rok_fluent::Model,
            {
                if n == 0 {
                    return Ok(0);
                }
                let rows: ::std::vec::Vec<::std::vec::Vec<(&'static str, ::rok_fluent::SqlValue)>> =
                    (0..n).map(|_| Self::fake_row()).collect();
                ::rok_fluent::orm::postgres::executor::bulk_insert::<Self>(
                    pool,
                    <Self as ::rok_fluent::Model>::table_name(),
                    &rows,
                )
                .await
            }

            pub async fn seed_returning(pool: &::sqlx::PgPool, n: usize) -> ::std::result::Result<::std::vec::Vec<Self>, ::sqlx::Error>
            where
                Self: ::rok_fluent::Model + for<'r> ::sqlx::FromRow<'r, ::sqlx::postgres::PgRow> + ::std::marker::Send + ::std::marker::Unpin,
            {
                if n == 0 {
                    return Ok(::std::vec::Vec::new());
                }
                let rows: ::std::vec::Vec<::std::vec::Vec<(&'static str, ::rok_fluent::SqlValue)>> =
                    (0..n).map(|_| Self::fake_row()).collect();
                ::rok_fluent::orm::postgres::executor::bulk_insert_returning::<Self>(
                    pool,
                    <Self as ::rok_fluent::Model>::table_name(),
                    &rows,
                )
                .await
            }
        }
    };

    expanded.into()
}

// ── #[derive(Table)] ──────────────────────────────────────────────────────────

/// Generate a typed DSL companion module for use with `feature = "query"`.
///
/// ```rust,ignore
/// use rok_fluent::dsl::db;
///
/// #[derive(Debug, Table, sqlx::FromRow)]
/// #[table(name = "users")]
/// pub struct User {
///     pub id:    i64,
///     pub name:  String,
///     pub email: String,
/// }
///
/// // Generated:
/// // pub mod users {
/// //     pub const table: TableMarker = TableMarker;
/// //     pub const id:    Column<User, i64>    = Column::new("users", "id");
/// //     pub const name:  Column<User, String> = Column::new("users", "name");
/// //     pub const email: Column<User, String> = Column::new("users", "email");
/// // }
///
/// let q = db::select()
///     .from(users::table)
///     .where_(users::id.eq(42_i64));
/// ```
#[proc_macro_derive(Table, attributes(table))]
pub fn derive_table(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_table(input).unwrap_or_else(|e| e.to_compile_error().into())
}

fn expand_table(input: DeriveInput) -> syn::Result<TokenStream> {
    let struct_name = &input.ident;

    let mut custom_table: Option<String> = None;

    for attr in &input.attrs {
        if !attr.path().is_ident("table") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                let value = meta.value()?;
                let s: LitStr = value.parse()?;
                custom_table = Some(s.value());
                Ok(())
            } else {
                Err(meta.error(
                    "unknown table attribute.\n\
                     Fix: expected `#[table(name = \"table_name\")]`",
                ))
            }
        })?;
    }

    let table =
        custom_table.unwrap_or_else(|| format!("{}s", struct_name.to_string().to_snake_case()));

    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(f) => &f.named,
            _ => {
                return Err(syn::Error::new(
                    Span::call_site(),
                    "rok-fluent: #[derive(Table)] only supports structs with named fields",
                ))
            }
        },
        _ => {
            return Err(syn::Error::new(
                Span::call_site(),
                "rok-fluent: #[derive(Table)] only supports structs",
            ))
        }
    };

    let mut col_defs: Vec<(syn::Ident, syn::Type, String)> = Vec::new();

    for field in fields.iter() {
        let field_ident = match &field.ident {
            Some(id) => id.clone(),
            None => continue,
        };

        let mut skip = false;
        let mut col_override: Option<String> = None;

        for attr in &field.attrs {
            if attr.path().is_ident("table") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("skip") {
                        skip = true;
                        Ok(())
                    } else if meta.path.is_ident("column") {
                        let value = meta.value()?;
                        let s: LitStr = value.parse()?;
                        col_override = Some(s.value());
                        Ok(())
                    } else {
                        Err(meta.error(
                            "unknown table field attribute.\n\
                             Fix: expected `#[table(skip)]` or `#[table(column = \"col_name\")]`",
                        ))
                    }
                })?;
            }
        }

        if skip {
            continue;
        }

        let col_name = col_override.unwrap_or_else(|| field_ident.to_string());
        col_defs.push((field_ident, field.ty.clone(), col_name));
    }

    let mod_ident = syn::Ident::new(&table, Span::call_site());

    let col_consts: Vec<proc_macro2::TokenStream> = col_defs
        .iter()
        .map(|(field_ident, field_ty, col_name)| {
            quote! {
                #[allow(non_upper_case_globals)]
                pub const #field_ident: ::rok_fluent::dsl::Column<super::#struct_name, #field_ty> =
                    ::rok_fluent::dsl::Column::new(#table, #col_name);
            }
        })
        .collect();

    let expanded = quote! {
        /// DSL companion module generated by `#[derive(Table)]`.
        ///
        /// Contains a `table` constant and one typed `Column` per field.
        #[cfg(feature = "query")]
        #[allow(non_snake_case, dead_code)]
        pub mod #mod_ident {
            #[allow(unused_imports)]
            use super::*;

            /// Marker type implementing [`::rok_fluent::dsl::Table`] for this table.
            #[derive(Debug, Clone, Copy)]
            pub struct TableMarker;

            impl ::rok_fluent::dsl::Table for TableMarker {
                fn table_name() -> &'static str {
                    #table
                }
            }

            #[allow(non_upper_case_globals)]
            /// Table singleton for use with `db::select().from(users::table)`.
            pub const table: TableMarker = TableMarker;

            #(#col_consts)*
        }
    };

    Ok(expanded.into())
}
