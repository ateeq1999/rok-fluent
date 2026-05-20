//! Polymorphic relationship types.
//!
//! - [`MorphTo`] — "belongs to" one of many parent types via `{key}_type` + `{key}_id`
//! - [`MorphMany`] — parent side "has many" via a morph key
//! - [`MorphOne`]  — parent side "has one" via a morph key
//! - [`MorphToMany`] — polymorphic pivot (taggable pattern)

use std::marker::PhantomData;

use sqlx::postgres::PgRow;
use sqlx::PgPool;

use crate::core::condition::SqlValue;
use crate::core::model::Model;
use crate::core::sqlx::pg as sqlx_pg;
use crate::orm::postgres::pool;

// ── shared helpers ────────────────────────────────────────────────────────────

fn current_pool() -> Result<PgPool, sqlx::Error> {
    pool::try_current_pool().ok_or_else(|| {
        sqlx::Error::Configuration(
            "no database pool in scope — add OrmLayer to your router or \
             call pool::with_pool() in tests"
                .to_string()
                .into(),
        )
    })
}

/// Singularise a table name: `users → user`, `categories → category`.
pub(crate) fn naive_singular(table: &str) -> String {
    if let Some(s) = table.strip_suffix("ies") {
        format!("{s}y")
    } else if let Some(s) = table.strip_suffix('s') {
        s.to_string()
    } else {
        table.to_string()
    }
}

// ── MorphTo ───────────────────────────────────────────────────────────────────

/// A polymorphic "belongs to" reference stored as `(type_name, id)`.
///
/// Build from a model's morph fields, then call `.resolve::<T>()` to fetch the
/// parent row.  Returns `Err` when the stored type doesn't match `T`.
///
/// # Example
///
/// ```rust,no_run
/// # use rok_fluent::orm::morph::MorphTo;
/// # use rok_fluent::core::model::Model;
/// # #[derive(Debug, sqlx::FromRow)]
/// # pub struct Post { pub id: i64 }
/// # impl Model for Post {
/// #     fn table_name() -> &'static str { "posts" }
/// #     fn columns() -> &'static [&'static str] { &["id"] }
/// # }
/// # async fn example() -> Result<(), sqlx::Error> {
/// let morph = MorphTo::new("post", 42i64);
/// let post: Option<Post> = morph.resolve::<Post>().await?;
/// # Ok(())
/// # }
/// ```
pub struct MorphTo {
    type_name: String,
    id: SqlValue,
}

impl MorphTo {
    /// Create a new `MorphTo` from a stored type name and ID.
    pub fn new(type_name: impl Into<String>, id: impl Into<SqlValue>) -> Self {
        Self {
            type_name: type_name.into(),
            id: id.into(),
        }
    }

    /// The stored type name (e.g., `"post"` or `"video"`).
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// The stored ID value.
    pub fn id(&self) -> &SqlValue {
        &self.id
    }

    /// Fetch the parent row of type `T`.
    ///
    /// Returns `Err` if `type_name` != `naive_singular(T::table_name())`.
    pub async fn resolve<T>(&self) -> Result<Option<T>, sqlx::Error>
    where
        T: Model + for<'r> sqlx::FromRow<'r, PgRow> + Send + Unpin,
    {
        let expected = naive_singular(T::table_name());
        if self.type_name != expected {
            return Err(sqlx::Error::Configuration(
                format!(
                    "MorphTo: stored type '{}' does not match resolved type '{}' (table '{}')",
                    self.type_name,
                    expected,
                    T::table_name()
                )
                .into(),
            ));
        }
        let pool = current_pool()?;
        let sql = format!(
            "SELECT * FROM {} WHERE {} = $1 LIMIT 1",
            T::table_name(),
            T::primary_key()
        );
        sqlx_pg::fetch_optional_as::<T>(&pool, &sql, vec![self.id.clone()]).await
    }
}

// ── MorphMany ─────────────────────────────────────────────────────────────────

/// A polymorphic "has many" relationship query.
///
/// Fetches all child rows of type `T` where `{key}_type = owner_type AND {key}_id = owner_id`.
pub struct MorphMany<T> {
    owner_type: &'static str,
    owner_id: SqlValue,
    key: &'static str,
    _marker: PhantomData<T>,
}

