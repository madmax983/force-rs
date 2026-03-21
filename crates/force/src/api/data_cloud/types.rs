//! Data Cloud API request and response types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// SQL query request body for the Data Cloud Query API.
///
/// # Examples
///
/// ```ignore
/// use force::api::data_cloud::SqlQueryRequest;
///
/// let request = SqlQueryRequest::new("SELECT * FROM UnifiedProfile__dlm LIMIT 10");
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct SqlQueryRequest {
    /// The SQL query string.
    pub sql: String,
}

impl SqlQueryRequest {
    /// Creates a new SQL query request.
    #[must_use]
    pub fn new(sql: impl Into<String>) -> Self {
        Self { sql: sql.into() }
    }
}

/// A single record from a Data Cloud SQL query result.
///
/// Records are represented as key-value maps where values are dynamic JSON.
pub type DataCloudRecord = HashMap<String, serde_json::Value>;

/// Metadata about a column in the query result.
#[derive(Debug, Clone, Deserialize)]
pub struct ColumnMetadata {
    /// The column name.
    #[serde(alias = "columnName")]
    pub name: String,

    /// The column data type (e.g., `"VARCHAR"`, `"NUMBER"`, `"BOOLEAN"`).
    #[serde(alias = "type", alias = "columnType", default)]
    pub data_type: String,

    /// Additional metadata fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Must;

    #[test]
    fn test_sql_query_request_new() {
        let req = SqlQueryRequest::new("SELECT Id FROM Profile__dlm");
        assert_eq!(req.sql, "SELECT Id FROM Profile__dlm");
    }

    #[test]
    fn test_sql_query_request_serialization() {
        let req = SqlQueryRequest::new("SELECT * FROM T LIMIT 1");
        let json = serde_json::to_string(&req).must();
        assert!(json.contains(r#""sql":"SELECT * FROM T LIMIT 1"#));
    }

    #[test]
    fn test_column_metadata_deserialization() {
        let json = r#"{"columnName": "Id", "type": "VARCHAR"}"#;
        let col: ColumnMetadata = serde_json::from_str(json).must();
        assert_eq!(col.name, "Id");
        assert_eq!(col.data_type, "VARCHAR");
    }

    #[test]
    fn test_column_metadata_with_extra() {
        let json = r#"{"columnName": "Amount", "type": "NUMBER", "precision": 18}"#;
        let col: ColumnMetadata = serde_json::from_str(json).must();
        assert_eq!(col.name, "Amount");
        assert!(col.extra.contains_key("precision"));
    }
}
