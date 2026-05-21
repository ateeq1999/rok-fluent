//! [`Model`] trait — implemented automatically by `#[derive(Model)]`.

use super::query::QueryBuilder;

/// The core ORM trait. Implemented via `#[derive(Model)]` from `rok-fluent`.
pub trait Model: Sized {
    /// SQL table name (e.g. `"users"`).
    fn table_name() -> &'static str;

    /// Primary-key column. Defaults to `"id"`.
    fn primary_key() -> &'static str {
        "id"
    }

    /// All primary-key columns. Defaults to `&["id"]`.
    fn primary_keys() -> &'static [&'static str] {
        &["id"]
    }

    /// All column names in declaration order.
    fn columns() -> &'static [&'static str];

    /// Soft-delete column, if this model uses soft deletes.
    fn soft_delete_column() -> Option<&'static str> {
        None
    }

    /// Columns marked `#[table(searchable)]` in declaration order.
    ///
    /// Used by [`SearchService`](crate::services::SearchService) as the default column
    /// set when no explicit column list is passed.
    fn searchable_columns() -> &'static [&'static str] {
        &[]
    }

    /// Auto-timestamp column names `(created_at, updated_at)`, if configured.
    fn timestamp_columns() -> Option<(&'static str, &'static str)> {
        None
    }

    /// Tenant column for row-level multi-tenancy, if this model is tenant-scoped.
    fn tenant_column() -> Option<&'static str> {
        None
    }

    /// Start a new [`QueryBuilder`] scoped to this model.
    fn query() -> QueryBuilder<Self> {
        let q = QueryBuilder::new(Self::table_name());
        #[cfg(feature = "tenant")]
        if let Some(col) = Self::tenant_column() {
            if let Some(tid) = super::tenant::current_tenant_id() {
                return q.where_eq(col, tid);
            }
        }
        q
    }

    /// Start a new [`QueryBuilder`] that bypasses the tenant scope.
    fn without_tenant_scope() -> QueryBuilder<Self> {
        QueryBuilder::new(Self::table_name())
    }

    /// The primary key value of this model instance as a `SqlValue`.
    fn pk_value(&self) -> super::condition::SqlValue {
        panic!(
            "`pk_value` not implemented for `{}` — use `#[derive(Model)]` to generate it",
            std::any::type_name::<Self>()
        )
    }

    /// All primary key values of this model instance as a `Vec<SqlValue>`.
    fn pk_values(&self) -> Vec<super::condition::SqlValue> {
        panic!(
            "`pk_values` not implemented for `{}` — use `#[derive(Model)]` to generate it",
            std::any::type_name::<Self>()
        )
    }

    /// Build a `SELECT … WHERE <pk> = $1` query.
    fn find(id: impl Into<super::condition::SqlValue>) -> QueryBuilder<Self> {
        Self::query().where_eq(Self::primary_key(), id)
    }

    /// Build a `SELECT … WHERE <pk1> = $1 AND <pk2> = $2 …` query for composite primary keys.
    fn find_composite(pks: Vec<super::condition::SqlValue>) -> QueryBuilder<Self> {
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
