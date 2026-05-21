//! Field casts — convert model field values to/from their database representation.
//!
//! | Type | Database type | Rust type |
//! |---|---|---|
//! | `CastJson<T>` | `TEXT` (JSON) | any `Serialize + DeserializeOwned` |
//! | [`CastBool`] | `BOOLEAN` | `bool` |
//! | [`CastDatetime`] | `TEXT` / `TIMESTAMPTZ` | `chrono::DateTime<Utc>` |
//! | [`CastDate`] | `TEXT` / `DATE` | `chrono::NaiveDate` |
//! | [`CastUuid`] | `TEXT` / `UUID` | `uuid::Uuid` |
//! | [`CastCommaList`] | `TEXT` | `Vec<String>` |
//! | `CastArray<T>` | `TEXT` (JSON) | `Vec<T>` |

use crate::core::condition::SqlValue;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use uuid::Uuid;

// ── Cast trait ────────────────────────────────────────────────────────────────

/// Defines how a Rust value is serialized to and deserialized from its database form.
pub trait Cast: Sized {
    /// The raw database representation type.
    type Database;

    /// Convert from the raw database value into the rich Rust type.
    fn from_db(val: Self::Database) -> Self;

    /// Convert from the rich Rust type into the raw database value.
    fn to_db(self) -> Self::Database;
}

// ── CastJson<T> ───────────────────────────────────────────────────────────────

/// Stores any `Serialize + DeserializeOwned` value as a JSON `TEXT` column.
#[derive(Debug, Clone, PartialEq)]
pub struct CastJson<T>(pub T);

impl<T: Serialize + DeserializeOwned> Cast for CastJson<T> {
    type Database = String;

    fn from_db(val: String) -> Self {
        let inner = serde_json::from_str(&val)
            .unwrap_or_else(|_| panic!("CastJson::from_db — invalid JSON: {val}"));
        CastJson(inner)
    }

    fn to_db(self) -> String {
        serde_json::to_string(&self.0).expect("CastJson::to_db — serialization failed")
    }
}

impl<T: Serialize + DeserializeOwned> From<CastJson<T>> for SqlValue {
    fn from(val: CastJson<T>) -> Self {
        SqlValue::Text(val.to_db())
    }
}

#[cfg(feature = "postgres")]
impl<T: Serialize + DeserializeOwned + Send + Unpin> sqlx::Type<sqlx::Postgres> for CastJson<T> {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

#[cfg(feature = "postgres")]
impl<'r, T: Serialize + DeserializeOwned + Send + Unpin> sqlx::Decode<'r, sqlx::Postgres>
    for CastJson<T>
{
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let raw = <String as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        Ok(CastJson::from_db(raw))
    }
}

#[cfg(feature = "postgres")]
impl<'q, T: Serialize + DeserializeOwned + Send + Unpin> sqlx::Encode<'q, sqlx::Postgres>
    for CastJson<T>
{
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        let json_str = serde_json::to_string(&self.0)?;
        <String as sqlx::Encode<sqlx::Postgres>>::encode_by_ref(&json_str, buf)
    }
}

// ── CastBool ──────────────────────────────────────────────────────────────────

/// Stores a `bool` — thin wrapper useful for explicit cast tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CastBool(pub bool);

impl Cast for CastBool {
    type Database = bool;
    fn from_db(val: bool) -> Self {
        CastBool(val)
    }
    fn to_db(self) -> bool {
        self.0
    }
}

impl From<CastBool> for SqlValue {
    fn from(val: CastBool) -> Self {
        SqlValue::Bool(val.0)
    }
}

impl From<bool> for CastBool {
    fn from(b: bool) -> Self {
        CastBool(b)
    }
}

// ── CastDatetime ─────────────────────────────────────────────────────────────

/// Parses an RFC 3339 datetime string stored in a `TEXT` column into
/// `chrono::DateTime<chrono::Utc>`.
#[derive(Debug, Clone, PartialEq)]
pub struct CastDatetime(pub chrono::DateTime<chrono::Utc>);

impl Cast for CastDatetime {
    type Database = String;

    fn from_db(val: String) -> Self {
        let dt = val
            .parse::<chrono::DateTime<chrono::Utc>>()
            .unwrap_or_else(|_| panic!("CastDatetime::from_db — invalid datetime string: {val}"));
        CastDatetime(dt)
    }

    fn to_db(self) -> String {
        self.0.to_rfc3339()
    }
}

impl From<CastDatetime> for SqlValue {
    fn from(val: CastDatetime) -> Self {
        SqlValue::Text(val.to_db())
    }
}

impl From<chrono::DateTime<chrono::Utc>> for CastDatetime {
    fn from(dt: chrono::DateTime<chrono::Utc>) -> Self {
        CastDatetime(dt)
    }
}

// ── CastDate ──────────────────────────────────────────────────────────────────

/// Parses a `YYYY-MM-DD` date string stored in a `TEXT` or `DATE` column into
/// `chrono::NaiveDate`.
#[derive(Debug, Clone, PartialEq)]
pub struct CastDate(pub chrono::NaiveDate);

impl Cast for CastDate {
    type Database = String;

    fn from_db(val: String) -> Self {
        let d = chrono::NaiveDate::parse_from_str(&val, "%Y-%m-%d")
            .unwrap_or_else(|_| panic!("CastDate::from_db — invalid date string: {val}"));
        CastDate(d)
    }

