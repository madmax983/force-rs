//! GraphQL request and response types for the Salesforce GraphQL API.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Request body for the Salesforce GraphQL API.
///
/// Construct via [`GraphqlRequest::new`] and optionally chain
/// [`with_variables`](GraphqlRequest::with_variables) /
/// [`with_operation_name`](GraphqlRequest::with_operation_name).
///
/// # Examples
///
/// ```
/// use force::api::graphql::GraphqlRequest;
/// use serde_json::json;
///
/// let req = GraphqlRequest::new("{ uiapi { query { Account { edges { node { Id } } } } } }");
///
/// let req_with_vars = GraphqlRequest::new("query($id: ID!) { ... }")
///     .with_variables(json!({"id": "001xx"}))
///     .with_operation_name("GetAccount");
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct GraphqlRequest {
    /// The GraphQL query or mutation string.
    pub query: String,

    /// Optional variables for parameterized queries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<Value>,

    /// Optional operation name when the query contains multiple operations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_name: Option<String>,
}

impl GraphqlRequest {
    /// Creates a new GraphQL request with the given query string.
    #[must_use]
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            variables: None,
            operation_name: None,
        }
    }

    /// Sets the variables for this request.
    #[must_use]
    pub fn with_variables(mut self, variables: Value) -> Self {
        self.variables = Some(variables);
        self
    }

    /// Sets the operation name for this request.
    #[must_use]
    pub fn with_operation_name(mut self, name: impl Into<String>) -> Self {
        self.operation_name = Some(name.into());
        self
    }
}

/// Full response envelope from the Salesforce GraphQL API.
///
/// Follows the GraphQL spec: a response may contain `data`, `errors`, or both.
/// The type parameter `T` controls how the `data` field is deserialized.
/// Defaults to `serde_json::Value` for untyped access.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GraphqlResponse<T = Value> {
    /// The data returned by the query (absent on total failure).
    pub data: Option<T>,

    /// GraphQL errors, if any. May coexist with `data` (partial success).
    #[serde(default)]
    pub errors: Option<Vec<GraphqlError>>,

    /// Server-defined extensions (request IDs, cost, etc.).
    #[serde(default)]
    pub extensions: Option<HashMap<String, Value>>,
}

impl<T> GraphqlResponse<T> {
    /// Returns `true` if the response contains any errors.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.errors
            .as_ref()
            .is_some_and(|errors| !errors.is_empty())
    }
}

/// A single GraphQL error from the response.
///
/// Follows the GraphQL spec error format with Salesforce-specific extensions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphqlError {
    /// Human-readable error message.
    pub message: String,

    /// Source locations in the query that caused the error.
    #[serde(default)]
    pub locations: Vec<GraphqlErrorLocation>,

    /// Path to the field that caused the error.
    #[serde(default)]
    pub path: Vec<Value>,

    /// Salesforce-specific error extensions (e.g., `errorCode`).
    #[serde(default)]
    pub extensions: HashMap<String, Value>,
}

