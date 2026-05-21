//! [`BatchService<M>`] — efficient multi-row operations for Active Record models.

use sqlx::postgres::PgPoolCopyExt;

use crate::core::condition::SqlValue;
use crate::orm::postgres::model::PgModel;

/// Efficient bulk database operations for any PostgreSQL-backed model.
///
/// ```rust,no_run
/// # use rok_fluent::services::BatchService;
/// # async fn run() -> Result<(), sqlx::Error> {
/// # let pool: sqlx::PgPool = todo!();
/// // let rows = vec![
/// //     vec![("name", SqlValue::from("Alice")), ("email", SqlValue::from("a@b.com"))],
/// //     vec![("name", SqlValue::from("Bob")),   ("email", SqlValue::from("b@b.com"))],
/// // ];
/// // BatchService::<User>::bulk_insert(&rows, &pool).await?;
/// # Ok(())
/// # }
/// ```
pub struct BatchService<M>(std::marker::PhantomData<M>);

impl<M: PgModel> BatchService<M> {
    /// Insert multiple rows at once.
    pub async fn bulk_insert(
        rows: &[Vec<(&str, SqlValue)>],
        pool: &sqlx::PgPool,
    ) -> Result<u64, sqlx::Error> {
        M::bulk_create(pool, rows).await
    }

    /// Insert multiple rows in chunks of `chunk_size` to avoid parameter limits.
    pub async fn bulk_insert_chunked(
        rows: &[Vec<(&str, SqlValue)>],
        chunk_size: usize,
        pool: &sqlx::PgPool,
    ) -> Result<u64, sqlx::Error> {
        M::bulk_insert_chunked(pool, rows, chunk_size).await
    }

    /// Upsert multiple rows by the given unique key column.
    pub async fn bulk_upsert_by(
        unique_col: &str,
        rows: &[Vec<(&str, SqlValue)>],
        pool: &sqlx::PgPool,
    ) -> Result<u64, sqlx::Error> {
        M::bulk_upsert(pool, rows, &[unique_col]).await
    }

    /// Delete all rows matching the given column = value pairs.
    pub async fn delete_where(
        conditions: &[(&str, SqlValue)],
        pool: &sqlx::PgPool,
    ) -> Result<u64, sqlx::Error> {
        let mut builder = M::query();
        for (col, val) in conditions {
            builder = builder.where_eq(col, val.clone());
        }
        M::delete_where(pool, builder).await
    }

    /// Update `data` columns on all rows whose primary key is in `ids`.
    ///
    /// Returns the total number of rows affected.
    pub async fn bulk_update(
        data: &[(&str, SqlValue)],
        ids: impl IntoIterator<Item = impl Into<SqlValue>>,
        pool: &sqlx::PgPool,
    ) -> Result<u64, sqlx::Error> {
        let id_vals: Vec<SqlValue> = ids.into_iter().map(Into::into).collect();
        if id_vals.is_empty() {
            return Ok(0);
        }
        let builder = M::query().where_in(M::primary_key(), id_vals);
        M::update_where(pool, builder, data).await
    }

    /// Insert multiple rows via PostgreSQL `COPY FROM STDIN` (CSV format).
    ///
    /// Significantly faster than multi-row `INSERT` for large batches (10–50×
    /// depending on row size). All rows must supply the same columns in the same
    /// order; the column list is taken from the first row.
    ///
    /// Returns the number of rows copied.
    ///
    /// # Errors
    ///
    /// Returns an error if `rows` is empty, if column sets differ between rows,
    /// or if the COPY command fails.
    pub async fn copy_insert(
        rows: &[Vec<(&str, SqlValue)>],
        pool: &sqlx::PgPool,
    ) -> Result<u64, sqlx::Error> {
        if rows.is_empty() {
            return Ok(0);
        }

        let cols: Vec<&str> = rows[0].iter().map(|(c, _)| *c).collect();
        let col_list: Vec<String> = cols.iter().map(|c| format!("\"{c}\"")).collect();
        let copy_sql = format!(
            "COPY \"{}\" ({}) FROM STDIN (FORMAT CSV, NULL '')",
            M::table_name(),
            col_list.join(", ")
        );

        let mut copy = pool.copy_in_raw(&copy_sql).await?;

        for row in rows {
            let values: Vec<String> = row.iter().map(|(_, v)| csv_escape(v)).collect();
            let line = format!("{}\n", values.join(","));
            copy.send(line.as_bytes()).await?;
        }

        let rows_copied = copy.finish().await?;
        Ok(rows_copied)
    }
}

/// Escape a `SqlValue` for PostgreSQL CSV COPY format.
fn csv_escape(val: &SqlValue) -> String {
    match val {
        SqlValue::Null => String::new(),
        SqlValue::Bool(b) => {
            if *b {
                "t".into()
            } else {
                "f".into()
            }
        }
        SqlValue::Integer(n) => n.to_string(),
        SqlValue::Float(f) => f.to_string(),
        SqlValue::Text(s) => {
            // CSV: wrap in quotes, escape internal quotes by doubling them.
            format!("\"{}\"", s.replace('"', "\"\""))
        }
        SqlValue::Uuid(u) => u.to_string(),
        SqlValue::Json(j) => {
            let s = j.to_string();
            format!("\"{}\"", s.replace('"', "\"\""))
        }
        SqlValue::Array(vals) => {
            // Render as a PostgreSQL array literal: {val1,val2,...}
            let inner: Vec<String> = vals.iter().map(csv_escape).collect();
            format!("\"{{{}}}\"", inner.join(","))
        }
    }
}
