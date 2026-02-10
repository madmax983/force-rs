//! Composite Graph API support.
//!
//! This module provides a builder interface for constructing and executing
//! Salesforce Composite Graph requests. These requests allow you to execute
//! complex, transactional operations in a single API call.

use crate::auth::Authenticator;
use crate::client::ForceClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Extension trait to add composite support to `ForceClient`.
pub trait ForceClientCompositeExt<A: Authenticator> {
    /// Returns a handler for the Composite Graph API.
    fn composite(&self) -> CompositeHandler<A>;
}

impl<A: Authenticator> ForceClientCompositeExt<A> for ForceClient<A> {
    fn composite(&self) -> CompositeHandler<A> {
        CompositeHandler::new(Arc::clone(self.inner()))
    }
}

/// Handler for Composite Graph API operations.
#[derive(Debug, Clone)]
pub struct CompositeHandler<A: Authenticator> {
    inner: Arc<crate::client::Inner<A>>,
}

impl<A: Authenticator> CompositeHandler<A> {
    pub(crate) fn new(inner: Arc<crate::client::Inner<A>>) -> Self {
        Self { inner }
    }

    /// Executes a composite graph request.
    ///
    /// # Arguments
    ///
    /// * `graph` - The composite graph request to execute
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The response indicates a failure
    pub async fn execute(&self, graph: CompositeGraphRequest) -> Result<CompositeGraphResponse> {
        let token = self.inner.token_manager.token().await?;
        let url = format!(
            "{}/services/data/{}/composite/graph",
            token.instance_url(),
            self.inner.config.api_version
        );

        let request = self
            .inner
            .http_client
            .post(&url)
            .json(&graph)
            .build()
            .map_err(crate::error::HttpError::from)?;

        let response = self.inner.execute_request(request).await?;

        if !response.status().is_success() {
            return Err(crate::http::response_to_force_error(
                response,
                "Composite graph request failed",
            )
            .await);
        }

        let result = response
            .json::<CompositeGraphResponse>()
            .await
            .map_err(crate::error::HttpError::from)?;
        Ok(result)
    }
}

/// A request to the Composite Graph API.
#[derive(Debug, Clone, Serialize, Default)]
pub struct CompositeGraphRequest {
    /// The list of graphs to execute.
    pub graphs: Vec<Graph>,
}

impl CompositeGraphRequest {
    /// Creates a new empty composite graph request.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a graph to the request.
    #[must_use]
    pub fn add_graph(mut self, graph: Graph) -> Self {
        self.graphs.push(graph);
        self
    }
}

/// A single graph in a composite request.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Graph {
    /// Unique identifier for the graph.
    pub graph_id: String,
    /// List of operations in the graph.
    pub composite_request: Vec<CompositeRequestItem>,
}

impl Graph {
    /// Creates a new graph with the given ID.
    pub fn new(graph_id: impl Into<String>) -> Self {
        Self {
            graph_id: graph_id.into(),
            composite_request: Vec::new(),
        }
    }

    /// Adds an item to the graph.
    #[must_use]
    pub fn add_item(mut self, item: CompositeRequestItem) -> Self {
        self.composite_request.push(item);
        self
    }
}

/// An individual operation within a graph.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositeRequestItem {
    /// The URL for the operation (relative to /services/data/vXX.X/).
    pub url: String,
    /// The HTTP method to use.
    pub method: String,
    /// Unique reference ID for this item.
    pub reference_id: String,
    /// The body of the request (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
    /// HTTP headers to include (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_headers: Option<std::collections::HashMap<String, String>>,
}

impl CompositeRequestItem {
    /// Creates a new request item.
    pub fn new(method: &str, url: &str, reference_id: &str) -> Self {
        Self {
            url: url.to_string(),
            method: method.to_string(),
            reference_id: reference_id.to_string(),
            body: None,
            http_headers: None,
        }
    }

    /// Sets the body of the request.
    pub fn body<T: Serialize>(mut self, body: T) -> serde_json::Result<Self> {
        self.body = Some(serde_json::to_value(body)?);
        Ok(self)
    }

    /// Sets the body from a JSON value.
    #[must_use]
    pub fn json_body(mut self, body: serde_json::Value) -> Self {
        self.body = Some(body);
        self
    }
}

/// Response from the Composite Graph API.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositeGraphResponse {
    /// List of results for each graph.
    pub graphs: Vec<GraphResult>,
}

/// Result of a single graph execution.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphResult {
    /// ID of the graph.
    pub graph_id: String,
    /// Whether the graph execution was successful.
    pub is_successful: bool,
    /// Results for each operation in the graph.
    pub graph_response: CompositeSubResponse,
}

