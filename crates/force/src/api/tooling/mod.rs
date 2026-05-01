//! Tooling API handler for Salesforce.
//!
//! The Tooling API provides REST-style access to metadata types like `ApexClass`,
//! `ApexTrigger`, and others. It supports the same CRUD, Query, and Describe
//! operations as the REST API, plus Tooling-specific endpoints.
//!
//! # URL Routing
//!
//! All requests are routed under the `/tooling/` prefix:
//! - `tooling/sobjects/ApexClass` (CRUD)
//! - `tooling/query?q=SELECT ...` (SOQL)
//! - `tooling/sobjects` (Describe Global)

pub(crate) mod completions;
pub(crate) mod execute_anonymous;
pub(crate) mod run_tests;

pub use completions::{CompletionsResult, CompletionsType};
pub use execute_anonymous::ExecuteAnonymousResult;
pub use run_tests::{
    CodeCoverageResult, RunTestsRequest, RunTestsResult, TestFailure, TestItem, TestSuccess,
};

use crate::api::rest_operation::RestOperation;
use std::sync::Arc;

/// Tooling API handler for Salesforce development operations.
///
/// Provides access to all Tooling API functionality including:
/// - CRUD operations on tooling objects (`ApexClass`, `ApexTrigger`, etc.)
/// - SOQL queries against tooling objects
/// - Metadata describe operations
/// - Execute anonymous Apex
/// - Run Apex tests (synchronous and asynchronous)
///
/// The handler is obtained from a [`ForceClient`](crate::client::ForceClient)
/// and shares its authentication and configuration.
///
/// # Examples
///
/// ```ignore
/// use force::api::rest_operation::RestOperation;
///
/// let client = builder().authenticate(auth).build().await?;
/// let result = client.tooling()
///     .query::<serde_json::Value>("SELECT Id, Name FROM ApexClass")
///     .await?;
/// ```
#[derive(Debug)]
pub struct ToolingHandler<A: crate::auth::Authenticator> {
    inner: Arc<crate::session::Session<A>>,
}

impl<A: crate::auth::Authenticator> Clone for ToolingHandler<A> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<A: crate::auth::Authenticator> ToolingHandler<A> {
    /// Creates a new Tooling API handler for the given session state.
    ///
    /// # Arguments
    ///
    /// * `inner` - The shared session state containing the HTTP client,
    ///   token manager, and configuration.
    #[must_use]
    pub(crate) fn new(inner: Arc<crate::session::Session<A>>) -> Self {
        Self { inner }
    }
}

impl<A: crate::auth::Authenticator> RestOperation<A> for ToolingHandler<A> {
    fn session(&self) -> &Arc<crate::session::Session<A>> {
        &self.inner
    }

    fn path_prefix(&self) -> &'static str {
        "tooling"
    }
}

#[cfg(test)]
mod tests {
    use crate::api::rest_operation::RestOperation;
    use crate::client::{ForceClient, builder};
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::{Must, MustMsg};
    use crate::types::SalesforceId;

