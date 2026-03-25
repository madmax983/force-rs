//! Salesforce GraphQL API handler.
//!
//! The GraphQL API (`POST /services/data/vXX.0/graphql`) provides a unified
//! query interface using the GraphQL query language. Unlike the REST API,
//! GraphQL allows requesting specific fields, nested relationships, and
//! multiple objects in a single request.
//!
//! # Feature Flag
//!
//! This module requires the `graphql` feature flag:
//! ```toml
//! [dependencies]
//! force = { version = "...", features = ["graphql"] }
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use force::api::graphql::GraphqlRequest;
//!
//! let client = builder().authenticate(auth).build().await?;
//! let gql = client.graphql();
//!
//! // Simple raw query
//! let data = gql.query_raw("{ uiapi { query { Account { edges { node { Id } } } } } }", None).await?;
//!
//! // Typed query with variables
//! let req = GraphqlRequest::new("query($limit: Int) { ... }")
//!     .with_variables(serde_json::json!({"limit": 10}));
//! let result: MyData = gql.query(&req).await?;
//! ```

pub(crate) mod error;
pub(crate) mod types;

// Re-export key types at module level
pub use error::GraphqlErrorResponse;
pub use types::{GraphqlError, GraphqlErrorLocation, GraphqlRequest, GraphqlResponse};

use crate::error::Result;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;

/// GraphQL API handler for Salesforce.
///
/// Provides access to the Salesforce GraphQL API via a single POST endpoint.
/// The handler supports typed queries, raw queries, and partial-success inspection.
///
/// Obtain a handler from [`ForceClient::graphql`](crate::client::ForceClient::graphql).
#[derive(Debug)]
pub struct GraphqlHandler<A: crate::auth::Authenticator> {
    /// Reference to the shared session state.
    inner: Arc<crate::session::Session<A>>,
}

impl<A: crate::auth::Authenticator> Clone for GraphqlHandler<A> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<A: crate::auth::Authenticator> GraphqlHandler<A> {
    /// Creates a new GraphQL handler wrapping the given session.
    #[must_use]
    pub(crate) fn new(inner: Arc<crate::session::Session<A>>) -> Self {
        Self { inner }
    }

    /// Resolves the GraphQL endpoint URL.
    ///
    /// Returns: `{instance_url}/services/data/{version}/graphql`
    pub(crate) async fn resolve_graphql_url(&self) -> Result<String> {
        self.inner.resolve_url("graphql").await
    }

    /// Executes a GraphQL request and returns the deserialized `data` field.
    ///
    /// If the response contains errors without data, returns
    /// [`ForceError::GraphQL`](crate::error::ForceError::GraphQL).
    /// If both `data` and `errors` are present (partial success), the data is
    /// returned — use [`query_with_errors`](Self::query_with_errors) to inspect
    /// errors in that case.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails (network, auth, rate limit)
    /// - The response contains GraphQL errors without data
    /// - The response body cannot be deserialized
    pub async fn query<T: DeserializeOwned>(&self, request: &GraphqlRequest) -> Result<T> {
        let url = self.resolve_graphql_url().await?;
        let http_request = self
            .inner
            .post(&url)
            .json(request)
            .build()
            .map_err(crate::error::HttpError::from)?;

        let response = self.inner.execute_request(http_request).await?;

        if !response.status().is_success() {
            return Err(
                crate::http::response_to_force_error(response, "GraphQL request failed").await,
            );
        }

        let envelope: GraphqlResponse<T> = response
            .json()
            .await
            .map_err(crate::error::HttpError::from)?;

        match (envelope.data, envelope.errors) {
            (Some(data), _) => Ok(data),
            (None, Some(errors)) if !errors.is_empty() => Err(GraphqlErrorResponse(errors).into()),
            (None, _) => Err(crate::error::ForceError::InvalidInput(
                "GraphQL response contained neither data nor errors".to_string(),
            )),
        }
    }

    /// Executes a GraphQL request and returns the full response envelope.
    ///
    /// Use this when you need to inspect both `data` and `errors` (partial
    /// success), or when you want access to response extensions.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP request fails or the response cannot be deserialized.
    pub async fn query_with_errors<T: DeserializeOwned>(
        &self,
        request: &GraphqlRequest,
    ) -> Result<GraphqlResponse<T>> {
        let url = self.resolve_graphql_url().await?;
        let http_request = self
            .inner
            .post(&url)
            .json(request)
            .build()
            .map_err(crate::error::HttpError::from)?;

        let response = self.inner.execute_request(http_request).await?;

        if !response.status().is_success() {
            return Err(
                crate::http::response_to_force_error(response, "GraphQL request failed").await,
            );
        }

        response
            .json::<GraphqlResponse<T>>()
            .await
            .map_err(crate::error::HttpError::from)
            .map_err(Into::into)
    }

