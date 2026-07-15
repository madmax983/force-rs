//! Token fetch, caching, proactive re-auth, and per-MID override behavior.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::TestHarness;
use wiremock::matchers::{body_json_string, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Counts how many requests hit the `/v2/token` endpoint.
async fn token_request_count(server: &MockServer) -> usize {
    server
        .received_requests()
        .await
        .unwrap_or_default()
        .iter()
        .filter(|r| r.url.path() == "/v2/token")
        .count()
}

#[tokio::test]
async fn token_is_fetched_once_and_cached() {
    let harness = TestHarness::start().await;
    let client = harness.client();

    // A generic REST responder so the two calls succeed.
    Mock::given(method("GET"))
        .and(path("/interaction/v1/interactions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "items": [] })))
        .mount(&harness.server)
        .await;

    client.journeys().list().await.unwrap();
    client.journeys().list().await.unwrap();

    assert_eq!(
        token_request_count(&harness.server).await,
        1,
        "second REST call should reuse the cached token"
    );
}

#[tokio::test]
async fn proactively_reauthenticates_after_soft_expiry() {
    // Token lifetime of 30s is inside the 60s soft-expiry buffer, so every call
    // should re-authenticate.
    let harness = TestHarness::start_with_token(30, None).await;
    let client = harness.client();

    Mock::given(method("GET"))
        .and(path("/interaction/v1/interactions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "items": [] })))
        .mount(&harness.server)
        .await;

    client.journeys().list().await.unwrap();
    client.journeys().list().await.unwrap();

    assert_eq!(
        token_request_count(&harness.server).await,
        2,
        "a soft-expired token must be re-fetched on the next call"
    );
}

#[tokio::test]
async fn per_mid_override_fetches_distinct_token() {
    let harness = TestHarness::start().await;
    let client = harness.client();

    Mock::given(method("GET"))
        .and(path("/interaction/v1/interactions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "items": [] })))
        .mount(&harness.server)
        .await;

    // Default business unit, then an explicit MID override.
    client.journeys().list().await.unwrap();
    client
        .journeys()
        .for_business_unit("999888")
        .list()
        .await
        .unwrap();

    assert_eq!(
        token_request_count(&harness.server).await,
        2,
        "a distinct MID must fetch and cache its own token"
    );

    // The override token is cached independently: no third token request.
    client
        .journeys()
        .for_business_unit("999888")
        .list()
        .await
        .unwrap();
    assert_eq!(token_request_count(&harness.server).await, 2);
}

#[tokio::test]
async fn token_request_body_carries_client_credentials() {
    let server = MockServer::start().await;
    let base = format!("{}/", server.uri());
    let expected_body = serde_json::json!({
        "grant_type": "client_credentials",
        "client_id": "test-client-id",
        "client_secret": "test-client-secret",
        "account_id": "555",
    })
    .to_string();

    Mock::given(method("POST"))
        .and(path("/v2/token"))
        .and(body_json_string(expected_body))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "mock-access-token",
            "token_type": "Bearer",
            "expires_in": 1080,
            "rest_instance_url": base,
            "soap_instance_url": base,
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = force_marketingcloud::MarketingCloudClient::builder()
        .tenant_subdomain("test-sub")
        .client_credentials("test-client-id", "test-client-secret")
        .account_id("555")
        .auth_url(format!("{}/v2/token", server.uri()))
        .build()
        .unwrap();

    Mock::given(method("GET"))
        .and(path("/interaction/v1/interactions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "items": [] })))
        .mount(&server)
        .await;

    client.journeys().list().await.unwrap();
    // `expect(1)` on the token mock is verified when `server` drops.
}
