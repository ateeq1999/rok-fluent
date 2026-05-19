//! rok-orm-factory — model factories for tests and seeding.
//!
//! # Quick start
//!
//! Define a factory by implementing [`Factory`] on your model:
//!
//! ```rust,ignore
//! use rok_orm_factory::{Factory, Faker};
//!
//! impl Factory for User {
//!     fn definition() -> Self {
//!         User {
//!             id: 0,
//!             name:  Faker::name(),
//!             email: Faker::email(),
//!             active: true,
//!         }
//!     }
//! }
//!
//! // Make models in-memory (no DB)
//! let user  = User::factory().make();
//! let users = User::factory().count(5).make_many();
//!
//! // Override specific fields
//! let admin = User::factory()
//!     .with(|u| u.name = "Admin".to_string())
//!     .make();
//!
//! // Persist to DB (requires `features = ["postgres"]` and a pool in scope)
//! let user = User::factory().create().await?;
//! let users = User::factory().count(3).create_many().await?;
//! ```

pub mod faker;

pub use faker::Faker;

// ── Factory trait ─────────────────────────────────────────────────────────────

/// Implement this on a model to enable the factory DSL.
///
/// The `definition()` method returns a default instance with fake data.
pub trait Factory: Sized + 'static {
    /// Return a model instance filled with fake/default data.
    fn definition() -> Self;

    /// Access the [`FactoryBuilder`] for this model.
    fn factory() -> FactoryBuilder<Self> {
        FactoryBuilder::new()
    }
}

// ── FactoryBuilder ────────────────────────────────────────────────────────────

/// Fluent builder for creating model instances.
#[allow(clippy::type_complexity)]
pub struct FactoryBuilder<T: Factory> {
    count: usize,
    overrides: Vec<Box<dyn Fn(&mut T)>>,
}

impl<T: Factory> FactoryBuilder<T> {
    pub fn new() -> Self {
        Self {
            count: 1,
            overrides: Vec::new(),
        }
    }

    /// Set how many instances to create.
    pub fn count(mut self, n: usize) -> Self {
        self.count = n;
        self
    }

    /// Apply a field override closure to each generated model.
    pub fn with(mut self, f: impl Fn(&mut T) + 'static) -> Self {
        self.overrides.push(Box::new(f));
        self
    }

    fn build_one(&self) -> T {
        let mut model = T::definition();
        for ov in &self.overrides {
            ov(&mut model);
        }
        model
    }

    /// Build one model in-memory using the factory definition.
    pub fn make(self) -> T {
        self.build_one()
    }

    /// Build `count` models in-memory.
    pub fn make_many(self) -> Vec<T> {
        let count = self.count;
        (0..count).map(|_| self.build_one()).collect()
    }
}

impl<T: Factory> Default for FactoryBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::{Factory, Faker};

    #[derive(Debug, PartialEq, Clone)]
    struct Post {
        id: u64,
        title: String,
        body: String,
        published: bool,
    }

    impl Factory for Post {
        fn definition() -> Self {
            Post {
                id: 0,
                title: Faker::sentence(3),
                body: Faker::sentence(8),
                published: false,
            }
        }
    }

    #[test]
    fn make_returns_one_model() {
        let post = Post::factory().make();
        assert_eq!(post.id, 0);
        assert!(!post.title.is_empty());
    }

    #[test]
    fn make_many_returns_correct_count() {
        let posts = Post::factory().count(5).make_many();
        assert_eq!(posts.len(), 5);
    }

    #[test]
    fn with_override_applies_to_each() {
        let posts = Post::factory()
            .count(3)
            .with(|p| p.published = true)
            .make_many();
        assert!(posts.iter().all(|p| p.published));
    }

    #[test]
    fn make_with_override_changes_field() {
        let post = Post::factory()
            .with(|p| p.title = "Custom Title".to_string())
            .make();
        assert_eq!(post.title, "Custom Title");
    }

    #[test]
    fn multiple_overrides_are_applied_in_order() {
        let post = Post::factory()
            .with(|p| p.id = 99)
            .with(|p| p.published = true)
            .make();
        assert_eq!(post.id, 99);
        assert!(post.published);
    }

    #[test]
    fn default_builder_count_is_one() {
        use super::FactoryBuilder;
        let builder = FactoryBuilder::<Post>::new();
        let many = builder.make_many();
        assert_eq!(many.len(), 1);
    }

    // ── Faker ─────────────────────────────────────────────────────────────────

    #[test]
    fn faker_name_not_empty() {
        assert!(!Faker::name().is_empty());
    }

    #[test]
    fn faker_email_contains_at() {
        assert!(Faker::email().contains('@'));
    }

    #[test]
    fn faker_uuid_is_36_chars() {
        assert_eq!(Faker::uuid().len(), 36);
    }

    #[test]
    fn faker_integer_in_range() {
        for _ in 0..20 {
            let n = Faker::integer(10, 20);
            assert!((10..=20).contains(&n));
        }
    }

    #[test]
    fn faker_sentence_ends_with_period() {
        let s = Faker::sentence(4);
        assert!(s.ends_with('.'));
    }

    #[test]
    fn faker_phone_starts_with_plus1() {
        assert!(Faker::phone().starts_with("+1-"));
    }

    #[test]
    fn faker_password_starts_with_pass() {
        assert!(Faker::password().starts_with("pass-"));
    }
}

