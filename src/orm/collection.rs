//! `ModelCollection<T>` — a `Vec<T>` wrapper with chainable in-memory helpers.
//!
//! Returned by `.collect()` on any `Vec<T>` (via the `IntoCollection` blanket impl).
//! Use it to filter, transform, and aggregate without issuing additional queries.
//!
//! # Example
//!
//! ```rust,no_run
//! # use rok_fluent::orm::collection::IntoCollection;
//! # struct User { pub role: String, pub email: String }
//! # let users: Vec<User> = vec![];
//! let users = users.into_collection();
//! let emails = users.pluck(|u| u.email.clone());
//! ```

use std::collections::HashMap;

/// A wrapper around `Vec<T>` providing chainable in-memory query helpers.
#[derive(Debug, Clone)]
pub struct ModelCollection<T>(Vec<T>);

impl<T> ModelCollection<T> {
    /// Wrap a `Vec<T>`.
    pub fn new(items: Vec<T>) -> Self {
        Self(items)
    }

    /// Return the inner `Vec<T>`.
    pub fn into_vec(self) -> Vec<T> {
        self.0
    }

    /// Borrow the inner slice.
    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// `true` if there are no items.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Alias for [`len`](Self::len).
    pub fn count(&self) -> usize {
        self.0.len()
    }

    // ── Filtering ──────────────────────────────────────────────────────────────

    /// Keep items for which `predicate` returns `true`.
    pub fn filter<F>(self, predicate: F) -> Self
    where
        F: FnMut(&T) -> bool,
    {
        Self(self.0.into_iter().filter(predicate).collect())
    }

    /// Keep items for which `predicate` returns `false` (inverse of `filter`).
    pub fn reject<F>(self, mut predicate: F) -> Self
    where
        F: FnMut(&T) -> bool,
    {
        Self(self.0.into_iter().filter(|item| !predicate(item)).collect())
    }

    /// Find the first item matching the predicate, or `None`.
    pub fn find<F>(&self, predicate: F) -> Option<&T>
    where
        F: FnMut(&&T) -> bool,
    {
        self.0.iter().find(predicate)
    }

    /// Check whether any item matches the predicate.
    pub fn contains<F>(&self, predicate: F) -> bool
    where
        F: FnMut(&T) -> bool,
    {
        self.0.iter().any(predicate)
    }

    // ── Retrieval ──────────────────────────────────────────────────────────────

    /// First item, or `None`.
    pub fn first(&self) -> Option<&T> {
        self.0.first()
    }

    /// Last item, or `None`.
    pub fn last(&self) -> Option<&T> {
        self.0.last()
    }

    // ── Transformation ─────────────────────────────────────────────────────────

    /// Map each item to a value, returning a plain `Vec<U>`.
    pub fn map<U, F>(self, f: F) -> Vec<U>
    where
        F: FnMut(T) -> U,
    {
        self.0.into_iter().map(f).collect()
    }

    /// Flat-map each item to an iterator.
    pub fn flat_map<U, I, F>(self, f: F) -> Vec<U>
    where
        F: FnMut(T) -> I,
        I: IntoIterator<Item = U>,
    {
        self.0.into_iter().flat_map(f).collect()
    }

    /// Extract a single field from every item, returning a `Vec<U>`.
    pub fn pluck<U, F>(&self, f: F) -> Vec<U>
    where
        F: FnMut(&T) -> U,
    {
        self.0.iter().map(f).collect()
    }

    /// Group items by a key extractor, returning a `HashMap<K, Vec<T>>`.
    pub fn group_by<K, F>(self, mut key_fn: F) -> HashMap<K, Vec<T>>
    where
        K: std::hash::Hash + Eq,
        F: FnMut(&T) -> K,
    {
        let mut map: HashMap<K, Vec<T>> = HashMap::new();
        for item in self.0 {
            map.entry(key_fn(&item)).or_default().push(item);
        }
        map
    }

