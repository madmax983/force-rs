//! Shared test harness: a wiremock server that mocks the `v2/token` endpoint and
//! points `rest_instance_url` back at itself so REST calls hit the same mock.

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use force_marketingcloud::MarketingCloudClient;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A running mock server plus a client wired to it.
pub struct TestHarness {
    /// The mock server; kept alive for the duration of a test.
    pub server: MockServer,
}

impl TestHarness {
    /// Starts a mock server and mounts a default `/v2/token` responder whose
    /// `rest_instance_url` points back at the same server.
    pub async fn start() -> Self {
        let server = MockServer::start().await;
        mount_token(&server, 1080, None).await;
        Self { server }
    }

    /// Starts a mock server whose issued token has the given lifetime in seconds
    /// and whose `/v2/token` responder may be `expect`ed a specific number of times.
    pub async fn start_with_token(expires_in: u64, expect: Option<u64>) -> Self {
        let server = MockServer::start().await;
        mount_token(&server, expires_in, expect).await;
        Self { server }
    }

    /// The base URI of the mock server (no trailing slash).
    pub fn uri(&self) -> String {
        self.server.uri()
    }

    /// Builds a client pointed at the mock server for both auth and REST.
    pub fn client(&self) -> MarketingCloudClient {
        MarketingCloudClient::builder()
            .tenant_subdomain("test-sub")
            .client_credentials("test-client-id", "test-client-secret")
            .auth_url(format!("{}/v2/token", self.server.uri()))
            .build()
            .expect("client builds")
    }
}

/// Mounts a `/v2/token` responder returning a token whose REST base points at
/// the same mock server.
pub async fn mount_token(server: &MockServer, expires_in: u64, expect: Option<u64>) {
    let base = format!("{}/", server.uri());
    let body = serde_json::json!({
        "access_token": "mock-access-token",
        "token_type": "Bearer",
        "expires_in": expires_in,
        "scope": "email_send",
        "soap_instance_url": base,
        "rest_instance_url": base,
    });
    let mut mock = Mock::given(method("POST"))
        .and(path("/v2/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body));
    if let Some(times) = expect {
        mock = mock.expect(times);
    }
    mock.mount(server).await;
}