    fn to_db(self) -> String {
        self.0.format("%Y-%m-%d").to_string()
    }
}

impl From<CastDate> for SqlValue {
    fn from(val: CastDate) -> Self {
        SqlValue::Text(val.to_db())
    }
}

impl From<chrono::NaiveDate> for CastDate {
    fn from(d: chrono::NaiveDate) -> Self {
        CastDate(d)
    }
}

// ── CastUuid ──────────────────────────────────────────────────────────────────

/// Stores a `uuid::Uuid` as a `TEXT` or `UUID` column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastUuid(pub Uuid);

impl Cast for CastUuid {
    type Database = String;

    fn from_db(val: String) -> Self {
        let u = val
            .parse::<Uuid>()
            .unwrap_or_else(|_| panic!("CastUuid::from_db — invalid UUID: {val}"));
        CastUuid(u)
    }

    fn to_db(self) -> String {
        self.0.to_string()
    }
}

impl From<CastUuid> for SqlValue {
    fn from(val: CastUuid) -> Self {
        SqlValue::Text(val.to_db())
    }
}

impl From<Uuid> for CastUuid {
    fn from(u: Uuid) -> Self {
        CastUuid(u)
    }
}

// ── CastCommaList ─────────────────────────────────────────────────────────────

/// Stores a `Vec<String>` as a comma-separated `TEXT` column.
#[derive(Debug, Clone, PartialEq)]
pub struct CastCommaList(pub Vec<String>);

impl Cast for CastCommaList {
    type Database = String;

    fn from_db(val: String) -> Self {
        if val.is_empty() {
            CastCommaList(Vec::new())
        } else {
            CastCommaList(val.split(',').map(|s| s.trim().to_string()).collect())
        }
    }

    fn to_db(self) -> String {
        self.0.join(",")
    }
}

impl From<CastCommaList> for SqlValue {
    fn from(val: CastCommaList) -> Self {
        SqlValue::Text(val.to_db())
    }
}

impl From<Vec<String>> for CastCommaList {
    fn from(v: Vec<String>) -> Self {
        CastCommaList(v)
    }
}

// ── CastArray<T> ──────────────────────────────────────────────────────────────

/// Bridges a Rust `Vec<T>` to a JSON array stored in a `TEXT` column.
#[derive(Debug, Clone, PartialEq)]
pub struct CastArray<T>(pub Vec<T>);

impl<T: serde::Serialize + serde::de::DeserializeOwned> Cast for CastArray<T> {
    type Database = String;

    fn from_db(val: String) -> Self {
        let inner: Vec<T> = serde_json::from_str(&val)
            .unwrap_or_else(|_| panic!("CastArray::from_db — invalid JSON array: {val}"));
        CastArray(inner)
    }

    fn to_db(self) -> String {
        serde_json::to_string(&self.0).expect("CastArray::to_db — serialization failed")
    }
}

impl<T: serde::Serialize + serde::de::DeserializeOwned> From<CastArray<T>> for SqlValue {
    fn from(val: CastArray<T>) -> Self {
        SqlValue::Text(val.to_db())
    }
}

impl<T: serde::Serialize + serde::de::DeserializeOwned> From<Vec<T>> for CastArray<T> {
    fn from(v: Vec<T>) -> Self {
        CastArray(v)
    }
}

// ── TypedJson<T> ───────────────────────────────────────────────────────────────

/// Wraps a `jsonb` column value and deserializes it directly into a typed struct.
///
/// Unlike [`CastJson<T>`] (which stores JSON as `TEXT`), `TypedJson<T>` targets
/// a native `jsonb` PostgreSQL column and uses `sqlx::types::Json<T>` for native
/// jsonb encoding/decoding.
///
/// # Example
///
/// ```rust,ignore
/// #[derive(Debug, sqlx::FromRow, serde::Deserialize)]
/// pub struct Metadata {
///     pub key: String,
///     pub value: i64,
/// }
///
/// #[derive(Debug, sqlx::FromRow, rok_fluent::Table)]
/// pub struct Product {
///     pub id: i64,
///     pub name: String,
///     pub meta: TypedJson<Metadata>,
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TypedJson<T>(pub T);

impl<T: Serialize + DeserializeOwned> From<TypedJson<T>> for SqlValue {
    fn from(val: TypedJson<T>) -> Self {
        SqlValue::Json(serde_json::to_value(val.0).unwrap_or(Value::Null))
    }
}

#[cfg(feature = "postgres")]
impl<T: Serialize + DeserializeOwned + Send + Unpin> sqlx::Type<sqlx::Postgres> for TypedJson<T> {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <sqlx::types::Json<T> as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

#[cfg(feature = "postgres")]
impl<'r, T: Serialize + DeserializeOwned + Send + Unpin> sqlx::Decode<'r, sqlx::Postgres>
    for TypedJson<T>
{
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let json_val = <sqlx::types::Json<T> as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        Ok(TypedJson(json_val.0))
    }
}

#[cfg(feature = "postgres")]
impl<'q, T: Serialize + DeserializeOwned + Send + Unpin> sqlx::Encode<'q, sqlx::Postgres>
    for TypedJson<T>
{
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <sqlx::types::Json<&T> as sqlx::Encode<sqlx::Postgres>>::encode_by_ref(
            &sqlx::types::Json(&self.0),
            buf,
        )
    }
}
