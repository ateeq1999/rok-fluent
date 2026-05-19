//! API resources — transform ORM models into JSON-friendly response shapes.
//!
//! # Overview
//!
//! `Resource<T>` wraps a single model for JSON serialization.
//! `ResourceCollection<T>` wraps a list of models and carries metadata.
//!
//! Both implement [`serde::Serialize`] so they can be returned from Axum handlers directly.
//!
//! # Example
//!
//! ```rust,ignore
//! use rok_orm::resource::{Resource, ResourceCollection};
//!
//! // Single resource
//! let res = Resource::new(user);
//! axum::Json(res) // => {"id": 1, "name": "Alice"}
//!
//! // Collection with metadata
//! let res = ResourceCollection::new(users);
//! axum::Json(res) // => {"data": [...], "total": 3}
//!
//! // From a paginated page
//! let page = User::all_query().paginate(20, 1).await?;
//! let res = ResourceCollection::from_page(&page);
//! ```

use serde::Serialize;

use crate::pagination::Page;

// ── Proc-macro helpers (used by #[derive(Resource)] generated code) ───────────

/// Serialize any `Serialize` value to a `serde_json::Value`.  Used by generated code.
#[doc(hidden)]
pub fn field_to_json<T: Serialize>(val: &T) -> serde_json::Value {
    serde_json::to_value(val).unwrap_or(serde_json::Value::Null)
}

/// Build a `serde_json::Value::Object` from a list of (key, value) pairs.  Used by generated code.
#[doc(hidden)]
pub fn build_resource(fields: Vec<(&str, serde_json::Value)>) -> serde_json::Value {
    let map: serde_json::Map<String, serde_json::Value> = fields
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    serde_json::Value::Object(map)
}

/// Type alias for the return type of `to_resource()` — re-exported for generated code.
#[doc(hidden)]
pub type ResourceValue = serde_json::Value;

// ── Resource<T> ───────────────────────────────────────────────────────────────

/// A single model wrapped for API serialization.
///
/// Serializes as the inner value directly (no wrapping object).
#[derive(Debug, Clone)]
pub struct Resource<T: Serialize>(pub T);

impl<T: Serialize> Resource<T> {
    pub fn new(data: T) -> Self {
        Self(data)
    }

    /// Serialize to a `serde_json::Value`.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(&self.0).unwrap_or(serde_json::Value::Null)
    }
}

impl<T: Serialize> Serialize for Resource<T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

impl<T: Serialize> From<T> for Resource<T> {
    fn from(val: T) -> Self {
        Self(val)
    }
}

// ── ResourceCollection<T> ────────────────────────────────────────────────────

/// A collection of models with total count metadata.
///
/// Serializes as `{ "data": [...], "total": N }`.
#[derive(Debug, Clone, Serialize)]
pub struct ResourceCollection<T: Serialize> {
    pub data: Vec<T>,
    pub total: usize,
}

impl<T: Serialize> ResourceCollection<T> {
    pub fn new(data: Vec<T>) -> Self {
        let total = data.len();
        Self { data, total }
    }

    /// Build from a [`Page`] result, preserving the true total count.
    pub fn from_page(page: &Page<T>) -> Self
    where
        T: Clone,
    {
        Self {
            data: page.data.clone(),
            total: page.meta.total as usize,
        }
    }

    /// Map each item through a transform function.
    pub fn map<U: Serialize>(self, f: impl Fn(T) -> U) -> ResourceCollection<U> {
        let total = self.total;
        ResourceCollection {
            data: self.data.into_iter().map(f).collect(),
            total,
        }
    }

    /// Convert into a plain `serde_json::Value`.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

impl<T: Serialize> From<Vec<T>> for ResourceCollection<T> {
    fn from(data: Vec<T>) -> Self {
        Self::new(data)
    }
}

// ── Paginated resource collection ─────────────────────────────────────────────

/// A paginated API response — wraps `Page<T>` with resource-friendly shape.
///
/// Serializes as `{ "data": [...], "meta": { "total": N, "per_page": N, ... } }`.
#[derive(Debug, Clone, Serialize)]
pub struct PageResource<T: Serialize> {
    pub data: Vec<T>,
    pub meta: crate::pagination::PageMeta,
    pub links: crate::pagination::PageLinks,
}

impl<T: Serialize + Clone> PageResource<T> {
    pub fn from_page(page: &Page<T>) -> Self {
        Self {
            data: page.data.clone(),
            meta: page.meta.clone(),
            links: page.links.clone(),
        }
    }
}

impl<T: Serialize + Clone> From<Page<T>> for PageResource<T> {
    fn from(page: Page<T>) -> Self {
        Self {
            data: page.data,
            meta: page.meta,
            links: page.links,
        }
    }
}
