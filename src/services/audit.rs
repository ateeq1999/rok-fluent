//! [`AuditService<M>`] — timestamp helpers for models with `created_at` / `updated_at`.

use crate::core::condition::SqlValue;
use crate::orm::postgres::model::PgModel;

/// Timestamp and audit-log helpers for Active Record models.
///
/// `touch` works on any model. `history` requires an `audit_log` table with columns:
/// `table_name TEXT, record_id TEXT, operation TEXT, old_data JSONB, new_data JSONB, changed_at TIMESTAMPTZ`.
///
/// # Example
///
/// ```rust,no_run
/// # use rok_fluent::services::AuditService;
/// # async fn run() -> Result<(), sqlx::Error> {
/// # let pool: sqlx::PgPool = todo!();
/// // AuditService::<Post>::touch(42_i64, &pool).await?;
/// // let entries = AuditService::<Post>::history(42_i64, &pool).await?;
/// # Ok(())
/// # }
/// ```
pub struct AuditService<M>(std::marker::PhantomData<M>);

/// A single row from the `audit_log` table.
#[derive(Debug, Clone)]
pub struct AuditEntry {
    /// Table the change was made on.
    pub table_name: String,
    /// Primary key value of the changed row, stored as text.
    pub record_id: String,
    /// `"INSERT"`, `"UPDATE"`, or `"DELETE"`.
    pub operation: String,
    /// Row snapshot before the change (`None` for INSERT).
    pub old_data: Option<serde_json::Value>,
    /// Row snapshot after the change (`None` for DELETE).
    pub new_data: Option<serde_json::Value>,
    /// When the change occurred.
    pub changed_at: chrono::DateTime<chrono::Utc>,
}

impl<M: PgModel> AuditService<M> {
    /// Set `updated_at = NOW()` for the row with the given primary key.
    ///
    /// Silently succeeds (0 rows affected) when `updated_at` is not a column on the model.
    pub async fn touch(
        id: impl Into<SqlValue> + Send,
        pool: &sqlx::PgPool,
    ) -> Result<u64, sqlx::Error> {
        M::touch_by_pk(pool, id.into()).await
    }

    /// Return audit log entries for the row with the given primary key, newest first.
    ///
    /// Requires an `audit_log` table. Returns an empty `Vec` gracefully when the table
    /// does not exist, so callers can safely call this on any model.
    pub async fn history(
        id: impl Into<SqlValue> + Send,
        pool: &sqlx::PgPool,
    ) -> Result<Vec<AuditEntry>, sqlx::Error> {
        let id_str = id_to_string(id.into());
        let table = M::table_name();

        let rows = sqlx::query(
            "SELECT table_name, record_id, operation, old_data, new_data, changed_at \
             FROM audit_log \
             WHERE table_name = $1 AND record_id = $2 \
             ORDER BY changed_at DESC",
        )
        .bind(table)
        .bind(&id_str)
        .fetch_all(pool)
        .await;

        match rows {
            Ok(rows) => {
                use sqlx::Row;
                rows.into_iter()
                    .map(|r| {
                        Ok(AuditEntry {
                            table_name: r.try_get("table_name")?,
                            record_id: r.try_get("record_id")?,
                            operation: r.try_get("operation")?,
                            old_data: r.try_get("old_data")?,
                            new_data: r.try_get("new_data")?,
                            changed_at: r.try_get("changed_at")?,
                        })
                    })
                    .collect()
            }
            Err(sqlx::Error::Database(ref e))
                if e.message().contains("audit_log") || e.message().contains("does not exist") =>
            {
                Ok(Vec::new())
            }
            Err(e) => Err(e),
        }
    }
}

fn id_to_string(val: SqlValue) -> String {
    match val {
        SqlValue::Integer(n) => n.to_string(),
        SqlValue::Text(s) => s,
        SqlValue::Uuid(u) => u.to_string(),
        other => format!("{other:?}"),
    }
}