    /// Convenience method: executes a raw GraphQL query string.
    ///
    /// Returns the `data` field as `serde_json::Value`.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails or the response contains only errors.
    pub async fn query_raw(&self, query: &str, variables: Option<Value>) -> Result<Value> {
        let mut request = GraphqlRequest::new(query);
        if let Some(vars) = variables {
            request = request.with_variables(vars);
        }
        self.query(&request).await
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use crate::client::{ForceClient, builder};
    use crate::test_support::{MockAuthenticator, Must, MustMsg};

    async fn test_client() -> ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", "https://test.salesforce.com");
        builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("failed to create test client")
    }

    #[tokio::test]
    async fn test_graphql_handler_construction() {
        let client = test_client().await;
        let _handler = client.graphql();
    }

    #[tokio::test]
    async fn test_graphql_handler_is_cloneable() {
        let client = test_client().await;
        let h1 = client.graphql();
        let h2 = h1.clone();
        let url1 = h1.resolve_graphql_url().await.must();
        let url2 = h2.resolve_graphql_url().await.must();
        assert_eq!(url1, url2);
    }

    #[tokio::test]
    async fn test_resolve_graphql_url() {
        let client = test_client().await;
        let handler = client.graphql();
        let url = handler.resolve_graphql_url().await.must();
        assert!(url.contains("/services/data/"));
        assert!(url.ends_with("/graphql"));
    }

    #[tokio::test]
    async fn test_graphql_url_includes_api_version() {
        let client = test_client().await;
        let handler = client.graphql();
        let url = handler.resolve_graphql_url().await.must();
        assert!(url.contains("v60.0"), "URL should contain API version");
    }

    #[tokio::test]
    async fn test_graphql_handler_debug() {
        let client = test_client().await;
        let handler = client.graphql();
        let debug = format!("{handler:?}");
        assert!(!debug.is_empty());
    }

    #[tokio::test]
    async fn test_multiple_handlers_share_session() {
        let client = test_client().await;
        let h1 = client.graphql();
        let h2 = client.graphql();
        let url1 = h1.resolve_graphql_url().await.must();
        let url2 = h2.resolve_graphql_url().await.must();
        assert_eq!(url1, url2);
    }
}

#[cfg(test)]
mod integration_tests {
    #![allow(clippy::unwrap_used, clippy::items_after_statements)]

    use super::*;
    use crate::client::builder;
    use crate::test_support::{MockAuthenticator, Must};
    use serde::Deserialize;
    use serde_json::json;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn setup() -> (MockServer, GraphqlHandler<MockAuthenticator>) {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();
        let handler = client.graphql();
        (mock_server, handler)
    }

