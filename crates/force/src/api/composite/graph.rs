//! Composite Graph API.
//!
//! The Composite Graph API provides an enhanced way to perform complex, dependent operations
//! in a single request. Unlike the Batch API, which is a flat list of independent requests,
//! the Graph API allows you to model dependencies between requests using a graph structure.
//!
//! Each graph can contain up to 500 nodes (requests) and a maximum depth of 15.
//! You can execute multiple graphs in a single API call.

use super::CompositeHandler;
use crate::api::soql::SoqlQueryBuilder;
use crate::auth::Authenticator;
use crate::error::{ForceError, Result};
use crate::types::validator;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Helper to validate graph reference IDs.
/// Reference IDs must be strictly alphanumeric/underscores.
fn validate_reference_id(id: &str) -> Result<()> {
    if id.is_empty() {
        return Err(ForceError::InvalidInput(
            "Reference ID cannot be empty".to_string(),
        ));
    }
    if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(ForceError::InvalidInput(format!(
            "Reference ID contains invalid characters: {}",
            id
        )));
    }
    Ok(())
}

/// Helper to validate graph IDs.
/// IDs can be Salesforce IDs or references (e.g. "@{ref.id}"),
/// but they strictly cannot contain path traversal characters.
fn validate_graph_id(id: &str) -> Result<()> {
    if id.is_empty() {
        return Err(ForceError::InvalidInput("ID cannot be empty".to_string()));
    }
    if id.contains('/') || id.contains("..") || id.contains('\\') || id.contains('?') {
        return Err(ForceError::InvalidInput(format!(
            "ID contains invalid path traversal characters: {}",
            id
        )));
    }
    Ok(())
}

/// Constructs a Composite Graph request.
///
/// Use this request object to add one or more graphs and execute them atomically.
#[derive(Debug)]
pub struct CompositeGraphRequest<A: Authenticator> {
    handler: CompositeHandler<A>,
    graphs: Vec<Graph>,
}

impl<A: Authenticator> CompositeGraphRequest<A> {
    /// Creates a new CompositeGraphRequest.
    ///
    /// # Performance
    ///
    /// Pre-allocates capacity for 15 graphs (typical small batch)
    /// to avoid heap reallocations during graph accumulation.
    pub(crate) fn new(handler: CompositeHandler<A>) -> Self {
        Self {
            handler,
            graphs: Vec::with_capacity(15),
        }
    }

    /// Adds a graph to the request.
    ///
    /// # Arguments
    ///
    /// * `graph` - The graph to add
    #[must_use]
    pub fn add_graph(mut self, graph: Graph) -> Self {
        self.graphs.push(graph);
        self
    }

    /// Executes the graph request.
    ///
    /// Sends all accumulated graphs to the Salesforce Composite Graph API.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The response cannot be deserialized
    pub async fn execute(self) -> Result<GraphResponse> {
        if self.graphs.is_empty() {
            return Err(ForceError::Serialization(
                crate::error::SerializationError::InvalidFormat(
                    "Graph request cannot be empty".to_string(),
                ),
            ));
        }

        // Construct the composite graph URL
        let url = self.handler.inner.resolve_url("composite/graph").await?;

        let request_body = GraphRequestBody {
            graphs: self.graphs,
        };

        let request = self
            .handler
            .inner
            .post(&url)
            .json(&request_body)
            .build()
            .map_err(crate::error::HttpError::from)?;

        self.handler
            .inner
            .send_request_and_decode(request, "Composite Graph failed")
            .await
    }
}

/// A single graph within a Composite Graph request.
///
/// A graph is a collection of dependent subrequests.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Graph {
    /// Unique identifier for this graph.
    pub graph_id: String,
    /// List of subrequests in this graph.
    pub composite_request: Vec<GraphRequest>,
}

impl Graph {
    /// Creates a new Graph.
    ///
    /// # Arguments
    ///
    /// * `graph_id` - A unique identifier for this graph (e.g., "graph1")
    ///
    /// # Performance
    ///
    /// Pre-allocates capacity for 15 subrequests (typical small graph)
    /// to avoid heap reallocations during request accumulation.
    #[must_use]
    pub fn new(graph_id: impl Into<String>) -> Self {
        Self {
            graph_id: graph_id.into(),
            composite_request: Vec::with_capacity(15),
        }
    }

