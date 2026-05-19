//! [`ModelQuery`] — lazy, fluent query that executes against the task-local pool.
//!
//! Created via [`PgModel::filter`], [`PgModel::all_query`], or
//! [`PgModel::find_query`]. Call a terminal method to execute.
//!
//! # Example
//!
//! ```rust,ignore
//! // All active users, newest first — no pool argument needed.
//! let users = User::filter("active", true)
//!     .order_by_desc("created_at")
//!     .limit(20)
//!     .get()
//!     .await?;
//!
//! // Count admins.
//! let n = User::filter("role", "admin").count().await?;
//!
//! // First active admin or 404.
//! let admin = User::filter("role", "admin")
//!     .and_where("active", true)
//!     .first_or_404()
//!     .await?;
//! ```

use rok_orm_core::{sqlx_pg, Condition, JoinOp, Model, QueryBuilder, SqlValue};
use serde::Serialize;
use sqlx::postgres::PgRow;

use crate::{executor, pagination, pool};

/// A lazy fluent query that reads the database pool from the current task scope.
///
/// Chain conditions with [`and_where`](Self::and_where), ordering with
/// [`order_by`](Self::order_by) / [`order_by_desc`](Self::order_by_desc), and
/// pagination with [`limit`](Self::limit) / [`offset`](Self::offset).
///
/// Finalize with `.get()`, `.first()`, `.first_or_404()`, or `.count()`.
pub struct ModelQuery<M> {
    builder: QueryBuilder<M>,
    /// When `false` (default) and `M::soft_delete_column().is_some()`, a
    /// `WHERE <col> IS NULL` clause is appended at execution time.
    include_trashed: bool,
    /// When `true`, registered global scopes are not applied.
    skip_scopes: bool,
    /// When `Some`, route this query to the named pool from the registry.
    named_db: Option<String>,
}