/// Sub-response containing the list of individual operation results.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositeSubResponse {
    /// List of results for each operation.
    pub composite_response: Vec<CompositeResponseItem>,
}

/// Result of a single operation.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositeResponseItem {
    /// Reference ID of the operation.
    pub reference_id: String,
    /// HTTP status code.
    pub http_status_code: u16,
    /// Response body.
    pub body: Option<serde_json::Value>,
    /// HTTP headers.
    pub http_headers: Option<std::collections::HashMap<String, String>>,
}

/// Builder helper for constructing SObject URLs.
pub struct SObjectUrl;

impl SObjectUrl {
    /// Returns the URL for creating a record of the given type.
    pub fn create(sobject: &str) -> String {
        format!("/services/data/v{{version}}/sobjects/{}", sobject)
    }

    /// Returns the URL for modifying a record with the given ID.
    pub fn id(sobject: &str, id: &str) -> String {
        format!("/services/data/v{{version}}/sobjects/{}/{}", sobject, id)
    }
}

/// Helper to create reference strings for dependent requests.
pub fn reference(reference_id: &str, field: &str) -> String {
    format!("@{{{}.{}}}", reference_id, field)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::builder;
    use crate::test_support::Must;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // Red Phase: Tests that would fail if implemented wrong (or verified here)

    #[test]
    fn test_graph_serialization() {
        let item1 = CompositeRequestItem::new(
            "POST",
            "/services/data/v60.0/sobjects/Account",
            "refAccount",
        )
        .json_body(json!({"Name": "Acme"}));

        let item2 = CompositeRequestItem::new(
            "POST",
            "/services/data/v60.0/sobjects/Contact",
            "refContact",
        )
        .json_body(json!({
            "LastName": "Doe",
            "AccountId": reference("refAccount", "id")
        }));

        let graph = Graph::new("graph1").add_item(item1).add_item(item2);

        let request = CompositeGraphRequest::new().add_graph(graph);

        let json = serde_json::to_string(&request).must();
        assert!(json.contains("\"graphId\":\"graph1\""));
        assert!(json.contains("\"referenceId\":\"refAccount\""));
        assert!(json.contains("\"referenceId\":\"refContact\""));
        assert!(json.contains("@{refAccount.id}"));
    }

    #[tokio::test]
    async fn test_composite_execution() {
        use crate::auth::{AccessToken, Authenticator, TokenResponse};
        use async_trait::async_trait;

        #[derive(Debug, Clone)]
        struct MockAuthenticator {
            token: String,
            instance_url: String,
        }

        impl MockAuthenticator {
            fn new(token: &str, instance_url: &str) -> Self {
                Self {
                    token: token.to_string(),
                    instance_url: instance_url.to_string(),
                }
            }
        }

        #[async_trait]
        impl Authenticator for MockAuthenticator {
            async fn authenticate(&self) -> crate::error::Result<AccessToken> {
                Ok(AccessToken::from_response(TokenResponse {
                    access_token: self.token.clone(),
                    instance_url: self.instance_url.clone(),
                    token_type: "Bearer".to_string(),
                    issued_at: "1704067200000".to_string(),
                    signature: "test_sig".to_string(),
                    expires_in: Some(7200),
                    refresh_token: None,
                }))
            }

            async fn refresh(&self) -> crate::error::Result<AccessToken> {
                self.authenticate().await
            }
        }

        let mock_server = MockServer::start().await;

        // Mock the Composite Graph endpoint
        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/composite/graph"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "graphs": [
                    {
                        "graphId": "graph1",
                        "isSuccessful": true,
                        "graphResponse": {
                            "compositeResponse": [
                                {
                                    "referenceId": "refAccount",
                                    "httpStatusCode": 201,
                                    "body": {
                                        "id": "001xx0000000001AAA",
                                        "success": true
                                    }
                                }
                            ]
                        }
                    }
                ]
            })))
            .mount(&mock_server)
            .await;

        // Setup client
        let auth = MockAuthenticator::new("token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        // Use extension trait
        let handler = client.composite();

        let request = CompositeGraphRequest::new().add_graph(
            Graph::new("graph1").add_item(CompositeRequestItem::new("POST", "/url", "refAccount")),
        );

        let response = handler.execute(request).await.must();

        assert_eq!(response.graphs.len(), 1);
        assert!(response.graphs[0].is_successful);
        assert_eq!(response.graphs[0].graph_id, "graph1");
        assert_eq!(
            response.graphs[0].graph_response.composite_response[0].http_status_code,
            201
        );
    }
}