    /// Adds a subrequest to the graph.
    ///
    /// # Arguments
    ///
    /// * `request` - The subrequest to add
    #[must_use]
    pub fn add_request(mut self, request: GraphRequest) -> Self {
        self.composite_request.push(request);
        self
    }

    /// Adds a GET request to the graph.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The SObject type (e.g., "Account")
    /// * `id` - The record ID or reference (e.g., "@{ref.id}")
    /// * `reference_id` - Unique reference ID for this request
    pub fn get(self, sobject: &str, id: &str, reference_id: &str) -> Result<Self> {
        validator::validate_sobject_name(sobject)?;
        validate_graph_id(id)?;
        validate_reference_id(reference_id)?;
        Ok(self.add_request(GraphRequest::new(
            "GET",
            crate::api::path_utils::format_sobject_path(sobject, Some(id)),
            reference_id,
        )))
    }

    /// Adds a POST (Create) request to the graph.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The SObject type (e.g., "Account")
    /// * `body` - The JSON body of the record
    /// * `reference_id` - Unique reference ID for this request
    pub fn post(self, sobject: &str, body: Value, reference_id: &str) -> Result<Self> {
        validator::validate_sobject_name(sobject)?;
        validate_reference_id(reference_id)?;
        Ok(self.add_request(
            GraphRequest::new(
                "POST",
                crate::api::path_utils::format_sobject_path(sobject, None),
                reference_id,
            )
            .body(body),
        ))
    }

    /// Adds a PATCH (Update) request to the graph.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The SObject type (e.g., "Account")
    /// * `id` - The record ID or reference
    /// * `body` - The JSON body with fields to update
    /// * `reference_id` - Unique reference ID for this request
    pub fn patch(self, sobject: &str, id: &str, body: Value, reference_id: &str) -> Result<Self> {
        validator::validate_sobject_name(sobject)?;
        validate_graph_id(id)?;
        validate_reference_id(reference_id)?;
        Ok(self.add_request(
            GraphRequest::new(
                "PATCH",
                crate::api::path_utils::format_sobject_path(sobject, Some(id)),
                reference_id,
            )
            .body(body),
        ))
    }

    /// Adds a DELETE request to the graph.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The SObject type (e.g., "Account")
    /// * `id` - The record ID or reference
    /// * `reference_id` - Unique reference ID for this request
    pub fn delete(self, sobject: &str, id: &str, reference_id: &str) -> Result<Self> {
        validator::validate_sobject_name(sobject)?;
        validate_graph_id(id)?;
        validate_reference_id(reference_id)?;
        Ok(self.add_request(GraphRequest::new(
            "DELETE",
            crate::api::path_utils::format_sobject_path(sobject, Some(id)),
            reference_id,
        )))
    }

    /// Adds a SOQL query request to the graph.
    ///
    /// The query will be properly URL-encoded according to form-urlencoded rules
    /// (e.g., spaces become `+`, `%` becomes `%25`).
    ///
    /// # Arguments
    ///
    /// * `query_builder` - The SOQL query builder
    /// * `reference_id` - Unique reference ID for this request
    ///
    /// # Errors
    ///
    /// Returns an error if the query builder contains validation errors
    /// or if formatting fails.
    #[allow(clippy::needless_pass_by_value)] // Ownership consumed to enforce builder pattern
    pub fn query(self, query_builder: SoqlQueryBuilder, reference_id: &str) -> Result<Self> {
        validate_reference_id(reference_id)?;

        if let Err(e) = query_builder.validate() {
            return Err(ForceError::InvalidInput(format!(
                "Invalid query builder: {}",
                e
            )));
        }

        let mut url = String::with_capacity(256 + 8);
        url.push_str("query?q=");

        {
            // We use a custom writer adapter to encode directly into the URL string.
            // This is identical to how `batch.rs` does it, to avoid intermediate allocations.
            struct UrlEncodedWriter<'a>(&'a mut String);

            impl std::fmt::Write for UrlEncodedWriter<'_> {
                fn write_str(&mut self, s: &str) -> std::fmt::Result {
                    self.0
                        .extend(url::form_urlencoded::byte_serialize(s.as_bytes()));
                    Ok(())
                }
            }