/// Source location within a GraphQL query string.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphqlErrorLocation {
    /// Line number (1-based).
    pub line: u32,
    /// Column number (1-based).
    pub column: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Must;
    use serde_json::json;

    // ── GraphqlRequest serialization ────────────────────────────────

    #[test]
    fn test_request_minimal_serialization() {
        let req = GraphqlRequest::new("{ uiapi { query { Account { edges { node { Id } } } } } }");
        let json = serde_json::to_value(&req).must();

        assert_eq!(
            json["query"],
            "{ uiapi { query { Account { edges { node { Id } } } } } }"
        );
        // Optional fields should be absent, not null
        assert!(json.get("variables").is_none());
        assert!(json.get("operation_name").is_none());
    }

    #[test]
    fn test_request_with_variables() {
        let req = GraphqlRequest::new("query($id: ID!) { node(id: $id) { id } }")
            .with_variables(json!({"id": "001xx000003DHP0AAA"}));
        let json = serde_json::to_value(&req).must();

        assert_eq!(json["variables"]["id"], "001xx000003DHP0AAA");
    }

    #[test]
    fn test_request_with_operation_name() {
        let req = GraphqlRequest::new("query GetAccount { ... }").with_operation_name("GetAccount");
        let json = serde_json::to_value(&req).must();

        assert_eq!(json["operation_name"], "GetAccount");
    }

    #[test]
    fn test_request_full_builder_chain() {
        let req = GraphqlRequest::new("query Op($x: Int!) { f(x: $x) }")
            .with_variables(json!({"x": 42}))
            .with_operation_name("Op");
        let json = serde_json::to_value(&req).must();

        assert_eq!(json["query"], "query Op($x: Int!) { f(x: $x) }");
        assert_eq!(json["variables"]["x"], 42);
        assert_eq!(json["operation_name"], "Op");
    }

    // ── GraphqlResponse deserialization ─────────────────────────────

    #[test]
    fn test_response_data_only() {
        let json_str = r#"{"data": {"uiapi": {"query": {"Account": {"edges": []}}}}}"#;
        let resp: GraphqlResponse = serde_json::from_str(json_str).must();

        assert!(resp.data.is_some());
        assert!(!resp.has_errors());
    }

    #[test]
    fn test_response_errors_only() {
        let json_str = r#"{
            "data": null,
            "errors": [
                {
                    "message": "Cannot query field 'Foo' on type 'Account'",
                    "locations": [{"line": 1, "column": 45}],
                    "extensions": {"errorCode": "INVALID_FIELD"}
                }
            ]
        }"#;
        let resp: GraphqlResponse = serde_json::from_str(json_str).must();

        assert!(resp.data.is_none());
        assert!(resp.has_errors());
        let errors = resp.errors.must();
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            "Cannot query field 'Foo' on type 'Account'"
        );
        assert_eq!(
            errors[0].locations,
            vec![GraphqlErrorLocation {
                line: 1,
                column: 45
            }]
        );
        assert_eq!(errors[0].extensions["errorCode"], "INVALID_FIELD");
    }

    #[test]
    fn test_response_partial_success() {
        let json_str = r#"{
            "data": {"uiapi": {"query": {"Account": {"edges": []}}}},
            "errors": [{"message": "Insufficient access to field 'Revenue'"}]
        }"#;
        let resp: GraphqlResponse = serde_json::from_str(json_str).must();

        assert!(resp.data.is_some());
        assert!(resp.has_errors());
    }

    #[test]
    fn test_response_neither_data_nor_errors() {
        let json_str = r"{}";
        let resp: GraphqlResponse = serde_json::from_str(json_str).must();

        assert!(resp.data.is_none());
        assert!(!resp.has_errors());
    }

    #[test]
    fn test_response_with_extensions() {
        let json_str = r#"{
            "data": {"value": 1},
            "extensions": {"requestId": "abc-123", "cost": 5}
        }"#;
        let resp: GraphqlResponse = serde_json::from_str(json_str).must();

        assert!(resp.data.is_some());
        let ext = resp.extensions.must();
        assert_eq!(ext["requestId"], "abc-123");
        assert_eq!(ext["cost"], 5);
    }

    #[test]
    fn test_response_typed_deserialization() {
        #[derive(Debug, Deserialize)]
        struct MyData {
            name: String,
            count: u32,
        }

        let json_str = r#"{"data": {"name": "test", "count": 42}}"#;
        let resp: GraphqlResponse<MyData> = serde_json::from_str(json_str).must();

        let data = resp.data.must();
        assert_eq!(data.name, "test");
        assert_eq!(data.count, 42);
    }

    // ── GraphqlError ────────────────────────────────────────────────

    #[test]
    fn test_error_minimal() {
        let json_str = r#"{"message": "Something went wrong"}"#;
        let err: GraphqlError = serde_json::from_str(json_str).must();

        assert_eq!(err.message, "Something went wrong");
        assert!(err.locations.is_empty());
        assert!(err.path.is_empty());
        assert!(err.extensions.is_empty());
    }

    #[test]
    fn test_error_with_path() {
        let json_str = r#"{
            "message": "Field error",
            "path": ["uiapi", "query", "Account", "edges", 0, "node", "Name"]
        }"#;
        let err: GraphqlError = serde_json::from_str(json_str).must();

        assert_eq!(err.path.len(), 7);
        assert_eq!(err.path[0], "uiapi");
        assert_eq!(err.path[4], 0);
    }
}