    /// Index items by a unique key extractor, returning `HashMap<K, T>`.
    ///
    /// Later items overwrite earlier items with the same key.
    pub fn key_by<K, F>(self, mut key_fn: F) -> HashMap<K, T>
    where
        K: std::hash::Hash + Eq,
        F: FnMut(&T) -> K,
    {
        let mut map: HashMap<K, T> = HashMap::new();
        for item in self.0 {
            let k = key_fn(&item);
            map.insert(k, item);
        }
        map
    }

    /// Split into chunks of `size` items.
    pub fn chunk(self, size: usize) -> Vec<Vec<T>>
    where
        T: Clone,
    {
        self.0
            .into_iter()
            .collect::<Vec<_>>()
            .chunks(size)
            .map(|c| c.to_vec())
            .collect()
    }

    /// Deduplicate items by a key extractor, keeping the first occurrence.
    pub fn unique_by<K, F>(self, mut key_fn: F) -> Self
    where
        K: std::hash::Hash + Eq,
        F: FnMut(&T) -> K,
    {
        let mut seen: std::collections::HashSet<K> = std::collections::HashSet::new();
        let filtered: Vec<T> = self
            .0
            .into_iter()
            .filter(|item| seen.insert(key_fn(item)))
            .collect();
        Self(filtered)
    }

    // ── Sorting ────────────────────────────────────────────────────────────────

    /// Sort in place using a comparator and return self.
    pub fn sort_by<F>(mut self, compare: F) -> Self
    where
        F: FnMut(&T, &T) -> std::cmp::Ordering,
    {
        self.0.sort_by(compare);
        self
    }

    // ── Numeric aggregates (require closure returning f64) ─────────────────────

    /// Sum a numeric field over all items.
    pub fn sum<F>(&self, mut f: F) -> f64
    where
        F: FnMut(&T) -> f64,
    {
        self.0.iter().map(&mut f).sum()
    }

    /// Average a numeric field over all items. Returns `None` when empty.
    pub fn avg<F>(&self, f: F) -> Option<f64>
    where
        F: FnMut(&T) -> f64,
    {
        if self.0.is_empty() {
            return None;
        }
        Some(self.sum(f) / self.0.len() as f64)
    }

    /// Item with the minimum value of a key.
    pub fn min_by<U, F>(&self, mut f: F) -> Option<&T>
    where
        U: Ord,
        F: FnMut(&T) -> U,
    {
        self.0.iter().min_by_key(|item| f(item))
    }

    /// Item with the maximum value of a key.
    pub fn max_by<U, F>(&self, mut f: F) -> Option<&T>
    where
        U: Ord,
        F: FnMut(&T) -> U,
    {
        self.0.iter().max_by_key(|item| f(item))
    }

    // ── where_field shorthand ──────────────────────────────────────────────────

    /// Keep items where `field_fn(item) == value`.
    pub fn where_field<U, F>(self, field_fn: F, value: U) -> Self
    where
        U: PartialEq,
        F: Fn(&T) -> U,
    {
        Self(
            self.0
                .into_iter()
                .filter(|item| field_fn(item) == value)
                .collect(),
        )
    }

    /// Keep items where `field_fn(item)` is contained in `values`.
    pub fn where_in_field<U, F>(self, field_fn: F, values: &[U]) -> Self
    where
        U: PartialEq,
        F: Fn(&T) -> U,
    {
        Self(
            self.0
                .into_iter()
                .filter(|item| values.contains(&field_fn(item)))
                .collect(),
        )
    }
}

impl<T> From<Vec<T>> for ModelCollection<T> {
    fn from(v: Vec<T>) -> Self {
        Self(v)
    }
}

impl<T> From<ModelCollection<T>> for Vec<T> {
    fn from(c: ModelCollection<T>) -> Self {
        c.0
    }
}

impl<T> IntoIterator for ModelCollection<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a ModelCollection<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// Convenience extension: convert any `Vec<T>` into a `ModelCollection<T>`.
pub trait IntoCollection<T> {
    /// Wrap this `Vec<T>` in a [`ModelCollection<T>`].
    fn into_collection(self) -> ModelCollection<T>;
}

impl<T> IntoCollection<T> for Vec<T> {
    fn into_collection(self) -> ModelCollection<T> {
        ModelCollection::new(self)
    }
}