            let mut writer = UrlEncodedWriter(&mut url);
            if let Err(e) = query_builder.write_query(&mut writer) {
                return Err(ForceError::InvalidInput(format!(
                    "Formatting failed: {}",
                    e
                )));
            }
        }

        Ok(self.add_request(GraphRequest::new("GET", url, reference_id)))
    }
}

/// A single subrequest within a graph.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GraphRequest {
    /// HTTP method (GET, POST, PATCH, DELETE, etc.)
    pub method: String,
    /// Relative URL (e.g., "sobjects/Account")
    pub url: String,
    /// Unique reference ID for dependent requests
    pub reference_id: String,
    /// Request body (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<Value>,
}

impl GraphRequest {
    /// Creates a new GraphRequest.
    ///
    /// # Arguments
    ///
    /// * `method` - HTTP method
    /// * `url` - Relative URL
    /// * `reference_id` - Unique reference ID
    pub fn new(
        method: impl Into<String>,
        url: impl Into<String>,
        reference_id: impl Into<String>,
    ) -> Self {
        Self {
            method: method.into(),
            url: url.into(),
            reference_id: reference_id.into(),
            body: None,
        }
    }

    /// Sets the request body.
    #[must_use]
    pub fn body(mut self, body: Value) -> Self {
        self.body = Some(body);
        self
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphRequestBody {
    graphs: Vec<Graph>,
}

/// The response from a Composite Graph API call.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphResponse {
    /// List of results for each graph executed.
    pub graphs: Vec<GraphResult>,
}

/// The result of a single graph execution.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphResult {
    /// The ID of the graph.
    pub graph_id: String,
    /// True if the graph execution was successful.
    pub is_successful: bool,
    /// List of results for each subrequest in the graph.
    #[serde(default)]
    pub composite_response: Vec<GraphSubResponse>,
    /// Error details if the graph failed (e.g., transaction rolled back).
    #[serde(default)]
    pub graph_response: Option<GraphErrorResponse>,
}

/// Detailed error response for a failed graph.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphErrorResponse {
    /// List of subrequests with their statuses.
    pub composite_response: Vec<GraphSubResponse>,
}

