//! Shared types used across the Marketing Cloud API handlers.

use serde::{Deserialize, Serialize};

/// A bag of personalization / attribute name-value pairs.
///
/// Marketing Cloud accepts arbitrary JSON values here, so this maps directly to
/// a JSON object.
pub type Attributes = serde_json::Map<String, serde_json::Value>;

/// A generic paged collection response used by several list endpoints.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Paged<T> {
    /// Total number of items matching the query, if reported.
    #[serde(default)]
    pub count: Option<u64>,

    /// The 1-based page index of this response, if reported.
    #[serde(default)]
    pub page: Option<u64>,

    /// The page size used for this response, if reported.
    #[serde(rename = "pageSize", default)]
    pub page_size: Option<u64>,

    /// The items on this page.
    #[serde(default = "Vec::new")]
    pub items: Vec<T>,
}