    use serde_json::json;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn create_test_client() -> ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", "https://test.salesforce.com");
        builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("failed to create test client")
    }

    #[tokio::test]
    async fn test_tooling_handler_construction() {
        let client = create_test_client().await;
        let _handler = client.tooling();
    }

    #[tokio::test]
    async fn test_tooling_handler_is_cloneable() {
        let client = create_test_client().await;
        let h1 = client.tooling();
        let _h2 = Clone::clone(&h1);
    }

    #[tokio::test]
    async fn test_tooling_path_prefix() {
        let client = create_test_client().await;
        let handler = client.tooling();
        assert_eq!(handler.path_prefix(), "tooling");
    }

    #[tokio::test]
    async fn test_tooling_handler_debug() {
        let client = create_test_client().await;
        let handler = client.tooling();
        let debug = format!("{:?}", handler);
        assert!(!debug.is_empty());
    }

    #[tokio::test]
    async fn test_tooling_resolve_api_path_sobjects() {
        let client = create_test_client().await;
        let handler = client.tooling();
        assert_eq!(
            handler.resolve_api_path("sobjects/ApexClass"),
            "tooling/sobjects/ApexClass"
        );
    }

    #[tokio::test]
    async fn test_tooling_resolve_api_path_query() {
        let client = create_test_client().await;
        let handler = client.tooling();
        assert_eq!(handler.resolve_api_path("query"), "tooling/query");
    }

    #[tokio::test]
    async fn test_tooling_resolve_api_path_describe() {
        let client = create_test_client().await;
        let handler = client.tooling();
        assert_eq!(handler.resolve_api_path("sobjects"), "tooling/sobjects");
    }

    // === Wiremock smoke tests: verify Tooling API URL wiring ===
    //
    // These tests verify that the ToolingHandler's trait default
    // implementations hit the correct `/tooling/` prefixed URLs.
    // The CRUD/query/describe logic itself is already proven by the
    // REST API tests; here we only check URL routing.

    #[tokio::test]
    async fn test_tooling_create_hits_tooling_url() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/tooling/sobjects/ApexClass"))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "id": "01p000000000001AAA",
                "success": true,
                "errors": []
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let tooling = client.tooling();
        let response = tooling
            .create("ApexClass", &json!({"Body": "public class MyClass {}"}))
            .await
            .must();

        assert!(response.is_success());
        assert_eq!(response.id.must().as_str(), "01p000000000001AAA");
    }

    #[tokio::test]
    async fn test_tooling_get_hits_tooling_url() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/tooling/sobjects/ApexClass/01p000000000001AAA",
            ))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "Id": "01p000000000001AAA",
                "Name": "MyClass"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let tooling = client.tooling();
        let id = SalesforceId::new("01p000000000001AAA").must();
        let record = tooling.get("ApexClass", &id).await.must();

        assert_eq!(record["Id"], "01p000000000001AAA");
        assert_eq!(record["Name"], "MyClass");
    }

    #[tokio::test]
    async fn test_tooling_query_hits_tooling_url() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/tooling/query"))
            .and(query_param("q", "SELECT Id FROM ApexClass"))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 1,
                "done": true,
                "records": [{"Id": "01p000000000001AAA"}]
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let tooling = client.tooling();
        let result = tooling
            .query::<serde_json::Value>("SELECT Id FROM ApexClass")
            .await
            .must();

        assert_eq!(result.total_size, 1);
        assert!(result.done);
        assert_eq!(result.records.len(), 1);
    }

    #[tokio::test]
    async fn test_tooling_describe_hits_tooling_url() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/tooling/sobjects/ApexClass/describe",
            ))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "activateable": false,
                "createable": true,
                "custom": false,
                "customSetting": false,
                "deletable": true,
                "deprecatedAndHidden": false,
                "feedEnabled": false,
                "hasSubtypes": false,
                "isSubtype": false,
                "keyPrefix": "01p",
                "label": "Apex Class",
                "labelPlural": "Apex Classes",
                "layoutable": false,
                "mergeable": false,
                "mruEnabled": false,
                "name": "ApexClass",
                "queryable": true,
                "replicateable": false,
                "retrieveable": true,
                "searchable": true,
                "triggerable": false,
                "undeletable": true,
                "updateable": true,
                "urls": {
                    "sobject": "/services/data/v60.0/tooling/sobjects/ApexClass"
                },
                "fields": [],
                "childRelationships": [],
                "recordTypeInfos": []
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let tooling = client.tooling();
        let describe = tooling.describe("ApexClass").await.must();

        assert_eq!(describe.name, "ApexClass");
        assert_eq!(describe.label, "Apex Class");
        assert_eq!(describe.key_prefix, Some("01p".to_string()));
    }

    #[tokio::test]
    async fn test_tooling_describe_global_hits_tooling_url() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/tooling/sobjects"))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "encoding": "UTF-8",
                "maxBatchSize": 200,
                "sobjects": []
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let tooling = client.tooling();
        let global = tooling.describe_global().await.must();

        assert_eq!(global.encoding, "UTF-8");
        assert_eq!(global.max_batch_size, 200);
        assert!(global.sobjects.is_empty());
    }
}