impl<T: Model> MorphMany<T> {
    /// Create a new `MorphMany`.
    pub fn new(owner_type: &'static str, owner_id: impl Into<SqlValue>, key: &'static str) -> Self {
        Self {
            owner_type,
            owner_id: owner_id.into(),
            key,
            _marker: PhantomData,
        }
    }

    pub(crate) fn select_sql(&self) -> (String, Vec<SqlValue>) {
        let table = T::table_name();
        let sql = format!(
            "SELECT * FROM {table} WHERE {key}_type = $1 AND {key}_id = $2",
            key = self.key
        );
        (
            sql,
            vec![
                SqlValue::Text(self.owner_type.to_string()),
                self.owner_id.clone(),
            ],
        )
    }

    pub(crate) fn count_sql(&self) -> (String, Vec<SqlValue>) {
        let table = T::table_name();
        let sql = format!(
            "SELECT COUNT(*) FROM {table} WHERE {key}_type = $1 AND {key}_id = $2",
            key = self.key
        );
        (
            sql,
            vec![
                SqlValue::Text(self.owner_type.to_string()),
                self.owner_id.clone(),
            ],
        )
    }
}

impl<T> MorphMany<T>
where
    T: Model + for<'r> sqlx::FromRow<'r, PgRow> + Send + Unpin,
{
    /// Fetch all child rows.
    pub async fn get(self) -> Result<Vec<T>, sqlx::Error> {
        let pool = current_pool()?;
        let (sql, params) = self.select_sql();
        sqlx_pg::fetch_all_as::<T>(&pool, &sql, params).await
    }

    /// Fetch the first child row, or `None`.
    pub async fn first(self) -> Result<Option<T>, sqlx::Error> {
        let pool = current_pool()?;
        let (base_sql, params) = self.select_sql();
        let sql = format!("{base_sql} LIMIT 1");
        sqlx_pg::fetch_optional_as::<T>(&pool, &sql, params).await
    }

    /// Return the count of child rows.
    pub async fn count(self) -> Result<i64, sqlx::Error> {
        let pool = current_pool()?;
        let (sql, params) = self.count_sql();
        let row = sqlx_pg::build_query(&sql, params).fetch_one(&pool).await?;
        use sqlx::Row;
        row.try_get::<i64, _>(0)
    }

    /// Return `true` if at least one child row exists.
    pub async fn exists(self) -> Result<bool, sqlx::Error> {
        Ok(self.count().await? > 0)
    }
}

// ── MorphOne ──────────────────────────────────────────────────────────────────

/// A polymorphic "has one" relationship query.
///
/// Same as [`MorphMany`] but always returns a single row.
pub struct MorphOne<T> {
    owner_type: &'static str,
    owner_id: SqlValue,
    key: &'static str,
    _marker: PhantomData<T>,
}

impl<T: Model> MorphOne<T> {
    /// Create a new `MorphOne`.
    pub fn new(owner_type: &'static str, owner_id: impl Into<SqlValue>, key: &'static str) -> Self {
        Self {
            owner_type,
            owner_id: owner_id.into(),
            key,
            _marker: PhantomData,
        }
    }

    pub(crate) fn select_sql(&self) -> (String, Vec<SqlValue>) {
        let table = T::table_name();
        let sql = format!(
            "SELECT * FROM {table} WHERE {key}_type = $1 AND {key}_id = $2 LIMIT 1",
            key = self.key
        );
        (
            sql,
            vec![
                SqlValue::Text(self.owner_type.to_string()),
                self.owner_id.clone(),
            ],
        )
    }
}

impl<T> MorphOne<T>
where
    T: Model + for<'r> sqlx::FromRow<'r, PgRow> + Send + Unpin,
{
    /// Fetch the single related row, or `None`.
    pub async fn get(self) -> Result<Option<T>, sqlx::Error> {
        let pool = current_pool()?;
        let (sql, params) = self.select_sql();
        sqlx_pg::fetch_optional_as::<T>(&pool, &sql, params).await
    }

    /// Return `true` if the related row exists.
    pub async fn exists(self) -> Result<bool, sqlx::Error> {
        let pool = current_pool()?;
        let table = T::table_name();
        let sql = format!(
            "SELECT COUNT(*) FROM {table} WHERE {key}_type = $1 AND {key}_id = $2 LIMIT 1",
            key = self.key
        );
        let params = vec![SqlValue::Text(self.owner_type.to_string()), self.owner_id];
        let row = sqlx_pg::build_query(&sql, params).fetch_one(&pool).await?;
        use sqlx::Row;
        Ok(row.try_get::<i64, _>(0)? > 0)
    }
}