impl<M> ModelQuery<M>
where
    M: Model + for<'r> sqlx::FromRow<'r, PgRow> + Send + Sync + Unpin + 'static,
{
    pub(crate) fn new(builder: QueryBuilder<M>) -> Self {
        Self {
            builder,
            include_trashed: false,
            skip_scopes: false,
            named_db: None,
        }
    }

    fn into_final_builder(self) -> QueryBuilder<M> {
        let mut b = self.builder;
        // Apply global scopes unless opted out
        if !self.skip_scopes {
            b = crate::scopes::apply_scopes::<M>(b);
        }
        // Auto-filter soft-deleted rows
        if !self.include_trashed {
            if let Some(col) = M::soft_delete_column() {
                b = b.where_null(col);
            }
        }
        b
    }

    /// Returns the pool for this query: named pool if `.on()` was called, else task-local.
    fn pool_for_query(&self) -> Result<sqlx::PgPool, sqlx::Error> {
        if let Some(ref name) = self.named_db {
            return pool::get_named_pool(name).ok_or_else(|| {
                sqlx::Error::Configuration(
                    format!(
                        "no named pool '{name}' registered — call pool::register_named_pool()"
                    )
                    .into(),
                )
            });
        }
        pool::try_current_pool().ok_or_else(|| {
            sqlx::Error::Configuration(
                "no database pool in scope — add OrmLayer to your router or \
                 call pool::with_pool() in tests"
                    .to_string()
                    .into(),
            )
        })
    }

    /// Returns the named-pool override if `.on()` was called, `None` otherwise.
    fn named_pool_override(&self) -> Option<sqlx::PgPool> {
        self.named_db.as_deref().and_then(pool::get_named_pool)
    }

    // ── chaining ──────────────────────────────────────────────────────────────

    /// Add `AND col = val`.
    pub fn and_where(self, col: &str, val: impl Into<SqlValue>) -> Self {
        Self {
            builder: self.builder.where_eq(col, val),
            ..self
        }
    }

    /// Add `OR col = val`.
    pub fn or_where(self, col: &str, val: impl Into<SqlValue>) -> Self {
        Self {
            builder: self.builder.or_where_eq(col, val),
            ..self
        }
    }

    /// Add `AND col IS NULL`.
    pub fn and_where_null(self, col: &str) -> Self {
        Self {
            builder: self.builder.where_null(col),
            ..self
        }
    }

    /// Add `AND col IS NOT NULL`.
    pub fn and_where_not_null(self, col: &str) -> Self {
        Self {
            builder: self.builder.where_not_null(col),
            ..self
        }
    }

    /// Add `AND col LIKE pattern`.
    pub fn and_where_like(self, col: &str, pattern: &str) -> Self {
        Self {
            builder: self.builder.where_like(col, pattern),
            ..self
        }
    }

    /// Add `AND col IN (vals)`.
    pub fn and_where_in(self, col: &str, vals: Vec<impl Into<SqlValue>>) -> Self {
        Self {
            builder: self.builder.where_in(col, vals),
            ..self
        }
    }

    /// Add `AND col NOT IN (vals)`.
    pub fn and_where_not_in(self, col: &str, vals: Vec<impl Into<SqlValue>>) -> Self {
        Self {
            builder: self.builder.where_not_in(col, vals),
            ..self
        }
    }

    /// Add `AND col ILIKE pattern` (case-insensitive, PostgreSQL).
    pub fn and_where_ilike(self, col: &str, pattern: &str) -> Self {
        Self {
            builder: self.builder.where_ilike(col, pattern),
            ..self
        }
    }

    /// Add `AND col BETWEEN lo AND hi`.
    pub fn and_where_between(
        self,
        col: &str,
        lo: impl Into<SqlValue>,
        hi: impl Into<SqlValue>,
    ) -> Self {
        Self {
            builder: self.builder.where_between(col, lo, hi),
            ..self
        }
    }

    /// Add `AND col <op> val` where op is `=`, `!=`, `>`, `>=`, `<`, `<=`.
    pub fn and_where_op(self, col: &str, op: &str, val: impl Into<SqlValue>) -> Self {
        Self {
            builder: self.builder.where_op(col, op, val),
            ..self
        }
    }

    /// Add a raw AND sub-group: `AND (sub1 AND/OR sub2 …)`.
    pub fn and_where_group<F>(self, f: F) -> Self
    where
        F: FnOnce(QueryBuilder<M>) -> QueryBuilder<M>,
    {
        Self {
            builder: self.builder.where_group(f),
            ..self
        }
    }

    /// Add a raw OR sub-group: `OR (sub1 AND/OR sub2 …)`.
    pub fn or_where_group<F>(self, f: F) -> Self
    where
        F: FnOnce(QueryBuilder<M>) -> QueryBuilder<M>,
    {
        Self {
            builder: self.builder.or_where_group(f),
            ..self
        }
    }

    /// Add `AND col->>'key' = val` (JSONB text extraction, PostgreSQL).
    pub fn and_where_json(self, col: &str, key: &str, val: impl Into<SqlValue>) -> Self {
        Self {
            builder: self.builder.where_json(col, key, val),
            ..self
        }
    }

    /// Add `AND col @> 'json_val'::jsonb` (JSONB containment, PostgreSQL).
    pub fn and_where_json_contains(self, col: &str, json_val: &str) -> Self {
        Self {
            builder: self.builder.where_json_contains(col, json_val),
            ..self
        }
    }

    /// Add a raw WHERE fragment (`AND raw_sql`).
    pub fn and_where_raw(self, sql: &str) -> Self {
        Self {
            builder: self.builder.where_raw(sql),
            ..self
        }
    }

    /// Add `AND EXISTS (subquery_sql)`.
    pub fn and_where_exists_sql(self, subquery_sql: &str) -> Self {
        Self {
            builder: self.builder.where_raw(&format!("EXISTS ({subquery_sql})")),
            ..self
        }
    }

    /// Add `AND NOT EXISTS (subquery_sql)`.
    pub fn and_where_not_exists_sql(self, subquery_sql: &str) -> Self {
        Self {
            builder: self
                .builder
                .where_raw(&format!("NOT EXISTS ({subquery_sql})")),
            ..self
        }
    }

    /// Add `ORDER BY col ASC`.
    pub fn order_by(self, col: &str) -> Self {
        Self {
            builder: self.builder.order_by(col),
            ..self
        }
    }

    /// Add `ORDER BY col DESC`.
    pub fn order_by_desc(self, col: &str) -> Self {
        Self {
            builder: self.builder.order_by_desc(col),
            ..self
        }
    }

    /// Set `LIMIT n`.
    pub fn limit(self, n: usize) -> Self {
        Self {
            builder: self.builder.limit(n),
            ..self
        }
    }

    /// Set `OFFSET n`.
    pub fn offset(self, n: usize) -> Self {
        Self {
            builder: self.builder.offset(n),
            ..self
        }
    }

    /// Include soft-deleted rows in results (bypasses the auto IS NULL filter).
    pub fn with_trashed(mut self) -> Self {
        self.include_trashed = true;
        self
    }

    /// Bypass all registered global scopes for this query.
    pub fn without_global_scopes(mut self) -> Self {
        self.skip_scopes = true;
        self
    }

    /// Restrict query to only soft-deleted rows.
    pub fn only_trashed(mut self) -> Self {
        self.include_trashed = true;
        if let Some(col) = M::soft_delete_column() {
            self.builder = self.builder.where_not_null(col);
        }
        self
    }

    /// Select specific columns.
    pub fn select(self, cols: &[&str]) -> Self {
        Self {
            builder: self.builder.select(cols),
            ..self
        }
    }

    /// Select a raw SQL expression.
    pub fn select_raw(self, expr: &str) -> Self {
        Self {
            builder: self.builder.select_raw(expr),
            ..self
        }
    }

    /// Append a raw JOIN fragment.
    pub fn join_raw(self, raw: &str) -> Self {
        Self {
            builder: self.builder.join_raw(raw),
            ..self
        }
    }

    // ── locking ───────────────────────────────────────────────────────────────

    /// Append `FOR UPDATE` — prevents concurrent modifications (pessimistic lock).
    ///
    /// Best used inside a transaction so the lock is released when the
    /// transaction commits or rolls back.
    pub fn lock_for_update(self) -> Self {
        Self {
            builder: self.builder.for_update(),
            ..self
        }
    }

    /// Append `FOR UPDATE NOWAIT` — like `lock_for_update` but fails immediately
    /// if another transaction holds the lock.
    pub fn lock_for_update_nowait(self) -> Self {
        Self {
            builder: self.builder.for_update().nowait(),
            ..self
        }
    }

    /// Append `FOR SHARE` — allows concurrent reads but blocks concurrent writes.
    pub fn lock_for_share(self) -> Self {
        Self {
            builder: self.builder.for_share(),
            ..self
        }
    }

    /// Append `FOR SHARE SKIP LOCKED` — skips rows locked by other transactions.
    pub fn lock_for_share_skip_locked(self) -> Self {
        Self {
            builder: self.builder.for_share().skip_locked(),
            ..self
        }
    }

    // ── multi-database routing ────────────────────────────────────────────────

    /// Route this query to a named database pool registered via
    /// [`pool::register_named_pool`](crate::pool::register_named_pool).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// rok_orm::pool::register_named_pool("analytics", analytics_pool);
    ///
    /// let rows = Report::all_query().on("analytics").get().await?;
    /// ```
    pub fn on(mut self, db_name: impl Into<String>) -> Self {
        self.named_db = Some(db_name.into());
        self
    }

    // ── replica routing ───────────────────────────────────────────────────────

    /// Route this query to the read replica pool (default behaviour).
    ///
    /// Queries use the replica pool by default.  Call this only after
    /// `on_write_db()` to re-enable replica routing for a subsequent chain.
    pub fn on_replica(self) -> Self {
        let mut builder = self.builder;
        builder.use_replica = true;
        Self { builder, ..self }
    }

    /// Force this query to the primary (write) pool, bypassing read replicas.
    pub fn on_write_db(self) -> Self {
        Self {
            builder: self.builder.on_write_db(),
            ..self
        }
    }

    // ── subquery helpers ──────────────────────────────────────────────────────

    /// Add `AND col IN (SELECT … from subquery_sql)`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Fetch posts by active users only
    /// let active_ids_sql = "SELECT id FROM users WHERE active = true";
    /// let posts = Post::all_query()
    ///     .where_in_subquery("user_id", active_ids_sql)
    ///     .get()
    ///     .await?;
    /// ```
    pub fn where_in_subquery(self, col: &str, subquery_sql: &str) -> Self {
        Self {
            builder: self
                .builder
                .where_raw(&format!("{col} IN ({subquery_sql})")),
            ..self
        }
    }

    /// Add `AND col NOT IN (SELECT … from subquery_sql)`.
    pub fn where_not_in_subquery(self, col: &str, subquery_sql: &str) -> Self {
        Self {
            builder: self
                .builder
                .where_raw(&format!("{col} NOT IN ({subquery_sql})")),
            ..self
        }
    }

    /// Add `AND EXISTS (subquery_sql)`.
    ///
    /// ```rust,ignore
    /// let users_with_posts = User::all_query()
    ///     .where_exists_sql("SELECT 1 FROM posts WHERE posts.user_id = users.id")
    ///     .get().await?;
    /// ```
    pub fn where_exists_sql(self, subquery_sql: &str) -> Self {
        Self {
            builder: self
                .builder
                .where_raw(&format!("EXISTS ({subquery_sql})")),
            ..self
        }
    }

    /// `GROUP BY` a raw expression (e.g. `"EXTRACT(YEAR FROM created_at)"`).
    pub fn group_by_raw(self, expr: &str) -> Self {
        Self {
            builder: self.builder.group_by(&[expr]),
            ..self
        }
    }

    /// `HAVING` a raw expression (e.g. `"COUNT(*) > 100"`).
    pub fn having_raw(self, expr: &str) -> Self {
        Self {
            builder: self.builder.having(expr),
            ..self
        }
    }

    /// Filter where `col` equals another column reference (no parameterisation).
    ///
    /// Useful for correlated subqueries or joins:
    /// ```rust,ignore
    /// Post::all_query().where_column("user_id", "users.id")
    /// ```
    pub fn where_column(self, col: &str, other_col: &str) -> Self {
        Self {
            builder: self
                .builder
                .where_raw(&format!("{col} = {other_col}")),
            ..self
        }
    }

    // ── relationship aggregates ───────────────────────────────────────────────

    /// Append `(SELECT COUNT(*) FROM {rel} WHERE {rel}.{fk} = {parent}.id) AS {rel}_count`
    /// to the SELECT list.  FK is inferred as `{singular(parent_table)}_id`.
    ///
    /// ```rust,ignore
    /// let users = User::all_query().with_count("posts").get().await?;
    /// // users[0].posts_count — available via .try_get::<i64, _>("posts_count")
    /// ```
    pub fn with_count(self, relation: &str) -> Self {
        let parent = M::table_name();
        let fk = format!("{}_id", naive_singular(parent));
        let expr = format!(
            "(SELECT COUNT(*) FROM {relation} WHERE {relation}.{fk} = {parent}.id) AS {relation}_count"
        );
        Self {
            builder: self.builder.add_select_expr(expr),
            ..self
        }
    }

    /// Append `(SELECT SUM(rel.col) … ) AS {rel}_sum_{col}` to the SELECT list.
    pub fn with_sum(self, relation: &str, col: &str) -> Self {
        self.rel_agg(relation, "SUM", col)
    }

    /// Append `(SELECT AVG(rel.col) … ) AS {rel}_avg_{col}` to the SELECT list.
    pub fn with_avg(self, relation: &str, col: &str) -> Self {
        self.rel_agg(relation, "AVG", col)
    }

    /// Append `(SELECT MIN(rel.col) … ) AS {rel}_min_{col}` to the SELECT list.
    pub fn with_min(self, relation: &str, col: &str) -> Self {
        self.rel_agg(relation, "MIN", col)
    }

    /// Append `(SELECT MAX(rel.col) … ) AS {rel}_max_{col}` to the SELECT list.
    pub fn with_max(self, relation: &str, col: &str) -> Self {
        self.rel_agg(relation, "MAX", col)
    }

    fn rel_agg(self, relation: &str, agg: &str, col: &str) -> Self {
        let parent = M::table_name();
        let fk = format!("{}_id", naive_singular(parent));
        let alias = format!("{relation}_{}_{col}", agg.to_lowercase());
        let expr = format!(
            "(SELECT {agg}({relation}.{col}) FROM {relation} WHERE {relation}.{fk} = {parent}.id) AS {alias}"
        );
        Self {
            builder: self.builder.add_select_expr(expr),
            ..self
        }
    }

    // ── eager loading ─────────────────────────────────────────────────────────

    /// Eagerly load a named relation — returns an [`EagerModelQuery`] that
    /// requires `M: EagerLoadable`.
    ///
    /// Chain multiple `.with()` calls to load several relations at once.
    /// The actual loading happens when you call `.get()` on the result.
    ///
    /// ```rust,ignore
    /// let users = User::all_query()
    ///     .with("posts")
    ///     .with("profile")
    ///     .get()
    ///     .await?;
    /// ```
    #[cfg(feature = "postgres")]
    pub fn with(self, relation: impl Into<String>) -> crate::eager::EagerModelQuery<M> {
        crate::eager::EagerModelQuery {
            query: self,
            relations: vec![relation.into()],
        }
    }

    // ── relationship existence ────────────────────────────────────────────────

    /// Add `AND EXISTS (SELECT 1 FROM {rel} WHERE {rel}.{fk} = {parent}.id)`.
    pub fn has(self, relation: &str) -> Self {
        let cond = self.subquery_cond(relation, true, vec![]);
        Self {
            builder: self.builder.push_condition(JoinOp::And, cond),
            ..self
        }
    }

    /// Add `AND NOT EXISTS (SELECT 1 FROM {rel} WHERE {rel}.{fk} = {parent}.id)`.
    pub fn doesnt_have(self, relation: &str) -> Self {
        let cond = self.subquery_cond(relation, false, vec![]);
        Self {
            builder: self.builder.push_condition(JoinOp::And, cond),
            ..self
        }
    }

    /// Add `AND EXISTS (SELECT 1 FROM {rel} WHERE {rel}.{fk} = {parent}.id AND <closure conds>)`.
    pub fn where_has<F>(self, relation: &str, f: F) -> Self
    where
        F: FnOnce(QueryBuilder<M>) -> QueryBuilder<M>,
    {
        let inner_builder = f(QueryBuilder::new(relation));
        let inner = inner_builder.conditions().to_vec();
        let cond = self.subquery_cond(relation, true, inner);
        Self {
            builder: self.builder.push_condition(JoinOp::And, cond),
            ..self
        }
    }

    /// Add `AND NOT EXISTS (SELECT 1 FROM {rel} WHERE {rel}.{fk} = {parent}.id AND <closure conds>)`.
    pub fn where_doesnt_have<F>(self, relation: &str, f: F) -> Self
    where
        F: FnOnce(QueryBuilder<M>) -> QueryBuilder<M>,
    {
        let inner_builder = f(QueryBuilder::new(relation));
        let inner = inner_builder.conditions().to_vec();
        let cond = self.subquery_cond(relation, false, inner);
        Self {
            builder: self.builder.push_condition(JoinOp::And, cond),
            ..self
        }
    }

    fn subquery_cond(
        &self,
        relation: &str,
        exists: bool,
        inner: Vec<(JoinOp, Condition)>,
    ) -> Condition {
        let parent = M::table_name();
        let fk = format!("{}_id", naive_singular(parent));
        Condition::Subquery {
            exists,
            table: relation.to_string(),
            fk_expr: format!("{relation}.{fk} = {parent}.id"),
            inner,
        }
    }

    // ── terminals ─────────────────────────────────────────────────────────────

    /// Fetch all matching rows.
    pub async fn get(self) -> Result<Vec<M>, sqlx::Error> {
        let table = M::table_name();
        let pool_override = self.named_pool_override();
        let builder = self.into_final_builder();
        let rows = match pool_override {
            Some(p) => pool::with_pool(p, pool::fetch_all(builder)).await?,
            None => pool::fetch_all(builder).await?,
        };
        crate::n1::record(table, rows.len());
        Ok(rows)
    }

    /// Fetch the first matching row, or `None`.
    pub async fn first(self) -> Result<Option<M>, sqlx::Error> {
        let pool_override = self.named_pool_override();
        let builder = self.into_final_builder().limit(1);
        match pool_override {
            Some(p) => pool::with_pool(p, pool::fetch_optional(builder)).await,
            None => pool::fetch_optional(builder).await,
        }
    }

    /// Fetch the first matching row or return [`sqlx::Error::RowNotFound`].
    pub async fn first_or_404(self) -> Result<M, sqlx::Error> {
        self.first().await?.ok_or(sqlx::Error::RowNotFound)
    }

    /// Return the count of matching rows.
    pub async fn count(self) -> Result<i64, sqlx::Error> {
        let pool_override = self.named_pool_override();
        let builder = self.into_final_builder();
        match pool_override {
            Some(p) => pool::with_pool(p, pool::count(builder)).await,
            None => pool::count(builder).await,
        }
    }

    /// Hard-delete all matching rows, even on soft-delete models.
    pub async fn force_delete(self) -> Result<u64, sqlx::Error> {
        let pool = self.pool_for_query()?;
        executor::delete(&pool, self.builder).await
    }

    /// Soft-delete all matching rows (`UPDATE SET <deleted_at> = NOW()`).
    ///
    /// Only meaningful on models with `#[rok_orm(soft_delete)]`.
    pub async fn soft_delete(self) -> Result<u64, sqlx::Error> {
        let col = M::soft_delete_column().unwrap_or("deleted_at");
        let pool = self.pool_for_query()?;
        executor::soft_delete::<M>(&pool, self.builder, col).await
    }

    /// Restore soft-deleted matching rows — sets the soft-delete column to `NULL`.
    ///
    /// Only meaningful on models with `#[rok_orm(soft_delete)]`.
    pub async fn restore(self) -> Result<u64, sqlx::Error> {
        let col = M::soft_delete_column().unwrap_or("deleted_at");
        let pool = self.pool_for_query()?;
        executor::restore::<M>(&pool, self.builder, col).await
    }

    // ── streaming ─────────────────────────────────────────────────────────────

    /// Stream matching rows lazily — O(1) memory regardless of result set size.
    ///
    /// Internally spawns a task that drives the cursor and sends each row
    /// through a bounded channel. Use `futures::StreamExt` or similar to drive.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use futures::StreamExt as _;
    ///
    /// let mut s = User::all_query().stream();
    /// while let Some(row) = s.next().await {
    ///     process(row?);
    /// }
    /// ```
    pub fn stream(
        self,
    ) -> impl futures_core::Stream<Item = Result<M, sqlx::Error>> + Send + 'static {
        let pool_result = self.pool_for_query();
        let (sql, params) = self.into_final_builder().to_sql();
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<M, sqlx::Error>>(64);
        tokio::spawn(async move {
            let pool = match pool_result {
                Ok(p) => p,
                Err(e) => {
                    let _ = tx.send(Err(e)).await;
                    return;
                }
            };
            let mut s = sqlx_pg::build_query_as::<M>(&sql, params).fetch(&pool);
            use futures::TryStreamExt as _;
            while let Some(result) = s.try_next().await.transpose() {
                if tx.send(result).await.is_err() {
                    break;
                }
            }
        });
        MpscStream(rx)
    }

    // ── aggregates ────────────────────────────────────────────────────────────

    /// Return `MAX(col)` for matching rows, or `None` if no rows match.
    pub async fn max(self, col: &str) -> Result<Option<f64>, sqlx::Error> {
        let pool_override = self.named_pool_override();
        let builder = self.into_final_builder();
        let agg = format!("MAX({col})");
        match pool_override {
            Some(p) => pool::with_pool(p, pool::aggregate(builder, &agg)).await,
            None => pool::aggregate(builder, &agg).await,
        }
    }

    /// Return `MIN(col)` for matching rows, or `None` if no rows match.
    pub async fn min(self, col: &str) -> Result<Option<f64>, sqlx::Error> {
        let pool_override = self.named_pool_override();
        let builder = self.into_final_builder();
        let agg = format!("MIN({col})");
        match pool_override {
            Some(p) => pool::with_pool(p, pool::aggregate(builder, &agg)).await,
            None => pool::aggregate(builder, &agg).await,
        }
    }

    /// Return `SUM(col)` for matching rows, or `None` if no rows match.
    pub async fn sum(self, col: &str) -> Result<Option<f64>, sqlx::Error> {
        let pool_override = self.named_pool_override();
        let builder = self.into_final_builder();
        let agg = format!("SUM({col})");
        match pool_override {
            Some(p) => pool::with_pool(p, pool::aggregate(builder, &agg)).await,
            None => pool::aggregate(builder, &agg).await,
        }
    }

    /// Return `AVG(col)` for matching rows, or `None` if no rows match.
    pub async fn avg(self, col: &str) -> Result<Option<f64>, sqlx::Error> {
        let pool_override = self.named_pool_override();
        let builder = self.into_final_builder();
        let agg = format!("AVG({col})");
        match pool_override {
            Some(p) => pool::with_pool(p, pool::aggregate(builder, &agg)).await,
            None => pool::aggregate(builder, &agg).await,
        }
    }

    /// Return `true` if at least one row matches.
    pub async fn exists(self) -> Result<bool, sqlx::Error> {
        Ok(self.count().await? > 0)
    }

    /// Return `true` if no rows match.
    pub async fn doesnt_exist(self) -> Result<bool, sqlx::Error> {
        Ok(self.count().await? == 0)
    }

    // ── additional retrieval terminals ────────────────────────────────────────

    /// Fetch the last matching row (reverses the current order or falls back to primary key DESC).
    pub async fn last(self) -> Result<Option<M>, sqlx::Error> {
        let pool_override = self.named_pool_override();
        let builder = self.into_final_builder().reorder_desc(M::primary_key()).limit(1);
        match pool_override {
            Some(p) => pool::with_pool(p, pool::fetch_optional(builder)).await,
            None => pool::fetch_optional(builder).await,
        }
    }

    /// Fetch the first matching row or return [`sqlx::Error::RowNotFound`].
    pub async fn find_or_fail(self) -> Result<M, sqlx::Error> {
        self.first_or_404().await
    }

    /// Fetch a single column value from the first matching row.
    ///
    /// Returns `None` when no row matches.  The column must be text-compatible.
    pub async fn value(self, col: &str) -> Result<Option<String>, sqlx::Error> {
        let pool = self.pool_for_query()?;
        let builder = self.into_final_builder().select(&[col]).limit(1);
        let (sql, params) = builder.to_sql();
        let row = sqlx_pg::build_query(&sql, params)
            .fetch_optional(&pool)
            .await?;
        use sqlx::Row;
        Ok(row.and_then(|r| r.try_get::<Option<String>, _>(0).ok().flatten()))
    }

    /// Fetch a single column from all matching rows as a flat `Vec<String>`.
    pub async fn pluck(self, col: &str) -> Result<Vec<String>, sqlx::Error> {
        let pool = self.pool_for_query()?;
        let builder = self.into_final_builder().select(&[col]);
        let (sql, params) = builder.to_sql();
        let rows = sqlx_pg::build_query(&sql, params).fetch_all(&pool).await?;
        use sqlx::Row;
        let vals = rows
            .into_iter()
            .filter_map(|r| r.try_get::<String, _>(0).ok())
            .collect();
        Ok(vals)
    }

    /// Fetch two columns from all matching rows as a `Vec<(String, String)>`.
    ///
    /// Useful for building dropdown option lists.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let options: Vec<(String, String)> = Category::all_query()
    ///     .order_by("name")
    ///     .pairs("id", "name")
    ///     .await?;
    /// // [("1", "Electronics"), ("2", "Books"), ...]
    /// ```
    pub async fn pairs(self, col1: &str, col2: &str) -> Result<Vec<(String, String)>, sqlx::Error> {
        let pool = self.pool_for_query()?;
        let builder = self.into_final_builder().select(&[col1, col2]);
        let (sql, params) = builder.to_sql();
        let rows = sqlx_pg::build_query(&sql, params).fetch_all(&pool).await?;
        use sqlx::Row;
        let vals = rows
            .into_iter()
            .filter_map(|r| {
                let a = r.try_get::<String, _>(0).ok()?;
                let b = r.try_get::<String, _>(1).ok()?;
                Some((a, b))
            })
            .collect();
        Ok(vals)
    }

    // ── chunking ──────────────────────────────────────────────────────────────

    /// Process rows in batches of `size` without loading the full result set.
    ///
    /// The closure receives each `Vec<M>` batch and must return `Ok(())` to continue.
    /// Returning `Err(e)` from the closure aborts iteration.
    pub async fn chunk<F, Fut, E>(self, size: usize, mut f: F) -> Result<(), E>
    where
        M: Clone,
        F: FnMut(Vec<M>) -> Fut,
        Fut: std::future::Future<Output = Result<(), E>>,
        E: From<sqlx::Error>,
    {
        let pool_override = self.named_pool_override();
        let base = self.into_final_builder().order_by(M::primary_key());
        let mut offset = 0usize;
        loop {
            let batch = match &pool_override {
                Some(p) => {
                    pool::with_pool(p.clone(), pool::fetch_all(base.clone().limit(size).offset(offset)))
                        .await
                        .map_err(E::from)?
                }
                None => pool::fetch_all(base.clone().limit(size).offset(offset))
                    .await
                    .map_err(E::from)?,
            };
            let is_last = batch.len() < size;
            f(batch).await?;
            if is_last {
                break;
            }
            offset += size;
        }
        Ok(())
    }

    /// Alias for [`chunk`](Self::chunk) — processes rows ordered by primary key in batches.
    pub async fn chunk_by_id<F, Fut, E>(self, size: usize, f: F) -> Result<(), E>
    where
        M: Clone,
        F: FnMut(Vec<M>) -> Fut,
        Fut: std::future::Future<Output = Result<(), E>>,
        E: From<sqlx::Error>,
    {
        self.chunk(size, f).await
    }

    // ── pagination ────────────────────────────────────────────────────────────

    /// Fetch a page of results with total count and links.
    ///
    /// Runs two queries: `SELECT COUNT(*)` + the data query.
    pub async fn paginate(
        self,
        per_page: u32,
        current_page: u32,
    ) -> Result<pagination::Page<M>, sqlx::Error>
    where
        M: Serialize + Clone,
    {
        let pool_override = self.named_pool_override();
        let final_builder = self.into_final_builder();
        let offset = ((current_page.saturating_sub(1)) as usize) * (per_page as usize);
        let (total, data) = match pool_override {
            Some(p) => {
                let total = pool::with_pool(p.clone(), pool::count(final_builder.clone())).await?;
                let data = pool::with_pool(
                    p,
                    pool::fetch_all(final_builder.limit(per_page as usize).offset(offset)),
                )
                .await?;
                (total, data)
            }
            None => {
                let total = pool::count(final_builder.clone()).await?;
                let data =
                    pool::fetch_all(final_builder.limit(per_page as usize).offset(offset)).await?;
                (total, data)
            }
        };
        Ok(pagination::Page::new(data, total, per_page, current_page))
    }

    /// Fetch a page of results without a total count (faster for large tables).
    ///
    /// Fetches `per_page + 1` rows to determine whether a next page exists.
    pub async fn simple_paginate(
        self,
        per_page: u32,
        current_page: u32,
    ) -> Result<pagination::SimplePage<M>, sqlx::Error>
    where
        M: Serialize,
    {
        let pool_override = self.named_pool_override();
        let offset = ((current_page.saturating_sub(1)) as usize) * (per_page as usize);
        let builder = self
            .into_final_builder()
            .limit(per_page as usize + 1)
            .offset(offset);
        let data = match pool_override {
            Some(p) => pool::with_pool(p, pool::fetch_all(builder)).await?,
            None => pool::fetch_all(builder).await?,
        };
        Ok(pagination::SimplePage::new(data, per_page, current_page))
    }

    /// Keyset (cursor) pagination — O(1) regardless of depth.
    ///
    /// `cursor_col` must be the column used for ordering (typically `"id"`).
    /// Pass `None` as `cursor` for the first page.
    pub async fn cursor_paginate(
        self,
        per_page: u32,
        cursor_col: &str,
        cursor: Option<&str>,
    ) -> Result<pagination::CursorPage<M>, sqlx::Error>
    where
        M: Serialize,
    {
        let pool_override = self.named_pool_override();
        let mut b = self.into_final_builder();

        let prev_cursor: Option<String>;

        if let Some(token) = cursor {
            if let Some(id) = pagination::decode_cursor(token) {
                b = b.where_gt(cursor_col, id);
                prev_cursor = Some(pagination::encode_cursor(id));
            } else {
                prev_cursor = None;
            }
        } else {
            prev_cursor = None;
        }

        let fetch_builder = b.order_by(cursor_col).limit(per_page as usize + 1);
        let data = match pool_override {
            Some(p) => pool::with_pool(p, pool::fetch_all(fetch_builder)).await?,
            None => pool::fetch_all(fetch_builder).await?,
        };

        let next_id_cursor: Option<String> = if data.len() > per_page as usize {
            match data[per_page as usize - 1].pk_value() {
                SqlValue::Integer(pk) => Some(pagination::encode_cursor(pk)),
                _ => None,
            }
        } else {
            None
        };

        Ok(pagination::CursorPage::new(
            data,
            per_page,
            next_id_cursor,
            prev_cursor,
        ))
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

/// Thin `Stream` wrapper over a `tokio::sync::mpsc::Receiver`.
///
/// Used internally by [`ModelQuery::stream`] to yield rows from a spawned task.
struct MpscStream<T>(tokio::sync::mpsc::Receiver<T>);

impl<T: Unpin> futures_core::Stream for MpscStream<T> {
    type Item = T;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<T>> {
        self.0.poll_recv(cx)
    }
}

/// Naive English singularization used for FK inference: `users` → `user`, `categories` → `category`.
fn naive_singular(table: &str) -> String {
    if let Some(s) = table.strip_suffix("ies") {
        format!("{s}y")
    } else if let Some(s) = table.strip_suffix('s') {
        s.to_string()
    } else {
        table.to_string()
    }
}
