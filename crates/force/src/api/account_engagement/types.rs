//! Shared types for the Account Engagement (Pardot) API v5.
#![allow(clippy::doc_markdown)]

use serde::Deserialize;

/// Paginated collection envelope returned by v5 query endpoints.
///
/// v5 list responses have the shape:
/// ```json
/// { "values": [ … ], "nextPageToken": "…", "nextPageUrl": "…" }
/// ```
///
/// `next_page_token` encapsulates the original filters, `limit`, and `orderBy`.
/// On the follow-up request pass ONLY `fields` + `nextPageToken`. Both
/// `next_page_token` and `next_page_url` are absent on the final page.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResponse<T> {
    /// The page of records.
    #[serde(default = "Vec::new")]
    pub values: Vec<T>,
    /// Cursor for the next page, or `None` on the last page.
    pub next_page_token: Option<String>,
    /// Convenience URL for the next page (token already appended), or `None`.
    pub next_page_url: Option<String>,
}
