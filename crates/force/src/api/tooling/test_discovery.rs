//! Tooling API Test Discovery resource (`GET /tooling/tests/`, v65.0+).
//!
//! Lists Apex and automated flow tests. In v68.0 `testLevel` replaces
//! `showAllMethods`. The client checks this rule before it sends a request.

use crate::api::rest_operation::{RestOperation, resolve_next_records_url};
use crate::error::ForceError;
use crate::types::ApiVersion;
use serde::{Deserialize, Serialize};

/// First version that accepts `testLevel` and rejects `showAllMethods`.
const TEST_LEVEL_MIN_MAJOR: u16 = 68;

/// Largest `pageSize` that the server accepts.
const MAX_PAGE_SIZE: u32 = 10_000;

/// Test category filter (v66.0+).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TestCategory {
    /// Apex test classes only.
    Apex,
    /// Automated flow tests only.
    Flow,
}

impl TestCategory {
    /// Returns the wire value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Apex => "apex",
            Self::Flow => "flow",
        }
    }
}

/// Test level filter (v68.0+).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiscoveryTestLevel {
    /// All tests, including managed-package tests. Server default.
    RunAllTestsInOrg,
    /// Tests in the org namespace plus flow tests.
    RunLocalTests,
}

impl DiscoveryTestLevel {
    /// Returns the wire value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RunAllTestsInOrg => "RunAllTestsInOrg",
            Self::RunLocalTests => "RunLocalTests",
        }
    }
}

/// Query for the Test Discovery resource.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TestDiscoveryRequest {
    /// Category filter.
    pub category: Option<TestCategory>,
    /// Test level filter. Needs v68.0+.
    pub test_level: Option<DiscoveryTestLevel>,
    /// Deprecated. Only v67.0 and earlier.
    pub show_all_methods: Option<bool>,
    /// Namespace filter.
    pub namespace_prefix: Option<String>,
    /// Classes per page (1 to 10000). Server default is 1000.
    pub page_size: Option<u32>,
}

impl TestDiscoveryRequest {
    /// Checks the request against the configured API version.
    ///
    /// `api_version` is the `ClientConfig.api_version` value. The `latest`
    /// alias counts as v68.0 or later. An unknown string skips the version
    /// rules.
    ///
    /// # Errors
    ///
    /// Returns [`ForceError::InvalidInput`] if `test_level` is set below v68.0,
    /// if `show_all_methods` is set on v68.0 or later, or if `page_size` is
    /// out of range.
    pub fn validate(&self, api_version: &str) -> crate::error::Result<()> {
        // `latest` is v68.0+. An unknown string skips the version rules; the
        // server rejects it.
        let is_v68_or_later = if ApiVersion::is_latest_alias(api_version) {
            Some(true)
        } else {
            ApiVersion::config_major(api_version).map(|m| m >= TEST_LEVEL_MIN_MAJOR)
        };
        if self.test_level.is_some() && is_v68_or_later == Some(false) {
            return Err(ForceError::InvalidInput(format!(
                "testLevel needs API v68.0 or later; configured {api_version}"
            )));
        }
        if self.show_all_methods.is_some() && is_v68_or_later == Some(true) {
            return Err(ForceError::InvalidInput(format!(
                "showAllMethods is only valid up to API v67.0; use test_level on {api_version}"
            )));
        }
        if let Some(size) = self.page_size
            && !(1..=MAX_PAGE_SIZE).contains(&size)
        {
            return Err(ForceError::InvalidInput(format!(
                "page_size must be 1 to {MAX_PAGE_SIZE}; got {size}"
            )));
        }
        Ok(())
    }

    /// Returns the query parameters.
    fn query_params(&self) -> Vec<(&'static str, String)> {
        let mut params = Vec::new();
        if let Some(c) = self.category {
            params.push(("category", c.as_str().to_owned()));
        }
        if let Some(l) = self.test_level {
            params.push(("testLevel", l.as_str().to_owned()));
        }
        if let Some(s) = self.show_all_methods {
            params.push(("showAllMethods", s.to_string()));
        }
        if let Some(ns) = &self.namespace_prefix {
            params.push(("namespacePrefix", ns.clone()));
        }
        if let Some(size) = self.page_size {
            params.push(("pageSize", size.to_string()));
        }
        params
    }
}

/// Test method reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestMethodRef {
    /// Method name. For flow tests: `FlowName_TestName`.
    pub name: String,
}

/// Test class in a discovery result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestClassInfo {
    /// Class ID. Empty for some flow tests.
    #[serde(default)]
    pub id: String,
    /// Class name. For a flow test, the flow API name.
    pub name: String,
    /// Namespace. Flow tests use `FlowTesting`.
    #[serde(default)]
    pub namespace_prefix: String,
    /// Test methods.
    #[serde(default)]
    pub test_methods: Vec<TestMethodRef>,
}

