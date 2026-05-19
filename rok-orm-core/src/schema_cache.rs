//! Runtime column metadata cache.
//!
//! Caches column names and types for a table so introspection has zero overhead
//! after the first call.

use std::collections::HashMap;
use std::sync::Mutex;

static SCHEMA_CACHE: once_cell::sync::Lazy<Mutex<HashMap<String, TableSchema>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone)]
pub struct ColumnMeta {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub is_pk: bool,
}

#[derive(Debug, Clone)]
pub struct TableSchema {
    pub table: String,
    pub columns: Vec<ColumnMeta>,
    pub primary_keys: Vec<String>,
}

pub fn get_schema(table: &str) -> Option<TableSchema> {
    SCHEMA_CACHE.lock().ok()?.get(table).cloned()
}

pub fn set_schema(schema: TableSchema) {
    if let Ok(mut cache) = SCHEMA_CACHE.lock() {
        cache.insert(schema.table.clone(), schema);
    }
}

pub fn invalidate(table: &str) {
    if let Ok(mut cache) = SCHEMA_CACHE.lock() {
        cache.remove(table);
    }
}

pub fn clear() {
    if let Ok(mut cache) = SCHEMA_CACHE.lock() {
        cache.clear();
    }
}
