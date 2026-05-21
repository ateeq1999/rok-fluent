//! `PivotQuery` — many-to-many relationship query via a pivot/junction table.
//!
//! Returned by `belongs_to_many!` macros.  Provides:
//! - Read operations: `get`, `first`, `count`, `exists`
//! - Write operations: `attach`, `attach_with`, `detach`, `detach_all`, `sync`, `toggle`

use std::marker::PhantomData;

use sqlx::postgres::PgRow;
use sqlx::PgPool;

use crate::core::condition::SqlValue;
use crate::core::model::Model;
use crate::core::sqlx::pg as sqlx_pg;
use crate::orm::postgres::pool;

/// A many-to-many relationship query operating through a pivot table.
///
/// # Example
///
/// ```rust,no_run
/// # use rok_fluent::orm::postgres::pivot_query::PivotQuery;
/// # use rok_fluent::core::model::Model;
/// # #[derive(Debug, sqlx::FromRow)]
/// # pub struct Tag { pub id: i64, pub name: String }
/// # impl Model for Tag {
/// #     fn table_name() -> &'static str { "tags" }
/// #     fn columns() -> &'static [&'static str] { &["id", "name"] }
/// # }
/// # async fn example() -> Result<(), sqlx::Error> {
/// let pq = PivotQuery::<Tag>::new(1i64, "post_tags", "post_id", "tag_id");
/// let tags = pq.get().await?;
/// # Ok(())
/// # }
/// ```
pub struct PivotQuery<T> {
    owner_id: SqlValue,
    through: &'static str,
    fk: &'static str,
    rfk: &'static str,
    _marker: PhantomData<T>,
}