/// One page of Test Discovery results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestDiscoveryResult {
    /// Test classes on this page.
    #[serde(default)]
    pub apex_test_classes: Vec<TestClassInfo>,
    /// Total classes across all pages.
    #[serde(default)]
    pub size: u64,
    /// Next page URL. `None` on the last page.
    #[serde(default)]
    pub next_records_url: Option<String>,
    /// MD5 hash of the result set. A change between pages means the tests changed.
    #[serde(default)]
    pub test_set_signature: String,
    /// Server message, for example an input warning.
    #[serde(default)]
    pub message: Option<String>,
}

/// Header that asks for raw (not HTML-encoded) output.
const ENTITY_ENCODING_HEADER: &str = "X-Chatter-Entity-Encoding";

impl<A: crate::auth::Authenticator> super::ToolingHandler<A> {
    /// Lists Apex and flow tests (Test Discovery API).
    ///
    /// `GET /tooling/tests/`. Needs API v65.0+. See
    /// [`TestDiscoveryRequest::validate`] for version rules.
    ///
    /// # Errors
    ///
    /// Returns an error if validation fails, the request fails, or the
    /// response cannot be decoded.
    pub async fn discover_tests(
        &self,
        request: &TestDiscoveryRequest,
    ) -> crate::error::Result<TestDiscoveryResult> {
        request.validate(&self.session().config.api_version)?;
        let url = self.session().resolve_url("tooling/tests/").await?;
        let http_request = self
            .session()
            .get(&url)
            .query(&request.query_params())
            .header(ENTITY_ENCODING_HEADER, "false")
            .build()
            .map_err(crate::error::HttpError::from)?;
        self.session()
            .send_request_and_decode(http_request, "Test discovery request failed")
            .await
    }

    /// Gets the next page of a Test Discovery result.
    ///
    /// `next_records_url` is [`TestDiscoveryResult::next_records_url`]. The URL
    /// must have the same origin as the org instance.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is not safe, the request fails, or the
    /// response cannot be decoded.
    pub async fn discover_tests_next(
        &self,
        next_records_url: &str,
    ) -> crate::error::Result<TestDiscoveryResult> {
        let instance_url = self.session().instance_url().await?;
        let url = resolve_next_records_url(&instance_url, next_records_url)?;
        let http_request = self
            .session()
            .get(&url)
            .header(ENTITY_ENCODING_HEADER, "false")
            .build()
            .map_err(crate::error::HttpError::from)?;
        self.session()
            .send_request_and_decode(http_request, "Test discovery request failed")
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::builder;
    use crate::config::ClientConfig;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;
    use serde_json::json;
    use wiremock::matchers::{header, method, path, query_param, query_param_is_missing};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn client_for(
        server: &MockServer,
        version: &str,
    ) -> crate::client::ForceClient<MockAuthenticator> {
        builder()
            .authenticate(MockAuthenticator::new("t", &server.uri()))
            .config(ClientConfig {
                api_version: version.to_owned(),
                ..ClientConfig::default()
            })
            .build()
            .await
            .must()
    }

    fn page_body(next: Option<&str>) -> serde_json::Value {
        json!({
            "apexTestClasses": [{
                "id": "01pxx0000004UVl",
                "name": "BAAdditionTest",
                "namespacePrefix": "my_namespace",
                "testMethods": [{"name": "testNegativeAddition"}, {"name": "testSimpleAddition"}]
            }],
            "message": null,
            "nextRecordsUrl": next,
            "size": 10,
            "testSetSignature": "91a678b54197669171f11eb824d6765a"
        })
    }

    #[test]
    fn test_validate_version_rules() {
        let level = TestDiscoveryRequest {
            test_level: Some(DiscoveryTestLevel::RunLocalTests),
            ..Default::default()
        };
        assert!(level.validate("v68.0").is_ok());
        assert!(level.validate("latest").is_ok());
        assert!(level.validate("v67.0").is_err());

        let show = TestDiscoveryRequest {
            show_all_methods: Some(true),
            ..Default::default()
        };
        assert!(show.validate("v67.0").is_ok());
        assert!(show.validate("v68.0").is_err());
        assert!(show.validate("latest").is_err());
    }

    #[test]
    fn test_validate_unknown_version_passes_through() {
        // Only `latest` counts as v68+. Other strings are the server's problem.
        for v in ["garbage", "68.0", "v68"] {
            let level = TestDiscoveryRequest {
                test_level: Some(DiscoveryTestLevel::RunAllTestsInOrg),
                ..Default::default()
            };
            assert!(level.validate(v).is_ok(), "{v}");
            let show = TestDiscoveryRequest {
                show_all_methods: Some(true),
                ..Default::default()
            };
            assert!(show.validate(v).is_ok(), "{v}");
        }
    }

    #[test]
    fn test_validate_show_all_methods_false_still_rejected_on_v68() {
        let req = TestDiscoveryRequest {
            show_all_methods: Some(false),
            ..Default::default()
        };
        assert!(req.validate("v68.0").is_err());
    }

    #[test]
    fn test_decode_flow_test_without_id() {
        let page: TestDiscoveryResult = serde_json::from_value(json!({
            "apexTestClasses": [{"name": "MyFlow", "namespacePrefix": "FlowTesting",
                "testMethods": [{"name": "MyFlow_T1"}]}],
            "size": 1, "nextRecordsUrl": null, "testSetSignature": "", "message": null
        }))
        .must();
        assert_eq!(page.apex_test_classes[0].id, "");
    }

    #[tokio::test]
    async fn test_discover_tests_rejects_test_level_on_v67_without_request() {
        let server = MockServer::start().await;
        let client = client_for(&server, "v67.0").await;
        let req = TestDiscoveryRequest {
            test_level: Some(DiscoveryTestLevel::RunLocalTests),
            ..Default::default()
        };
        let err = client.tooling().discover_tests(&req).await.err().must();
        assert!(matches!(err, ForceError::InvalidInput(_)), "{err:?}");
        assert!(server.received_requests().await.must().is_empty());
    }

    #[test]
    fn test_validate_page_size_bounds() {
        let req = |n| TestDiscoveryRequest {
            page_size: Some(n),
            ..Default::default()
        };
        assert!(req(1).validate("v68.0").is_ok());
        assert!(req(10_000).validate("v68.0").is_ok());
        assert!(req(0).validate("v68.0").is_err());
        assert!(req(10_001).validate("v68.0").is_err());
    }

    #[tokio::test]
    async fn test_discover_tests_sends_params_and_header() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/services/data/v68.0/tooling/tests/"))
            .and(header("X-Chatter-Entity-Encoding", "false"))
            .and(query_param("category", "apex"))
            .and(query_param("testLevel", "RunLocalTests"))
            .and(query_param("namespacePrefix", "my_namespace"))
            .and(query_param("pageSize", "2"))
            .and(query_param_is_missing("showAllMethods"))
            .respond_with(ResponseTemplate::new(200).set_body_json(page_body(None)))
            .expect(1)
            .mount(&server)
            .await;
        let client = client_for(&server, "v68.0").await;

        let req = TestDiscoveryRequest {
            category: Some(TestCategory::Apex),
            test_level: Some(DiscoveryTestLevel::RunLocalTests),
            namespace_prefix: Some("my_namespace".into()),
            page_size: Some(2),
            ..Default::default()
        };
        let out = client.tooling().discover_tests(&req).await.must();
        assert_eq!(out.size, 10);
        assert_eq!(out.apex_test_classes[0].test_methods.len(), 2);
        assert!(out.next_records_url.is_none());
    }