// ── MorphToMany ───────────────────────────────────────────────────────────────

/// A polymorphic many-to-many relationship query (taggable pattern).
///
/// SQL pattern (from owner's side):
/// ```sql
/// SELECT t.* FROM t
/// INNER JOIN pivot ON pivot.related_fk = t.pk
/// WHERE pivot.{key}_type = $1 AND pivot.{key}_id = $2
/// ```
pub struct MorphToMany<T> {
    owner_type: &'static str,
    owner_id: SqlValue,
    pivot: &'static str,
    key: &'static str,
    related_fk: &'static str,
    _marker: PhantomData<T>,
}

impl<T: Model> MorphToMany<T> {
    /// Create a new `MorphToMany`.
    pub fn new(
        owner_type: &'static str,
        owner_id: impl Into<SqlValue>,
        pivot: &'static str,
        key: &'static str,
        related_fk: &'static str,
    ) -> Self {
        Self {
            owner_type,
            owner_id: owner_id.into(),
            pivot,
            key,
            related_fk,
            _marker: PhantomData,
        }
    }

    pub(crate) fn select_sql(&self) -> (String, Vec<SqlValue>) {
        let t = T::table_name();
        let pk = T::primary_key();
        let pivot = self.pivot;
        let key = self.key;
        let related_fk = self.related_fk;
        let sql = format!(
            "SELECT {t}.* FROM {t} \
             INNER JOIN {pivot} ON {pivot}.{related_fk} = {t}.{pk} \
             WHERE {pivot}.{key}_type = $1 AND {pivot}.{key}_id = $2"
        );
        (
            sql,
            vec![
                SqlValue::Text(self.owner_type.to_string()),
                self.owner_id.clone(),
            ],
        )
    }

    pub(crate) fn count_sql(&self) -> (String, Vec<SqlValue>) {
        let t = T::table_name();
        let pk = T::primary_key();
        let pivot = self.pivot;
        let key = self.key;
        let related_fk = self.related_fk;
        let sql = format!(
            "SELECT COUNT(*) FROM {t} \
             INNER JOIN {pivot} ON {pivot}.{related_fk} = {t}.{pk} \
             WHERE {pivot}.{key}_type = $1 AND {pivot}.{key}_id = $2"
        );
        (
            sql,
            vec![
                SqlValue::Text(self.owner_type.to_string()),
                self.owner_id.clone(),
            ],
        )
    }
}

