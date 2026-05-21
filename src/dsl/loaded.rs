//! [`Loaded<T>`] — lazy relationship carrier for DSL model fields.

use serde::{Deserialize, Serialize};

/// Represents whether a relationship field has been loaded.
///
/// Relationship fields on `#[derive(Table)]` structs use this type to
/// distinguish "not yet fetched" from "fetched but empty":
///
/// ```rust,ignore
/// #[derive(Debug, sqlx::FromRow, rok_fluent::Table)]
/// #[table(name = "users")]
/// pub struct User {
///     pub id:   i64,
///     pub name: String,
///     // #[sqlx(skip)] is required so sqlx doesn't try to read this column.
///     #[sqlx(skip)]
///     #[table(has_many)]
///     pub posts: Loaded<Vec<Post>>,
/// }
/// ```
///
/// After fetching users you call `.with(User::POSTS)` on the builder (Phase 26),
/// which fills each user's `posts` field with `Loaded::Some(vec![…])`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Loaded<T> {
    /// The relationship has not been requested and was not fetched.
    #[default]
    NotLoaded,
    /// The relationship was fetched and contains `T` (may be an empty `Vec`).
    Some(T),
}

impl<T> Loaded<T> {
    /// Return `true` if the relationship has been loaded.
    pub fn is_loaded(&self) -> bool {
        matches!(self, Self::Some(_))
    }

    /// Return a reference to the loaded value, or `None` if not yet loaded.
    pub fn as_option(&self) -> Option<&T> {
        match self {
            Self::Some(v) => Some(v),
            Self::NotLoaded => None,
        }
    }

    /// Consume and return the loaded value, or `None` if not yet loaded.
    pub fn into_option(self) -> Option<T> {
        match self {
            Self::Some(v) => Some(v),
            Self::NotLoaded => None,
        }
    }

    /// Return a reference to the loaded value.
    ///
    /// # Panics
    ///
    /// Panics if the relationship has not been loaded yet.
    pub fn unwrap(self) -> T {
        match self {
            Self::Some(v) => v,
            Self::NotLoaded => panic!("called Loaded::unwrap() on a NotLoaded value"),
        }
    }

    /// Map the loaded value if present.
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Loaded<U> {
        match self {
            Self::Some(v) => Loaded::Some(f(v)),
            Self::NotLoaded => Loaded::NotLoaded,
        }
    }
}

// Serialize as the inner value when loaded, null otherwise.
impl<T: Serialize> Serialize for Loaded<T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Some(v) => v.serialize(s),
            Self::NotLoaded => s.serialize_none(),
        }
    }
}

// Deserialize as Option<T>: null / missing → NotLoaded.
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Loaded<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Option::<T>::deserialize(d).map(|opt| match opt {
            Some(v) => Self::Some(v),
            None => Self::NotLoaded,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_not_loaded() {
        let l: Loaded<Vec<i32>> = Loaded::default();
        assert!(!l.is_loaded());
        assert!(l.as_option().is_none());
    }

    #[test]
    fn some_is_loaded() {
        let l = Loaded::Some(vec![1, 2, 3]);
        assert!(l.is_loaded());
        assert_eq!(l.as_option().unwrap().len(), 3);
    }

    #[test]
    fn map_transforms_value() {
        let l = Loaded::Some(vec![1_i32, 2, 3]);
        let l2 = l.map(|v| v.len());
        assert_eq!(l2, Loaded::Some(3));
    }

    #[test]
    fn into_option() {
        let l: Loaded<i32> = Loaded::NotLoaded;
        assert_eq!(l.into_option(), None);
        assert_eq!(Loaded::Some(42_i32).into_option(), Some(42));
    }

    #[test]
    fn serialize_not_loaded_as_null() {
        let l: Loaded<i32> = Loaded::NotLoaded;
        let s = serde_json::to_string(&l).unwrap();
        assert_eq!(s, "null");
    }

    #[test]
    fn serialize_some() {
        let l = Loaded::Some(vec![1_i32, 2]);
        let s = serde_json::to_string(&l).unwrap();
        assert_eq!(s, "[1,2]");
    }
}
