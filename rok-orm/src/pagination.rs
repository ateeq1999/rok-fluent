//! Pagination types — `Page<T>`, `SimplePage<T>`, `CursorPage<T>`.
//!
//! Used by [`ModelQuery::paginate`], [`ModelQuery::simple_paginate`], and
//! [`ModelQuery::cursor_paginate`].

use base64::Engine;
use serde::Serialize;

// ── D.1 Offset Pagination ─────────────────────────────────────────────────────

/// A paginated result set with full metadata.
///
/// Serializes as `{ "data": [...], "meta": {...}, "links": {...} }`.
#[derive(Debug, Clone, Serialize)]
pub struct Page<T: Serialize> {
    pub data: Vec<T>,
    pub meta: PageMeta,
    pub links: PageLinks,
}

#[derive(Debug, Clone, Serialize)]
pub struct PageMeta {
    pub total: i64,
    pub per_page: u32,
    pub current_page: u32,
    pub last_page: u32,
    pub from: u64,
    pub to: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PageLinks {
    pub first: String,
    pub last: String,
    pub prev: Option<String>,
    pub next: Option<String>,
}

impl<T: Serialize> Page<T> {
    pub fn new(data: Vec<T>, total: i64, per_page: u32, current_page: u32) -> Self {
        let last_page = ((total as f64) / (per_page as f64)).ceil() as u32;
        let last_page = last_page.max(1);
        let from = if data.is_empty() {
            0
        } else {
            ((current_page - 1) * per_page + 1) as u64
        };
        let to = from + data.len().saturating_sub(1) as u64;

        let page_url = |p: u32| format!("?page={p}&per_page={per_page}");

        Self {
            meta: PageMeta {
                total,
                per_page,
                current_page,
                last_page,
                from,
                to,
            },
            links: PageLinks {
                first: page_url(1),
                last: page_url(last_page),
                prev: if current_page > 1 {
                    Some(page_url(current_page - 1))
                } else {
                    None
                },
                next: if current_page < last_page {
                    Some(page_url(current_page + 1))
                } else {
                    None
                },
            },
            data,
        }
    }

    pub fn has_more(&self) -> bool {
        self.meta.current_page < self.meta.last_page
    }
}

// ── D.2 Simple Pagination (no total count) ────────────────────────────────────

/// A lightweight paginated result set — skips the `COUNT(*)` query.
///
/// `has_more` is determined by fetching `per_page + 1` rows.
#[derive(Debug, Clone, Serialize)]
pub struct SimplePage<T: Serialize> {
    pub data: Vec<T>,
    pub per_page: u32,
    pub has_more: bool,
    pub links: SimplePageLinks,
}

#[derive(Debug, Clone, Serialize)]
pub struct SimplePageLinks {
    pub prev: Option<String>,
    pub next: Option<String>,
}

impl<T: Serialize> SimplePage<T> {
    pub fn new(mut data: Vec<T>, per_page: u32, current_page: u32) -> Self {
        let has_more = data.len() > per_page as usize;
        if has_more {
            data.truncate(per_page as usize);
        }
        let page_url = |p: u32| format!("?page={p}&per_page={per_page}");
        Self {
            has_more,
            links: SimplePageLinks {
                prev: if current_page > 1 {
                    Some(page_url(current_page - 1))
                } else {
                    None
                },
                next: if has_more {
                    Some(page_url(current_page + 1))
                } else {
                    None
                },
            },
            data,
            per_page,
        }
    }
}

// ── D.3 Cursor Pagination ─────────────────────────────────────────────────────

/// A keyset-based cursor paginated result.
///
/// `next_cursor` / `prev_cursor` are opaque base64-encoded tokens.
#[derive(Debug, Clone, Serialize)]
pub struct CursorPage<T: Serialize> {
    pub data: Vec<T>,
    pub per_page: u32,
    pub next_cursor: Option<String>,
    pub prev_cursor: Option<String>,
    pub links: CursorPageLinks,
}

#[derive(Debug, Clone, Serialize)]
pub struct CursorPageLinks {
    pub next: Option<String>,
    pub prev: Option<String>,
}

impl<T: Serialize> CursorPage<T> {
    pub fn new(
        mut data: Vec<T>,
        per_page: u32,
        next_cursor: Option<String>,
        prev_cursor: Option<String>,
    ) -> Self {
        let has_more = data.len() > per_page as usize;
        if has_more {
            data.truncate(per_page as usize);
        }
        let actual_next = if has_more { next_cursor.clone() } else { None };
        let next_link = actual_next.as_deref().map(|c| format!("?cursor={c}"));
        let prev_link = prev_cursor.as_deref().map(|c| format!("?cursor={c}"));
        Self {
            data,
            per_page,
            next_cursor: actual_next,
            prev_cursor: prev_cursor.clone(),
            links: CursorPageLinks {
                next: next_link,
                prev: prev_link,
            },
        }
    }
}

// ── Cursor encoding helpers ───────────────────────────────────────────────────

/// Encode an `i64` cursor value to a base64 opaque token.
pub fn encode_cursor(id: i64) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(id.to_le_bytes())
}

/// Decode an opaque cursor token back to an `i64`.  Returns `None` on error.
pub fn decode_cursor(token: &str) -> Option<i64> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(token)
        .ok()?;
    if bytes.len() < 8 {
        return None;
    }
    Some(i64::from_le_bytes(bytes[..8].try_into().ok()?))
}
