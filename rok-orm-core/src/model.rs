//! [`Model`] trait — implemented automatically by `#[derive(Model)]`.

use crate::query::QueryBuilder;

/// The core ORM trait.  Implemented via `#[derive(Model)]` from `rok-orm`.
///
/// ```rust,ignore
/// use rok_orm::Model;
///
/// #[derive(Model)]
/// pub struct Post {
///     pub id: i64,
///     pub title: String,
///     pub body: String,
/// }
///
/// let q = Post::query()
///     .where_eq("title", "Hello")
///     .order_by_desc("id")
///     .limit(10);
///
/// let (sql, params) = q.to_sql();
/// assert!(sql.starts_with("SELECT * FROM posts"));
/// ```
pub trait Model: Sized {
    /// SQL table name (e.g. `"users"`).
    fn table_name() -> &'static str;

    /// Primary-key column.  Defaults to `"id"`.
    /// Returns the first primary key for backward compatibility.
    fn primary_key() -> &'static str {
        "id"
    }

    /// All primary-key columns.  Defaults to `&["id"]`.
    fn primary_keys() -> &'static [&'static str] {
        &["id"]
    }

    /// All column names in declaration order.
    fn columns() -> &'static [&'static str];

    /// The soft-delete column name, if this model uses soft deletes.
    ///
    /// When `Some("deleted_at")`, fluent queries automatically append
    /// `WHERE deleted_at IS NULL` and delete operations update the timestamp
    /// instead of issuing a `DELETE FROM`.
    fn soft_delete_column() -> Option<&'static str> {
        None
    }

    /// Auto-timestamp column names `(created_at, updated_at)`, if configured.
    ///
    /// Used for introspection; the DB `DEFAULT NOW()` handles the actual value.
    fn timestamp_columns() -> Option<(&'static str, &'static str)> {
        None
    }

    /// The tenant column for row-level multi-tenancy, if this model is tenant-scoped.
    ///
    /// Set via `#[rok_orm(tenant_column = "tenant_id")]` on the struct.
    /// When set and a tenant is active (via [`TenantLayer`]), all queries produced by
    /// [`query()`] automatically include `WHERE tenant_id = $current_tenant`.
    ///
    /// [`TenantLayer`]: crate::tenant::TenantLayer
    /// [`query()`]: Model::query
    fn tenant_column() -> Option<&'static str> {
        None
    }

    /// Start a new [`QueryBuilder`] scoped to this model.
    ///
    /// If this model has a [`tenant_column`] and the `tenant` feature is enabled,
    /// the current request's tenant ID (from [`TenantLayer`]) is automatically
    /// injected as the first WHERE condition.
    ///
    /// [`tenant_column`]: Model::tenant_column
    /// [`TenantLayer`]: crate::tenant::TenantLayer
    fn query() -> QueryBuilder<Self> {
        let q = QueryBuilder::new(Self::table_name());
        #[cfg(feature = "tenant")]
        if let Some(col) = Self::tenant_column() {
            if let Some(tid) = crate::tenant::current_tenant_id() {
                return q.where_eq(col, tid);
            }
        }
        q
    }

    /// Start a new [`QueryBuilder`] that **bypasses** the tenant scope.
    ///
    /// Use for admin/cross-tenant queries where the tenant filter should not apply.
    fn without_tenant_scope() -> QueryBuilder<Self> {
        QueryBuilder::new(Self::table_name())
    }

    /// The primary key value of this model instance as a `SqlValue`.
    ///
    /// Generated automatically by `#[derive(Model)]`.  The default panics to
    /// catch models that weren't generated via the derive macro.
    fn pk_value(&self) -> crate::condition::SqlValue {
        panic!(
            "`pk_value` not implemented for `{}` — use `#[derive(Model)]` to generate it",
            std::any::type_name::<Self>()
        )
    }

    /// All primary key values of this model instance as a `Vec<SqlValue>`.
    ///
    /// Generated automatically by `#[derive(Model)]`.  The default panics to
    /// catch models that weren't generated via the derive macro.
    fn pk_values(&self) -> Vec<crate::condition::SqlValue> {
        panic!(
            "`pk_values` not implemented for `{}` — use `#[derive(Model)]` to generate it",
            std::any::type_name::<Self>()
        )
    }

    /// Build a `SELECT … WHERE <pk> = $1` query.
    fn find(id: impl Into<crate::condition::SqlValue>) -> QueryBuilder<Self> {
        Self::query().where_eq(Self::primary_key(), id)
    }

    /// Build a `SELECT … WHERE <pk1> = $1 AND <pk2> = $2 …` query for composite primary keys.
    fn find_composite(pks: Vec<crate::condition::SqlValue>) -> QueryBuilder<Self> {
        let pk_names = Self::primary_keys();
        assert_eq!(
            pks.len(),
            pk_names.len(),
            "find_composite: number of values ({}) must match number of primary keys ({})",
            pks.len(),
            pk_names.len()
        );
        let mut q = Self::query();
        for (i, pk_name) in pk_names.iter().enumerate() {
            q = q.where_eq(pk_name, pks[i].clone());
        }
        q
    }

    /// KNN similarity search: `ORDER BY {col} <-> embedding LIMIT k`.
    ///
    /// Uses the `"embedding"` column by default.  For a different column use
    /// `Self::query().nearest_to("other_col", embedding, k)`.
    fn nearest_to(embedding: &[f32], k: usize) -> QueryBuilder<Self> {
        Self::query().nearest_to("embedding", embedding, k)
    }

    /// Cosine-distance filter: `WHERE {col} <=> embedding {op} {threshold}`.
    fn where_cosine_distance(
        col: &str,
        embedding: &[f32],
        op: &str,
        threshold: f64,
    ) -> QueryBuilder<Self> {
        Self::query().where_cosine_distance(col, embedding, op, threshold)
    }
}