    #[tokio::test]
    async fn test_discover_tests_rejects_show_all_methods_on_v68_without_request() {
        let server = MockServer::start().await;
        // No mock: a sent request would fail with 404, not InvalidInput.
        let client = client_for(&server, "v68.0").await;
        let req = TestDiscoveryRequest {
            show_all_methods: Some(true),
            ..Default::default()
        };
        let err = client.tooling().discover_tests(&req).await.err().must();
        assert!(matches!(err, ForceError::InvalidInput(_)), "{err:?}");
        assert!(server.received_requests().await.must().is_empty());
    }

    #[tokio::test]
    async fn test_discover_tests_v67_show_all_methods() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/tooling/tests/"))
            .and(query_param("showAllMethods", "true"))
            .respond_with(ResponseTemplate::new(200).set_body_json(page_body(None)))
            .expect(1)
            .mount(&server)
            .await;
        let client = client_for(&server, "v67.0").await;
        let req = TestDiscoveryRequest {
            show_all_methods: Some(true),
            ..Default::default()
        };
        client.tooling().discover_tests(&req).await.must();
    }

    #[tokio::test]
    async fn test_discover_tests_next_page() {
        let server = MockServer::start().await;
        let next = "/services/data/v68.0/tooling/tests?pageSize=2&nextRecord=my_namespace.BAComparisonTest";
        Mock::given(method("GET"))
            .and(path("/services/data/v68.0/tooling/tests"))
            .and(query_param("nextRecord", "my_namespace.BAComparisonTest"))
            .and(header("X-Chatter-Entity-Encoding", "false"))
            .respond_with(ResponseTemplate::new(200).set_body_json(page_body(None)))
            .expect(1)
            .mount(&server)
            .await;
        let client = client_for(&server, "v68.0").await;

        let out = client.tooling().discover_tests_next(next).await.must();
        assert_eq!(out.apex_test_classes.len(), 1);
    }

    #[tokio::test]
    async fn test_discover_tests_next_rejects_foreign_origin() {
        let server = MockServer::start().await;
        let client = client_for(&server, "v68.0").await;
        let err = client
            .tooling()
            .discover_tests_next("https://evil.example.com/services/data/v68.0/tooling/tests")
            .await
            .err()
            .must();
        assert!(matches!(err, ForceError::InvalidInput(_)), "{err:?}");
    }
}