impl<T> PivotQuery<T>
where
    T: Model + for<'r> sqlx::FromRow<'r, PgRow> + Send + Unpin,
{
    /// Create a new `PivotQuery`.
    pub fn new(
        owner_id: impl Into<SqlValue>,
        through: &'static str,
        fk: &'static str,
        rfk: &'static str,
    ) -> Self {
        Self {
            owner_id: owner_id.into(),
            through,
            fk,
            rfk,
            _marker: PhantomData,
        }
    }

    fn pool() -> Result<PgPool, sqlx::Error> {
        pool::try_current_pool().ok_or_else(|| {
            sqlx::Error::Configuration(
                "no database pool in scope — add OrmLayer to your router or \
                 call pool::with_pool() in tests"
                    .to_string()
                    .into(),
            )
        })
    }

    // ── SQL helpers ───────────────────────────────────────────────────────────

    fn select_sql(&self) -> (String, Vec<SqlValue>) {
        let t = T::table_name();
        let pk = T::primary_key();
        let sql = format!(
            "SELECT {t}.* FROM {t} INNER JOIN {through} ON {through}.{rfk} = {t}.{pk} \
             WHERE {through}.{fk} = $1",
            through = self.through,
            rfk = self.rfk,
            fk = self.fk,
        );
        (sql, vec![self.owner_id.clone()])
    }

    fn count_sql(&self) -> (String, Vec<SqlValue>) {
        let t = T::table_name();
        let pk = T::primary_key();
        let sql = format!(
            "SELECT COUNT(*) FROM {t} INNER JOIN {through} ON {through}.{rfk} = {t}.{pk} \
             WHERE {through}.{fk} = $1",
            through = self.through,
            rfk = self.rfk,
            fk = self.fk,
        );
        (sql, vec![self.owner_id.clone()])
    }

    // ── read terminals ────────────────────────────────────────────────────────

    /// Fetch all related rows.
    pub async fn get(self) -> Result<Vec<T>, sqlx::Error> {
        let pool = Self::pool()?;
        let (sql, params) = self.select_sql();
        sqlx_pg::fetch_all_as::<T>(&pool, &sql, params).await
    }

    /// Fetch the first related row, or `None`.
    pub async fn first(self) -> Result<Option<T>, sqlx::Error> {
        let pool = Self::pool()?;
        let (base_sql, params) = self.select_sql();
        let sql = format!("{base_sql} LIMIT 1");
        sqlx_pg::fetch_optional_as::<T>(&pool, &sql, params).await
    }

    /// Return the count of related rows.
    pub async fn count(self) -> Result<i64, sqlx::Error> {
        let pool = Self::pool()?;
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

    /// Attach `related_id` to the owner via the pivot table.
    /// Uses `ON CONFLICT DO NOTHING` so it is idempotent.
    pub async fn attach(self, related_id: impl Into<SqlValue>) -> Result<u64, sqlx::Error> {
        let pool = Self::pool()?;
        let sql = format!(
            "INSERT INTO {through} ({fk}, {rfk}) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            through = self.through,
            fk = self.fk,
            rfk = self.rfk,
        );
        let params = vec![self.owner_id, related_id.into()];
        let result = sqlx_pg::build_query(&sql, params).execute(&pool).await?;
        Ok(result.rows_affected())
    }

    /// Attach `related_id` with additional pivot column values.
    pub async fn attach_with(
        self,
        related_id: impl Into<SqlValue>,
        extra: &[(&str, SqlValue)],
    ) -> Result<u64, sqlx::Error> {
        let pool = Self::pool()?;
        let mut cols = vec![self.fk, self.rfk];
        let mut ph = vec!["$1".to_string(), "$2".to_string()];
        let mut params = vec![self.owner_id, related_id.into()];

        for (i, (col, val)) in extra.iter().enumerate() {
            cols.push(col);
            ph.push(format!("${}", i + 3));
            params.push(val.clone());
        }

        let sql = format!(
            "INSERT INTO {through} ({cols}) VALUES ({ph}) ON CONFLICT DO NOTHING",
            through = self.through,
            cols = cols.join(", "),
            ph = ph.join(", "),
        );
        let result = sqlx_pg::build_query(&sql, params).execute(&pool).await?;
        Ok(result.rows_affected())
    }

    /// Detach `related_id` from the owner.
    pub async fn detach(self, related_id: impl Into<SqlValue>) -> Result<u64, sqlx::Error> {
        let pool = Self::pool()?;
        let sql = format!(
            "DELETE FROM {through} WHERE {fk} = $1 AND {rfk} = $2",
            through = self.through,
            fk = self.fk,
            rfk = self.rfk,
        );
        let params = vec![self.owner_id, related_id.into()];
        let result = sqlx_pg::build_query(&sql, params).execute(&pool).await?;
        Ok(result.rows_affected())
    }

    /// Remove all pivot rows for the owner (detach everything).
    pub async fn detach_all(self) -> Result<u64, sqlx::Error> {
        let pool = Self::pool()?;
        let sql = format!(
            "DELETE FROM {through} WHERE {fk} = $1",
            through = self.through,
            fk = self.fk,
        );
        let params = vec![self.owner_id];
        let result = sqlx_pg::build_query(&sql, params).execute(&pool).await?;
        Ok(result.rows_affected())
    }

    /// Sync: detach IDs not in `ids`, attach missing ones (idempotent).
    pub async fn sync(self, ids: &[impl Into<SqlValue> + Clone]) -> Result<(), sqlx::Error> {
        let pool = Self::pool()?;

        if ids.is_empty() {
            let sql = format!(
                "DELETE FROM {through} WHERE {fk} = $1",
                through = self.through,
                fk = self.fk,
            );
            sqlx_pg::build_query(&sql, vec![self.owner_id])
                .execute(&pool)
                .await?;
            return Ok(());
        }

        let id_vals: Vec<SqlValue> = ids.iter().map(|v| v.clone().into()).collect();

        let placeholders: Vec<String> = (2..=id_vals.len() + 1).map(|i| format!("${i}")).collect();
        let del_sql = format!(
            "DELETE FROM {through} WHERE {fk} = $1 AND {rfk} NOT IN ({ph})",
            through = self.through,
            fk = self.fk,
            rfk = self.rfk,
            ph = placeholders.join(", "),
        );
        let mut del_params = vec![self.owner_id.clone()];
        del_params.extend(id_vals.iter().cloned());
        sqlx_pg::build_query(&del_sql, del_params)
            .execute(&pool)
            .await?;

        for id_val in id_vals {
            let ins_sql = format!(
                "INSERT INTO {through} ({fk}, {rfk}) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                through = self.through,
                fk = self.fk,
                rfk = self.rfk,
            );
            sqlx_pg::build_query(&ins_sql, vec![self.owner_id.clone(), id_val])
                .execute(&pool)
                .await?;
        }

        Ok(())
    }

    /// Update extra pivot columns for an existing row (`related_id`).
    ///
    /// Only updates rows where both `fk = owner_id` AND `rfk = related_id`.
    pub async fn update_pivot(
        self,
        related_id: impl Into<SqlValue>,
        data: &[(&str, SqlValue)],
    ) -> Result<u64, sqlx::Error> {
        if data.is_empty() {
            return Ok(0);
        }
        let pool = Self::pool()?;
        let set_clauses: Vec<String> = data
            .iter()
            .enumerate()
            .map(|(i, (col, _))| format!("{col} = ${}", i + 3))
            .collect();
        let sql = format!(
            "UPDATE {through} SET {sets} WHERE {fk} = $1 AND {rfk} = $2",
            through = self.through,
            sets = set_clauses.join(", "),
            fk = self.fk,
            rfk = self.rfk,
        );
        let mut params = vec![self.owner_id, related_id.into()];
        params.extend(data.iter().map(|(_, v)| v.clone()));
        let result = sqlx_pg::build_query(&sql, params).execute(&pool).await?;
        Ok(result.rows_affected())
    }

    /// Toggle: attach IDs not present, detach IDs already present.
    pub async fn toggle(self, ids: &[impl Into<SqlValue> + Clone]) -> Result<(), sqlx::Error> {
        let pool = Self::pool()?;
        let id_vals: Vec<SqlValue> = ids.iter().map(|v| v.clone().into()).collect();

        if id_vals.is_empty() {
            return Ok(());
        }

        let placeholders: Vec<String> = (2..=id_vals.len() + 1).map(|i| format!("${i}")).collect();
        let sel_sql = format!(
            "SELECT {rfk} FROM {through} WHERE {fk} = $1 AND {rfk} IN ({ph})",
            through = self.through,
            fk = self.fk,
            rfk = self.rfk,
            ph = placeholders.join(", "),
        );
        let mut sel_params = vec![self.owner_id.clone()];
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
            let id_i64 = match &id_val {
                SqlValue::Integer(n) => Some(*n),
                _ => None,
            };
            let already_exists = id_i64.map(|n| existing.contains(&n)).unwrap_or(false);

            if already_exists {
                let del = format!(
                    "DELETE FROM {through} WHERE {fk} = $1 AND {rfk} = $2",
                    through = self.through,
                    fk = self.fk,
                    rfk = self.rfk,
                );
                sqlx_pg::build_query(&del, vec![self.owner_id.clone(), id_val])
                    .execute(&pool)
                    .await?;
            } else {
                let ins = format!(
                    "INSERT INTO {through} ({fk}, {rfk}) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                    through = self.through,
                    fk = self.fk,
                    rfk = self.rfk,
                );
                sqlx_pg::build_query(&ins, vec![self.owner_id.clone(), id_val])
                    .execute(&pool)
                    .await?;
            }
        }

        Ok(())
    }
}