// ── Helper: extract SqlValue from a serde_json::Value ──────────────────────

#[cfg(feature = "postgres")]
fn json_to_sqlvalue(val: &serde_json::Value) -> rok_orm_core::SqlValue {
    match val {
        serde_json::Value::Number(n) => n
            .as_i64()
            .map(rok_orm_core::SqlValue::Integer)
            .or_else(|| n.as_f64().map(rok_orm_core::SqlValue::Float))
            .unwrap_or(rok_orm_core::SqlValue::Null),
        serde_json::Value::String(s) => rok_orm_core::SqlValue::Text(s.clone()),
        serde_json::Value::Bool(b) => rok_orm_core::SqlValue::Bool(*b),
        _ => rok_orm_core::SqlValue::Null,
    }
}

/// Serialize a model to column-value pairs via serde_json, then execute
/// `INSERT … RETURNING *` and return the persisted row.
#[cfg(feature = "postgres")]
async fn persist_model<T>(model: &T, pool: &sqlx::PgPool) -> Result<T, sqlx::Error>
where
    T: serde::Serialize
        + rok_orm_core::Model
        + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>
        + Send
        + Unpin,
{
    let json = serde_json::to_value(model).map_err(|e| sqlx::Error::Protocol(e.to_string()))?;

    let cols = T::columns();
    let mut data: Vec<(&str, rok_orm_core::SqlValue)> = Vec::with_capacity(cols.len());
    for col in cols {
        let val = json_to_sqlvalue(json.get(col).unwrap_or(&serde_json::Value::Null));
        data.push((col, val));
    }

    let (sql, params) = rok_orm_core::QueryBuilder::<T>::insert_sql(T::table_name(), &data);
    let sql = format!("{sql} RETURNING *");

    let mut query = sqlx::query_as::<_, T>(&sql);
    for param in params {
        query = match param {
            rok_orm_core::SqlValue::Text(s) => query.bind(s),
            rok_orm_core::SqlValue::Integer(n) => query.bind(n),
            rok_orm_core::SqlValue::Float(f) => query.bind(f),
            rok_orm_core::SqlValue::Bool(b) => query.bind(b),
            rok_orm_core::SqlValue::Null => query.bind(Option::<String>::None),
            _ => query.bind(Option::<String>::None),
        };
    }
    query.fetch_one(pool).await
}

// ── Async create (postgres feature) ──────────────────────────────────────────

#[cfg(feature = "postgres")]
impl<T> FactoryBuilder<T>
where
    T: Factory
        + serde::Serialize
        + rok_orm_core::Model
        + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>
        + Send
        + Sync
        + Unpin
        + 'static,
{
    /// Persist one model to the database using the task-local pool.
    ///
    /// Requires a pool in scope via [`rok_orm::pool::with_pool`] or
    /// [`rok_orm::OrmLayer`].
    pub async fn create(self) -> Result<T, sqlx::Error> {
        let pool = rok_orm::pool::try_current_pool().ok_or_else(|| {
            sqlx::Error::Configuration(
                "no database pool in scope — use pool::with_pool() or OrmLayer"
                    .to_string()
                    .into(),
            )
        })?;
        self.create_with_pool(&pool).await
    }

    /// Persist one model to the database using the given pool.
    pub async fn create_with_pool(&self, pool: &sqlx::PgPool) -> Result<T, sqlx::Error> {
        let model = self.build_one();
        persist_model::<T>(&model, pool).await
    }

    /// Persist `count` models to the database.
    pub async fn create_many(self) -> Result<Vec<T>, sqlx::Error> {
        let pool = rok_orm::pool::try_current_pool().ok_or_else(|| {
            sqlx::Error::Configuration(
                "no database pool in scope — use pool::with_pool() or OrmLayer"
                    .to_string()
                    .into(),
            )
        })?;
        let mut results = Vec::with_capacity(self.count);
        for _ in 0..self.count {
            let model = self.build_one();
            results.push(persist_model::<T>(&model, &pool).await?);
        }
        Ok(results)
    }
}
