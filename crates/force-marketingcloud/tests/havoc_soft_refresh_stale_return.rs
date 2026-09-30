//! Wait, marketing cloud has integration tests via client.
//! Let's write an integration test.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use force_marketingcloud::MarketingCloudClient;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_havoc_marketingcloud_soft_refresh_stale_return() {
    let server = MockServer::start().await;
    let base = format!("{}/", server.uri());

    // First call returns a soft-expired token
    Mock::given(method("POST"))
        .and(path("/v2/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "soft-expired-token",
            "token_type": "Bearer",
            "expires_in": 30, // Under 60s soft buffer
            "rest_instance_url": base,
            "soap_instance_url": base,
        })))
        .expect(1) // we only mock one successful call to simulate failure on next
        .mount(&server)
        .await;

    let client = MarketingCloudClient::builder()
        .tenant_subdomain("test-sub")
        .client_credentials("test-client-id", "test-client-secret")
        .auth_url(format!("{}/v2/token", server.uri()))
        .build()
        .unwrap();

    // Mock API
    Mock::given(method("GET"))
        .and(path("/interaction/v1/interactions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "items": [] })))
        .mount(&server)
        .await;

    // Call 1: Fetches the soft-expired token and succeeds
    client.journeys().list().await.unwrap();

    // Call 2: Tries to refresh, but there's no mock for the 2nd time... wait, let's mock it as a failure
    server.reset().await;
    Mock::given(method("POST"))
        .and(path("/v2/token"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/interaction/v1/interactions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "items": [] })))
        .mount(&server)
        .await;

    // Call 2: Tries to refresh, fails!
    // It SHOULD use the still-valid soft-expired token and succeed.
    let result = client.journeys().list().await;

    assert!(
        result.is_ok(),
        "👺 Havoc: Soft refresh failure dropped the still-valid token and returned an error: {result:?}"
    );
}
