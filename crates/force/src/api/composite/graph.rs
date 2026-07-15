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

/// Validates graph reference IDs using the shared identifier validator.
fn validate_reference_id(id: &str) -> Result<()> {
    validator::validate_identifier(id, "Reference ID")
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

/// Normalizes a Composite Graph sub-request URL to a full absolute API path.
///
/// The Composite Graph API resolves sub-request URLs as absolute paths from the
/// API root, so every URL must start with `/services/data/vNN.N/`. This differs
/// from the Batch API, whose [`normalize_subrequest_url`](super::batch::normalize_subrequest_url)
/// emits version-relative URLs (`vNN.N/...`) that Salesforce resolves against
/// `/services/data/`.
///
/// Behavior:
/// - Absolute URLs (`http://`, `https://`) are left untouched.
/// - URLs already rooted at `/services/` (with or without a leading slash) are
///   normalized to a single leading slash and otherwise left untouched.
/// - Version-prefixed URLs (`vNN.N/...`) get the `/services/data/` prefix.
/// - Bare relative URLs (`query?q=...`, `sobjects/Account`) get the full
///   `/services/data/{api_version}/` prefix.
fn normalize_graph_subrequest_url(url: &str, api_version: &str) -> String {
    let trimmed = url.trim_start_matches('/');

    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return url.to_string();
    }

    if trimmed.starts_with("services/") {
        return format!("/{trimmed}");
    }

    if super::batch::is_api_version_prefixed(trimmed) {
        return format!("/services/data/{trimmed}");
    }

    let api_ver = api_version.trim_matches('/');
    format!("/services/data/{api_ver}/{trimmed}")
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
    ///
    /// # Errors
    ///
    /// Returns an error if the total number of subrequests across all graphs exceeds 500.
    pub fn add_graph(mut self, graph: Graph) -> Result<Self> {
        let current_subrequests: usize =
            self.graphs.iter().map(|g| g.composite_request.len()).sum();
        if current_subrequests + graph.composite_request.len() > 500 {
            return Err(ForceError::InvalidInput(
                "Composite Graph limit of 500 total subrequests exceeded".to_string(),
            ));
        }
        self.graphs.push(graph);
        Ok(self)
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

        // Normalize each sub-request URL to a FULL absolute API path. Unlike the
        // Batch API — which resolves version-relative URLs (`v62.0/query?...`)
        // against `/services/data/` — the Composite Graph API requires each
        // sub-request URL to be the absolute path from the API root
        // (`/services/data/v62.0/query?...`). A bare `query?q=...` is otherwise
        // rejected with `PROCESSING_HALTED: "... is not a valid url"`.
        let api_version = self.handler.inner.config.api_version.as_str();
        let mut graphs = self.graphs;
        for graph in &mut graphs {
            for request in &mut graph.composite_request {
                request.url = normalize_graph_subrequest_url(&request.url, api_version);
            }
        }

        let request_body = GraphRequestBody { graphs };

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
    pub fn add_request(mut self, request: GraphRequest) -> Result<Self> {
        if self.composite_request.len() >= 500 {
            return Err(ForceError::InvalidInput(
                "Graph size limit of 500 requests exceeded".to_string(),
            ));
        }
        self.composite_request.push(request);
        Ok(self)
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
        self.add_request(GraphRequest::new(
            "GET",
            crate::api::path_utils::format_sobject_path(sobject, Some(id)),
            reference_id,
        )?)
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
        self.add_request(
            GraphRequest::new(
                "POST",
                crate::api::path_utils::format_sobject_path(sobject, None),
                reference_id,
            )?
            .body(body),
        )
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
        self.add_request(
            GraphRequest::new(
                "PATCH",
                crate::api::path_utils::format_sobject_path(sobject, Some(id)),
                reference_id,
            )?
            .body(body),
        )
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
        self.add_request(GraphRequest::new(
            "DELETE",
            crate::api::path_utils::format_sobject_path(sobject, Some(id)),
            reference_id,
        )?)
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
        let url = crate::api::soql::encode_soql_query_url(&query_builder)?;
        self.add_request(GraphRequest::new("GET", url, reference_id)?)
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
    ) -> Result<Self> {
        let url_str = url.into();
        validator::validate_url_path(&url_str)?;
        Ok(Self {
            method: method.into(),
            url: url_str,
            reference_id: reference_id.into(),
            body: None,
        })
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
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn create_builder() -> CompositeGraphRequest<MockAuthenticator> {
        let auth = MockAuthenticator::new("token", "https://test.salesforce.com");
        let client = client_builder().authenticate(auth).build().await.must();

        client.composite().graph()
    }

    #[tokio::test]
    async fn test_composite_graph_total_limit() {
        let mut builder = create_builder().await;

        let mut graph1 = Graph::new("graph1");
        for i in 0..250 {
            graph1 = graph1
                .get("Account", "001000000000000AAA", &format!("ref{}", i))
                .must();
        }
        builder = builder.add_graph(graph1).must();

        let mut graph2 = Graph::new("graph2");
        for i in 0..251 {
            graph2 = graph2
                .get("Account", "001000000000000AAA", &format!("ref2_{}", i))
                .must();
        }

        // Total would be 501, which exceeds the limit
        let result = builder.add_graph(graph2);
        assert!(matches!(result, Err(ForceError::InvalidInput(_))));
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

        let builder = client.composite().graph().add_graph(graph).must();

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
            .and(path("/services/data/v67.0/composite/graph"))
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
    async fn test_graph_execute_uses_full_services_data_subrequest_urls() {
        use wiremock::matchers::body_json;

        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("token", &mock_server.uri());
        let client = client_builder().authenticate(auth).build().await.must();

        // This drives the exact real code path `tests/live_core.rs` uses:
        // `Graph::new(..).query(SoqlQueryBuilder::new().select(&["Id"]).from("Account").limit(1), ..)`.
        // `encode_soql_query_url` yields a bare `query?q=...` sub-request URL. The
        // Composite Graph API rejects that unless it is the full absolute
        // `/services/data/vNN.N/` path — a version-relative `v62.0/query?...`
        // (the pre-fix batch-style normalization) was echoed and rejected by the
        // live org as `/v62.0/query?...` (OPERATION_NOT_ALLOWED, missing
        // /services/data). Reverting `normalize_graph_subrequest_url` to that
        // behavior makes the `body_json` matcher below miss and this test fail.
        let account_query = crate::api::soql::SoqlQueryBuilder::new()
            .select(&["Id"])
            .from("Account")
            .limit(1);
        let graph = Graph::new("graph1")
            .query(account_query, "acctQuery")
            .must();

        let builder = client.composite().graph().add_graph(graph).must();

        Mock::given(method("POST"))
            .and(path("/services/data/v67.0/composite/graph"))
            .and(body_json(json!({
                "graphs": [
                    {
                        "graphId": "graph1",
                        "compositeRequest": [
                            {
                                "method": "GET",
                                "url": "/services/data/v67.0/query?q=SELECT+Id+FROM+Account+LIMIT+1",
                                "referenceId": "acctQuery"
                            }
                        ]
                    }
                ]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "graphs": [
                    {
                        "graphId": "graph1",
                        "isSuccessful": true,
                        "compositeResponse": [
                            {
                                "body": {"records": []},
                                "httpHeaders": {},
                                "httpStatusCode": 200,
                                "referenceId": "acctQuery"
                            }
                        ]
                    }
                ]
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let response = builder.execute().await.must();
        assert!(response.graphs[0].is_successful);
    }

    #[tokio::test]
    async fn test_graph_execute_empty() {
        let builder = create_builder().await;
        let result = builder.execute().await;

        let Err(ForceError::Serialization(e)) = result else {
            panic!("Expected Serialization error, got {:?}", result);
        };
        assert!(e.to_string().contains("Graph request cannot be empty"));
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
    fn test_normalize_graph_subrequest_url() {
        // Bare relative URLs get the full /services/data/{version}/ prefix.
        assert_eq!(
            normalize_graph_subrequest_url("query?q=SELECT+Id+FROM+Account", "v62.0"),
            "/services/data/v62.0/query?q=SELECT+Id+FROM+Account"
        );
        assert_eq!(
            normalize_graph_subrequest_url("sobjects/Account", "v62.0"),
            "/services/data/v62.0/sobjects/Account"
        );

        // Version-prefixed URLs only need /services/data/.
        assert_eq!(
            normalize_graph_subrequest_url("v62.0/query?q=SELECT+Id", "v67.0"),
            "/services/data/v62.0/query?q=SELECT+Id"
        );

        // Leading-slash + version (`/v62.0/query?...`) — the shape the live org
        // rejected as `/v62.0/query?...` (missing /services/data). The leading
        // slash is trimmed before the version check, so it still gets the full
        // /services/data/ prefix.
        assert_eq!(
            normalize_graph_subrequest_url(
                "/v62.0/query?q=SELECT+Id+FROM+Account+LIMIT+1",
                "v67.0"
            ),
            "/services/data/v62.0/query?q=SELECT+Id+FROM+Account+LIMIT+1"
        );

        // Already-rooted /services/ paths are left untouched (single leading slash).
        assert_eq!(
            normalize_graph_subrequest_url("/services/data/v62.0/sobjects/Account", "v67.0"),
            "/services/data/v62.0/sobjects/Account"
        );
        // Idempotent: a fully-qualified query path is returned verbatim.
        assert_eq!(
            normalize_graph_subrequest_url(
                "/services/data/v62.0/query?q=SELECT+Id+FROM+Account+LIMIT+1",
                "v67.0"
            ),
            "/services/data/v62.0/query?q=SELECT+Id+FROM+Account+LIMIT+1"
        );
        assert_eq!(
            normalize_graph_subrequest_url("services/data/v62.0/sobjects/Account", "v67.0"),
            "/services/data/v62.0/sobjects/Account"
        );

        // Absolute URLs are left untouched.
        assert_eq!(
            normalize_graph_subrequest_url("https://example.com/foo", "v62.0"),
            "https://example.com/foo"
        );

        // API version separators are trimmed so we never emit a double slash.
        assert_eq!(
            normalize_graph_subrequest_url("query?q=SELECT+Id", "/v62.0/"),
            "/services/data/v62.0/query?q=SELECT+Id"
        );
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
    fn test_graph_post_patch_delete() {
        let mut graph = Graph::new("graph1");

        // Valid POST
        graph = graph
            .post("Account", json!({"Name": "Test"}), "refPost")
            .must();

        // Valid PATCH
        graph = graph
            .patch(
                "Account",
                "001000000000000AAA",
                json!({"Name": "Updated"}),
                "refPatch",
            )
            .must();

        // Valid DELETE
        graph = graph
            .delete("Account", "001000000000000AAA", "refDelete")
            .must();

        assert_eq!(graph.composite_request.len(), 3);

        let post_req = &graph.composite_request[0];
        assert_eq!(post_req.method, "POST");
        assert_eq!(post_req.reference_id, "refPost");

        let patch_req = &graph.composite_request[1];
        assert_eq!(patch_req.method, "PATCH");
        assert_eq!(patch_req.reference_id, "refPatch");

        let delete_req = &graph.composite_request[2];
        assert_eq!(delete_req.method, "DELETE");
        assert_eq!(delete_req.reference_id, "refDelete");
    }

    #[test]
    fn test_graph_size_limit() {
        let mut graph = Graph::new("graph1");

        // Add 500 requests
        for i in 0..500 {
            graph = graph
                .get("Account", "001000000000000AAA", &format!("ref{}", i))
                .must();
        }

        // 501st request should fail
        let result = graph.get("Account", "001000000000000AAA", "ref501");

        assert!(
            matches!(result, Err(ForceError::InvalidInput(ref msg)) if msg.contains("limit of 500"))
        );
    }

    #[test]
    fn test_havoc_path_traversal() {
        let graph = Graph::new("graph1");

        let result = graph.get("Account", "../../../../../etc/passwd", "ref1");

        let Err(crate::error::ForceError::InvalidInput(msg)) = result else {
            panic!(
                "Expected InvalidInput error for path traversal, got: {:?}",
                result
            );
        };
        assert!(msg.contains("invalid path traversal characters"));
    }

    #[test]
    fn test_havoc_invalid_reference_id() {
        let graph = Graph::new("graph1");

        let result = graph.get("Account", "001xx000003DHP0AAO", "invalid ref id! @#$");

        let Err(crate::error::ForceError::InvalidInput(msg)) = result else {
            panic!(
                "Expected InvalidInput error for invalid reference id, got: {:?}",
                result
            );
        };
        assert!(msg.contains("Reference ID contains invalid characters"));
    }

    #[test]
    fn test_validate_reference_id_table() {
        let valid_ids = vec!["ref1", "Ref_2", "A", "1", "valid_ref_id_123"];
        let invalid_ids = vec![
            "", "ref-1", "ref 1", "ref!1", "ref@1", "ref#1", "ref$1", "ref%1", "ref^1", "ref&1",
            "ref*1", "ref(1", "ref)1", "ref+1", "ref=1", "ref{1", "ref}1", "ref[1", "ref]1",
            "ref|1", "ref\\1", "ref:1", "ref;1", "ref\"1", "ref'1", "ref<1", "ref>1", "ref,1",
            "ref.1", "ref?1", "ref/1",
        ];

        for id in valid_ids {
            assert!(
                validate_reference_id(id).is_ok(),
                "Expected {} to be valid",
                id
            );
        }

        for id in invalid_ids {
            let result = validate_reference_id(id);
            assert!(result.is_err(), "Expected {} to be invalid", id);
            let Err(ForceError::InvalidInput(msg)) = result else {
                panic!("Expected InvalidInput error for {}", id);
            };
            if id.is_empty() {
                assert_eq!(msg, "Reference ID cannot be empty");
            } else {
                assert!(msg.contains("Reference ID contains invalid characters:"));
            }
        }
    }

    #[test]
    fn test_graph_request_body() {
        let req = GraphRequest::new("POST", "sobjects/Account", "ref1")
            .must()
            .body(serde_json::json!({"Name": "Test"}));
        assert_eq!(req.body.must()["Name"], "Test");
    }

    #[test]
    fn test_validate_graph_id_table() {
        let valid_ids = vec!["001000000000000AAA", "@{ref1.id}"];
        let invalid_ids = vec![
            "",
            "../../../etc/passwd",
            "../something",
            "path\\to",
            "path/to",
            "id?param=1",
        ];

        for id in valid_ids {
            // validate_graph_id is a private helper, but we can test it indirectly
            // by calling graph.get() which uses it.
            let graph = Graph::new("graph1");
            let result = graph.get("Account", id, "ref1");
            assert!(result.is_ok(), "Expected {} to be valid graph id", id);
        }

        for id in invalid_ids {
            let graph = Graph::new("graph1");
            let result = graph.get("Account", id, "ref1");
            assert!(result.is_err(), "Expected {} to be invalid graph id", id);
            let Err(ForceError::InvalidInput(msg)) = result else {
                panic!("Expected InvalidInput error for {}", id);
            };
            if id.is_empty() {
                assert_eq!(msg, "ID cannot be empty");
            } else {
                assert!(msg.contains("ID contains invalid path traversal characters:"));
            }
        }
    }
}