/// The result of a single subrequest within a graph.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphSubResponse {
    /// The HTTP status code.
    pub http_status_code: u16,
    /// The reference ID of the request.
    pub reference_id: String,
    /// The body of the response (can be success or error).
    pub body: Option<Value>,
    /// HTTP headers (optional).
    pub http_headers: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::builder as client_builder;
    use crate::test_support::{MockAuthenticator, Must};
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn create_builder() -> CompositeGraphRequest<MockAuthenticator> {
        let auth = MockAuthenticator::new("token", "https://test.salesforce.com");
        let client = client_builder().authenticate(auth).build().await.must();

        client.composite().graph()
    }

    #[test]
    fn test_graph_serialization() {
        let mut graph = Graph::new("graph1");
        graph = graph
            .post("Account", json!({"Name": "RefAccount"}), "refAccount")
            .must();
        graph = graph
            .post(
                "Contact",
                json!({
                    "LastName": "Doe",
                    "AccountId": "@{refAccount.id}"
                }),
                "refContact",
            )
            .must();

        let req = GraphRequestBody {
            graphs: vec![graph],
        };

        let json = serde_json::to_string(&req).must();
        assert!(json.contains("\"graphId\":\"graph1\""));
        assert!(json.contains("\"referenceId\":\"refAccount\""));
        assert!(json.contains("\"referenceId\":\"refContact\""));
        assert!(json.contains("\"AccountId\":\"@{refAccount.id}\""));
    }

    #[tokio::test]
    async fn test_graph_execute_success() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("token", &mock_server.uri());
        let client = client_builder().authenticate(auth).build().await.must();

        let mut graph = Graph::new("graph1");
        graph = graph
            .post("Account", json!({"Name": "Test"}), "acc1")
            .must();

        let builder = client.composite().graph().add_graph(graph);

        let response_json = json!({
            "graphs": [
                {
                    "graphId": "graph1",
                    "isSuccessful": true,
                    "compositeResponse": [
                        {
                            "body": {
                                "id": "001...",
                                "success": true,
                                "errors": []
                            },
                            "httpHeaders": {},
                            "httpStatusCode": 201,
                            "referenceId": "acc1"
                        }
                    ]
                }
            ]
        });

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/composite/graph"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_json))
            .mount(&mock_server)
            .await;

        let response = builder.execute().await.must();
        assert_eq!(response.graphs.len(), 1);
        assert!(response.graphs[0].is_successful);
        assert_eq!(response.graphs[0].graph_id, "graph1");
        assert_eq!(
            response.graphs[0].composite_response[0].reference_id,
            "acc1"
        );
    }

    #[tokio::test]
    async fn test_graph_execute_empty() {
        let builder = create_builder().await;
        let result = builder.execute().await;

        match result {
            Err(ForceError::Serialization(e)) => {
                assert!(e.to_string().contains("Graph request cannot be empty"));
            }
            _ => panic!("Expected Serialization error, got {:?}", result),
        }
    }

    #[test]
    fn test_graph_query_encoding() {
        let query = SoqlQueryBuilder::new()
            .select(&["Id", "Name"])
            .from("Account")
            .where_eq("Name", "Acme & Co.");

        let graph = Graph::new("graph1").query(query, "refQuery").must();

        assert_eq!(graph.composite_request.len(), 1);

        let req = &graph.composite_request[0];
        assert_eq!(req.method, "GET");
        assert_eq!(req.reference_id, "refQuery");
        assert!(req.body.is_none());

        // Expected SOQL: SELECT Id, Name FROM Account WHERE Name = 'Acme & Co.'
        // Encoded: query?q=SELECT+Id%2C+Name+FROM+Account+WHERE+Name+%3D+%27Acme+%26+Co.%27
        let expected_url =
            "query?q=SELECT+Id%2C+Name+FROM+Account+WHERE+Name+%3D+%27Acme+%26+Co.%27";
        assert_eq!(req.url, expected_url);
    }

    #[test]
    fn test_validate_reference_id() {
        assert!(validate_reference_id("valid_id_123").is_ok());
        assert!(validate_reference_id("validId").is_ok());
        assert!(validate_reference_id("").is_err());
        assert!(validate_reference_id("invalid ref id! @#$").is_err());
        assert!(validate_reference_id("invalid-ref").is_err());
    }

    #[test]
    fn test_validate_graph_id() {
        assert!(validate_graph_id("001000000000000").is_ok());
        assert!(validate_graph_id("@{ref.id}").is_ok());
        assert!(validate_graph_id("validId").is_ok());
        assert!(validate_graph_id("").is_err());
        assert!(validate_graph_id("some/path").is_err());
        assert!(validate_graph_id("..").is_err());
        assert!(validate_graph_id("path\\test").is_err());
        assert!(validate_graph_id("path?query").is_err());
    }

    #[test]
    fn test_havoc_path_traversal() {
        let graph = Graph::new("graph1");

        let result = graph.get("Account", "../../../../../etc/passwd", "ref1");

        let Err(err) = result else {
            panic!("Expected an error");
        };
        match err {
            crate::error::ForceError::InvalidInput(msg) => {
                assert!(msg.contains("invalid path traversal characters"));
            }
            _ => panic!(
                "Expected InvalidInput error for path traversal, got: {:?}",
                err
            ),
        }
    }

    #[test]
    fn test_havoc_invalid_reference_id() {
        let graph = Graph::new("graph1");

        let result = graph.get("Account", "001xx000003DHP0AAO", "invalid ref id! @#$");

        let Err(err) = result else {
            panic!("Expected an error");
        };
        match err {
            crate::error::ForceError::InvalidInput(msg) => {
                assert!(msg.contains("Reference ID contains invalid characters"));
            }
            _ => panic!(
                "Expected InvalidInput error for invalid reference id, got: {:?}",
                err
            ),
        }
    }
}
