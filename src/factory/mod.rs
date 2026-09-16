mod faker;

pub use faker::Faker;

// Only referenced by the `create`/`create_many` DB-execution path below, which
// itself only exists when a database backend's `PgModel`/`SqliteModel` (both
// gated behind `active`) is available — gate the import the same way to avoid
// an unused-import warning under `factory` alone.
#[cfg(all(
    feature = "active",
    any(feature = "factory-postgres", feature = "sqlite")
))]
use crate::core::condition::SqlValue;
#[cfg(all(
    feature = "active",
    any(feature = "factory-postgres", feature = "sqlite")
))]
use crate::core::model::ModelValues;

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

// `create`/`create_many` need to run against either a `PgPool` or a
// `SqlitePool` while defining the method exactly once — two separate inherent
// `impl<T> FactoryBuilder<T>` blocks with overlapping bounds (one requiring
// `PgModel`, one requiring `SqliteModel`) would conflict under
// `--all-features` (E0592, duplicate `create` definitions), since rustc can't
// prove the bounds are mutually exclusive. Routing through this
// backend-agnostic bridge trait, generic over the pool type, avoids that.
// `PgModel`/`SqliteModel` (and therefore `create_returning`) only exist when
// `active` is also enabled — see `src/orm/postgres/mod.rs` / `src/orm/sqlite/mod.rs`.
#[cfg(all(
    feature = "active",
    any(feature = "factory-postgres", feature = "sqlite")
))]
#[doc(hidden)]
pub trait FactoryExecutor<Pool>: Sized {
    fn create_returning(
        pool: &Pool,
        data: &[(&str, SqlValue)],
    ) -> impl std::future::Future<Output = Result<Self, sqlx::Error>> + Send;
}

#[cfg(all(feature = "active", feature = "factory-postgres"))]
impl<T> FactoryExecutor<sqlx::PgPool> for T
where
    T: crate::orm::postgres::model::PgModel,
{
    async fn create_returning(
        pool: &sqlx::PgPool,
        data: &[(&str, SqlValue)],
    ) -> Result<Self, sqlx::Error> {
        <T as crate::orm::postgres::model::PgModel>::create_returning(pool, data).await
    }
}

#[cfg(all(feature = "active", feature = "sqlite"))]
impl<T> FactoryExecutor<sqlx::SqlitePool> for T
where
    T: crate::orm::sqlite::model::SqliteModel,
{
    async fn create_returning(
        pool: &sqlx::SqlitePool,
        data: &[(&str, SqlValue)],
    ) -> Result<Self, sqlx::Error> {
        <T as crate::orm::sqlite::model::SqliteModel>::create_returning(pool, data).await
    }
}

// Primary-key columns are dropped from the insert — the database is expected
// to assign them (serial / autoincrement), matching how every hand-written
// `Model::create` call site in this crate omits the primary key explicitly.
#[cfg(all(
    feature = "active",
    any(feature = "factory-postgres", feature = "sqlite")
))]
fn insertable_values<T: ModelValues>(instance: &T) -> Vec<(&'static str, SqlValue)> {
    let pks = T::primary_keys();
    instance
        .to_values()
        .into_iter()
        .filter(|(col, _)| !pks.contains(col))
        .collect()
}

#[cfg(all(
    feature = "active",
    any(feature = "factory-postgres", feature = "sqlite")
))]
impl<T: Factory + ModelValues> FactoryBuilder<T> {
    /// Build one instance and insert it, returning the row as stored by the
    /// database (primary key included).
    pub async fn create<Pool>(self, pool: &Pool) -> Result<T, sqlx::Error>
    where
        T: FactoryExecutor<Pool>,
    {
        let instance = self.make();
        let data = insertable_values(&instance);
        T::create_returning(pool, &data).await
    }

    /// Build `count` instances and insert each one, returning the rows as
    /// stored by the database.
    pub async fn create_many<Pool>(self, pool: &Pool) -> Result<Vec<T>, sqlx::Error>
    where
        T: FactoryExecutor<Pool>,
    {
        let mut created = Vec::with_capacity(self.count);
        for instance in self.make_many() {
            let data = insertable_values(&instance);
            created.push(T::create_returning(pool, &data).await?);
        }
        Ok(created)
    }
}