    #[tokio::test]
    async fn test_query_success() {
        let (mock_server, handler) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/graphql"))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "uiapi": {
                        "query": {
                            "Account": {
                                "edges": [
                                    {"node": {"Id": "001xx000003DHP0AAA", "Name": {"value": "Acme"}}}
                                ]
                            }
                        }
                    }
                }
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let req = GraphqlRequest::new(
            "{ uiapi { query { Account { edges { node { Id Name { value } } } } } } }",
        );
        let data: Value = handler.query(&req).await.must();
        assert_eq!(
            data["uiapi"]["query"]["Account"]["edges"][0]["node"]["Id"],
            "001xx000003DHP0AAA"
        );
    }

    #[tokio::test]
    async fn test_query_with_variables() {
        let (mock_server, handler) = setup().await;

        let expected_body = json!({
            "query": "query($limit: Int) { uiapi { query { Account(first: $limit) { edges { node { Id } } } } } }",
            "variables": {"limit": 5}
        });

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/graphql"))
            .and(body_json(&expected_body))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {"uiapi": {"query": {"Account": {"edges": []}}}}
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let req = GraphqlRequest::new(
            "query($limit: Int) { uiapi { query { Account(first: $limit) { edges { node { Id } } } } } }",
        )
        .with_variables(json!({"limit": 5}));

        let data: Value = handler.query(&req).await.must();
        assert!(
            data["uiapi"]["query"]["Account"]["edges"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn test_query_with_operation_name() {
        let (mock_server, handler) = setup().await;

        let expected_body = json!({
            "query": "query GetAccounts { uiapi { query { Account { edges { node { Id } } } } } }",
            "operation_name": "GetAccounts"
        });

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/graphql"))
            .and(body_json(&expected_body))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {"result": "ok"}
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let req = GraphqlRequest::new(
            "query GetAccounts { uiapi { query { Account { edges { node { Id } } } } } }",
        )
        .with_operation_name("GetAccounts");

        let _: Value = handler.query(&req).await.must();
    }

    #[tokio::test]
    async fn test_query_errors_empty() {
        let (mock_server, handler) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": null,
                "errors": []
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let req = GraphqlRequest::new("{ bad query }");
        let result: crate::error::Result<Value> = handler.query(&req).await;

        let Err(err) = result else {
            panic!("Expected an error");
        };
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("neither data nor errors"),
            "Error should complain about neither data nor errors, got: {err_msg}"
        );
    }

    #[tokio::test]
    async fn test_query_graphql_errors_only() {
        let (mock_server, handler) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": null,
                "errors": [
                    {
                        "message": "Cannot query field 'Foo' on type 'Account'",
                        "locations": [{"line": 1, "column": 45}],
                        "extensions": {"errorCode": "INVALID_FIELD"}
                    }
                ]
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let req = GraphqlRequest::new("{ bad query }");
        let result: crate::error::Result<Value> = handler.query(&req).await;

        let Err(err) = result else {
            panic!("Expected an error");
        };
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("Cannot query field 'Foo'"),
            "Error should contain the GraphQL error message, got: {err_msg}"
        );
    }

    #[tokio::test]
    async fn test_query_partial_success_returns_data() {
        let (mock_server, handler) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {"partial": "result"},
                "errors": [{"message": "Insufficient access to field 'Revenue'"}]
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let req = GraphqlRequest::new("{ partial query }");
        // query<T> returns data when both data and errors present
        let data: Value = handler.query(&req).await.must();
        assert_eq!(data["partial"], "result");
    }

    #[tokio::test]
    async fn test_query_with_errors_returns_envelope() {
        let (mock_server, handler) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {"partial": "result"},
                "errors": [{"message": "Warning: deprecated field"}]
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let req = GraphqlRequest::new("{ query }");
        let envelope: GraphqlResponse<Value> = handler.query_with_errors(&req).await.must();

        assert!(envelope.data.is_some());
        assert!(envelope.has_errors());
        assert_eq!(
            envelope.errors.unwrap()[0].message,
            "Warning: deprecated field"
        );
    }

    #[tokio::test]
    async fn test_query_http_error() {
        let (mock_server, handler) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/graphql"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .expect(1)
            .mount(&mock_server)
            .await;

        let req = GraphqlRequest::new("{ query }");
        let result: crate::error::Result<Value> = handler.query(&req).await;
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(
            matches!(
                err,
                crate::error::ForceError::Api(_) | crate::error::ForceError::Http(_)
            ),
            "Expected Api or Http error, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_query_raw_convenience() {
        let (mock_server, handler) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {"count": 42}
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let data = handler.query_raw("{ count }", None).await.must();
        assert_eq!(data["count"], 42);
    }

    #[tokio::test]
    async fn test_query_raw_with_variables() {
        let (mock_server, handler) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {"result": "ok"}
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let data = handler
            .query_raw("query($x: Int) { f(x: $x) }", Some(json!({"x": 1})))
            .await
            .must();
        assert_eq!(data["result"], "ok");
    }

    #[tokio::test]
    async fn test_query_typed_deserialization() {
        let (mock_server, handler) = setup().await;

        #[derive(Debug, Deserialize)]
        struct AccountData {
            name: String,
            count: u32,
        }

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {"name": "Acme", "count": 7}
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let req = GraphqlRequest::new("{ query }");
        let data: AccountData = handler.query(&req).await.must();
        assert_eq!(data.name, "Acme");
        assert_eq!(data.count, 7);
    }

    #[tokio::test]
    async fn test_query_empty_response() {
        let (mock_server, handler) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&mock_server)
            .await;

        let req = GraphqlRequest::new("{ query }");
        let result: crate::error::Result<Value> = handler.query(&req).await;
        let Err(err_val) = result else {
            panic!("Expected an error");
        };
        let err = err_val.to_string();
        assert!(err.contains("neither data nor errors"), "Got: {err}");
    }

    #[tokio::test]
    async fn test_query_salesforce_realistic_response() {
        let (mock_server, handler) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "uiapi": {
                        "query": {
                            "Account": {
                                "edges": [
                                    {
                                        "node": {
                                            "Id": "001xx000003DHP0AAA",
                                            "Name": {"value": "Acme Corp", "displayValue": null},
                                            "Industry": {"value": "Technology", "displayValue": "Technology"}
                                        },
                                        "cursor": "eyJsaW1pdCI6MSwib2Zmc2V0IjowfQ=="
                                    }
                                ],
                                "pageInfo": {
                                    "hasNextPage": true,
                                    "hasPreviousPage": false
                                },
                                "totalCount": 150
                            }
                        }
                    }
                }
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let req = GraphqlRequest::new(
            "{ uiapi { query { Account(first: 1) { edges { node { Id Name { value } Industry { value displayValue } } cursor } pageInfo { hasNextPage } totalCount } } } }",
        );
        let data: Value = handler.query(&req).await.must();

        let account = &data["uiapi"]["query"]["Account"];
        assert_eq!(account["totalCount"], 150);
        assert_eq!(account["pageInfo"]["hasNextPage"], true);

        let node = &account["edges"][0]["node"];
        assert_eq!(node["Name"]["value"], "Acme Corp");
        assert_eq!(node["Industry"]["displayValue"], "Technology");
    }
}