impl<T> MorphToMany<T>
where
    T: Model + for<'r> sqlx::FromRow<'r, PgRow> + Send + Unpin,
{
    // ── read terminals ────────────────────────────────────────────────────────

    /// Fetch all related rows.
    pub async fn get(self) -> Result<Vec<T>, sqlx::Error> {
        let pool = current_pool()?;
        let (sql, params) = self.select_sql();
        sqlx_pg::fetch_all_as::<T>(&pool, &sql, params).await
    }

    /// Fetch the first related row, or `None`.
    pub async fn first(self) -> Result<Option<T>, sqlx::Error> {
        let pool = current_pool()?;
        let (base_sql, params) = self.select_sql();
        let sql = format!("{base_sql} LIMIT 1");
        sqlx_pg::fetch_optional_as::<T>(&pool, &sql, params).await
    }

    /// Return the count of related rows.
    pub async fn count(self) -> Result<i64, sqlx::Error> {
        let pool = current_pool()?;
        let (sql, params) = self.count_sql();
        let row = sqlx_pg::build_query(&sql, params).fetch_one(&pool).await?;
        use sqlx::Row;
        row.try_get::<i64, _>(0)
    }

    /// Return `true` if at least one related row exists.
    pub async fn exists(self) -> Result<bool, sqlx::Error> {
        Ok(self.count().await? > 0)
    }

    // ── write operations ──────────────────────────────────────────────────────

    /// Attach `related_id` to the owner via the morph pivot. Idempotent.
    pub async fn attach(self, related_id: impl Into<SqlValue>) -> Result<u64, sqlx::Error> {
        let pool = current_pool()?;
        let sql = format!(
            "INSERT INTO {pivot} ({rfk}, {key}_id, {key}_type) \
             VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
            pivot = self.pivot,
            rfk = self.related_fk,
            key = self.key,
        );
        let params = vec![
            related_id.into(),
            self.owner_id,
            SqlValue::Text(self.owner_type.to_string()),
        ];
        let result = sqlx_pg::build_query(&sql, params).execute(&pool).await?;
        Ok(result.rows_affected())
    }

    /// Detach `related_id` from the owner.
    pub async fn detach(self, related_id: impl Into<SqlValue>) -> Result<u64, sqlx::Error> {
        let pool = current_pool()?;
        let sql = format!(
            "DELETE FROM {pivot} WHERE {rfk} = $1 AND {key}_id = $2 AND {key}_type = $3",
            pivot = self.pivot,
            rfk = self.related_fk,
            key = self.key,
        );
        let params = vec![
            related_id.into(),
            self.owner_id,
            SqlValue::Text(self.owner_type.to_string()),
        ];
        let result = sqlx_pg::build_query(&sql, params).execute(&pool).await?;
        Ok(result.rows_affected())
    }

    /// Remove all pivot rows for this owner (detach everything).
    pub async fn detach_all(self) -> Result<u64, sqlx::Error> {
        let pool = current_pool()?;
        let sql = format!(
            "DELETE FROM {pivot} WHERE {key}_id = $1 AND {key}_type = $2",
            pivot = self.pivot,
            key = self.key,
        );
        let params = vec![self.owner_id, SqlValue::Text(self.owner_type.to_string())];
        let result = sqlx_pg::build_query(&sql, params).execute(&pool).await?;
        Ok(result.rows_affected())
    }

    /// Sync: detach IDs not in `ids`, attach missing ones. Idempotent.
    pub async fn sync(self, ids: &[impl Into<SqlValue> + Clone]) -> Result<(), sqlx::Error> {
        let pool = current_pool()?;
        let owner_type = SqlValue::Text(self.owner_type.to_string());

        if ids.is_empty() {
            let sql = format!(
                "DELETE FROM {pivot} WHERE {key}_id = $1 AND {key}_type = $2",
                pivot = self.pivot,
                key = self.key,
            );
            sqlx_pg::build_query(&sql, vec![self.owner_id, owner_type])
                .execute(&pool)
                .await?;
            return Ok(());
        }

        let id_vals: Vec<SqlValue> = ids.iter().map(|v| v.clone().into()).collect();

        let ph: Vec<String> = (3..=id_vals.len() + 2).map(|i| format!("${i}")).collect();
        let del_sql = format!(
            "DELETE FROM {pivot} WHERE {key}_id = $1 AND {key}_type = $2 AND {rfk} NOT IN ({ph})",
            pivot = self.pivot,
            key = self.key,
            rfk = self.related_fk,
            ph = ph.join(", "),
        );
        let mut del_params = vec![self.owner_id.clone(), owner_type.clone()];
        del_params.extend(id_vals.iter().cloned());
        sqlx_pg::build_query(&del_sql, del_params)
            .execute(&pool)
            .await?;

        for id_val in id_vals {
            let ins_sql = format!(
                "INSERT INTO {pivot} ({rfk}, {key}_id, {key}_type) \
                 VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
                pivot = self.pivot,
                rfk = self.related_fk,
                key = self.key,
            );
            sqlx_pg::build_query(
                &ins_sql,
                vec![id_val, self.owner_id.clone(), owner_type.clone()],
            )
            .execute(&pool)
            .await?;
        }

        Ok(())
    }

    /// Toggle: attach IDs not present, detach IDs already present.
    pub async fn toggle(self, ids: &[impl Into<SqlValue> + Clone]) -> Result<(), sqlx::Error> {
        let pool = current_pool()?;
        let owner_type = SqlValue::Text(self.owner_type.to_string());
        let id_vals: Vec<SqlValue> = ids.iter().map(|v| v.clone().into()).collect();

        if id_vals.is_empty() {
            return Ok(());
        }

        let ph: Vec<String> = (3..=id_vals.len() + 2).map(|i| format!("${i}")).collect();
        let sel_sql = format!(
            "SELECT {rfk} FROM {pivot} \
             WHERE {key}_id = $1 AND {key}_type = $2 AND {rfk} IN ({ph})",
            pivot = self.pivot,
            rfk = self.related_fk,
            key = self.key,
            ph = ph.join(", "),
        );
        let mut sel_params = vec![self.owner_id.clone(), owner_type.clone()];
        sel_params.extend(id_vals.iter().cloned());

        let rows = sqlx_pg::build_query(&sel_sql, sel_params)
            .fetch_all(&pool)
            .await?;
        use sqlx::Row;
        let existing: Vec<i64> = rows
            .iter()
            .filter_map(|r| r.try_get::<i64, _>(0).ok())
            .collect();

        for id_val in id_vals {
            let already = match &id_val {
                SqlValue::Integer(n) => existing.contains(n),
                _ => false,
            };

            if already {
                let del = format!(
                    "DELETE FROM {pivot} WHERE {rfk} = $1 AND {key}_id = $2 AND {key}_type = $3",
                    pivot = self.pivot,
                    rfk = self.related_fk,
                    key = self.key,
                );
                sqlx_pg::build_query(
                    &del,
                    vec![id_val, self.owner_id.clone(), owner_type.clone()],
                )
                .execute(&pool)
                .await?;
            } else {
                let ins = format!(
                    "INSERT INTO {pivot} ({rfk}, {key}_id, {key}_type) \
                     VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
                    pivot = self.pivot,
                    rfk = self.related_fk,
                    key = self.key,
                );
                sqlx_pg::build_query(
                    &ins,
                    vec![id_val, self.owner_id.clone(), owner_type.clone()],
                )
                .execute(&pool)
                .await?;
            }
        }

        Ok(())
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    struct Comment;
    impl Model for Comment {
        fn table_name() -> &'static str {
            "comments"
        }
        fn columns() -> &'static [&'static str] {
            &["id", "body"]
        }
    }

    struct Tag;
    impl Model for Tag {
        fn table_name() -> &'static str {
            "tags"
        }
        fn columns() -> &'static [&'static str] {
            &["id", "name"]
        }
    }

    #[allow(dead_code)]
    struct Post;
    impl Model for Post {
        fn table_name() -> &'static str {
            "posts"
        }
        fn columns() -> &'static [&'static str] {
            &["id", "title"]
        }
    }

    #[test]
    fn singular_regular() {
        assert_eq!(naive_singular("users"), "user");
        assert_eq!(naive_singular("posts"), "post");
    }

    #[test]
    fn singular_ies_ending() {
        assert_eq!(naive_singular("categories"), "category");
    }

    #[test]
    fn morph_to_stores_type_and_id() {
        let m = MorphTo::new("post", 42i64);
        assert_eq!(m.type_name(), "post");
        assert!(matches!(m.id(), SqlValue::Integer(42)));
    }

    #[test]
    fn morph_many_select_sql() {
        let q = MorphMany::<Comment>::new("post", 1i64, "commentable");
        let (sql, params) = q.select_sql();
        assert_eq!(
            sql,
            "SELECT * FROM comments WHERE commentable_type = $1 AND commentable_id = $2"
        );
        assert_eq!(params.len(), 2);
        assert!(matches!(&params[0], SqlValue::Text(s) if s == "post"));
    }

    #[test]
    fn morph_one_select_sql_has_limit() {
        let q = MorphOne::<Comment>::new("post", 1i64, "commentable");
        let (sql, params) = q.select_sql();
        assert!(sql.contains("LIMIT 1"), "sql={sql}");
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn morph_to_many_select_sql() {
        let q = MorphToMany::<Tag>::new("post", 1i64, "taggables", "taggable", "tag_id");
        let (sql, params) = q.select_sql();
        assert!(sql.contains("SELECT tags.* FROM tags"), "sql={sql}");
        assert!(
            sql.contains("INNER JOIN taggables ON taggables.tag_id = tags.id"),
            "sql={sql}"
        );
        assert!(sql.contains("taggable_type = $1"), "sql={sql}");
        assert_eq!(params.len(), 2);
        assert!(matches!(&params[0], SqlValue::Text(s) if s == "post"));
    }
}
