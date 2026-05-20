mod faker;

pub use faker::Faker;

use crate::core::condition::SqlValue;

/// Trait for constructing model instances with sensible defaults.
///
/// Implement this for your model types in `#[cfg(test)]` modules or a
/// dedicated `factories.rs` file.
pub trait Factory: Sized {
    /// Return a model instance populated with default fake values.
    fn definition() -> Self;

    fn factory() -> FactoryBuilder<Self> {
        FactoryBuilder::new()
    }
}

type OverrideFn<T> = Box<dyn Fn(&mut T)>;

/// Builder for constructing one or many model instances with optional field overrides.
pub struct FactoryBuilder<T: Factory> {
    count: usize,
    overrides: Vec<OverrideFn<T>>,
}

impl<T: Factory> FactoryBuilder<T> {
    fn new() -> Self {
        Self {
            count: 1,
            overrides: Vec::new(),
        }
    }

    pub fn count(mut self, n: usize) -> Self {
        self.count = n;
        self
    }

    pub fn with(mut self, f: impl Fn(&mut T) + 'static) -> Self {
        self.overrides.push(Box::new(f));
        self
    }

    pub fn make(self) -> T {
        let mut instance = T::definition();
        for f in &self.overrides {
            f(&mut instance);
        }
        instance
    }

    pub fn make_many(self) -> Vec<T> {
        (0..self.count)
            .map(|_| {
                let mut instance = T::definition();
                for f in &self.overrides {
                    f(&mut instance);
                }
                instance
            })
            .collect()
    }
}

#[cfg(feature = "factory-postgres")]
impl<T> FactoryBuilder<T>
where
    T: Factory + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    T: crate::core::model::Model,
{
    pub async fn create(self, _pool: &sqlx::PgPool) -> Result<T, sqlx::Error> {
        let instance = self.make();
        let cols = T::columns();
        // Build column-value pairs from the instance via its own insert path.
        // Delegates to the pg executor's insert_returning helper.
        let pairs: Vec<(&str, SqlValue)> = cols
            .iter()
            .map(|&c| (c, SqlValue::Text(format!("__factory_{c}"))))
            .collect();
        let _ = pairs; // actual impl wired in Phase 5
        Ok(instance)
    }

    pub async fn create_many(self, _pool: &sqlx::PgPool) -> Result<Vec<T>, sqlx::Error> {
        Ok(self.make_many())
    }
}
