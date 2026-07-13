//! Error-envelope parsing for both documented shapes plus non-JSON fallback.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::TestHarness;
use force_marketingcloud::MarketingCloudError;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn flat_error_envelope_is_parsed() {
    let harness = TestHarness::start().await;
    let client = harness.client();

    Mock::given(method("GET"))
        .and(path("/asset/v1/content/assets/1"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "message": "Asset not found",
            "errorcode": 118_001,
            "documentation": "https://developer.salesforce.com/x"
        })))
        .mount(&harness.server)
        .await;

    let err = client.assets().get(1).await.unwrap_err();
    match err {
        MarketingCloudError::Api {
            status,
            message,
            error_code,
            documentation,
            ..
        } => {
            assert_eq!(status, 404);
            assert_eq!(message, "Asset not found");
            assert_eq!(error_code.as_deref(), Some("118001"));
            assert_eq!(
                documentation.as_deref(),
                Some("https://developer.salesforce.com/x")
            );
        }
        other => panic!("expected Api error, got {other:?}"),
    }
}

#[tokio::test]
async fn rich_error_envelope_is_parsed() {
    let harness = TestHarness::start().await;
    let client = harness.client();

    Mock::given(method("GET"))
        .and(path("/asset/v1/content/assets/2"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "requestId": "rid-77",
            "errorcode": 30000,
            "message": "top-level",
            "errors": [ {
                "id": "err-1",
                "responseCode": 400,
                "errorCode": "validation.generic",
                "message": "nested detail",
                "details": []
            } ]
        })))
        .mount(&harness.server)
        .await;

    let err = client.assets().get(2).await.unwrap_err();
    match err {
        MarketingCloudError::Api {
            message,
            error_code,
            request_id,
            ..
        } => {
            assert_eq!(message, "nested detail");
            assert_eq!(error_code.as_deref(), Some("validation.generic"));
            assert_eq!(request_id.as_deref(), Some("rid-77"));
        }
        other => panic!("expected Api error, got {other:?}"),
    }
}

#[tokio::test]
async fn non_json_error_body_falls_back_to_raw() {
    let harness = TestHarness::start().await;
    let client = harness.client();

    Mock::given(method("GET"))
        .and(path("/asset/v1/content/assets/3"))
        .respond_with(ResponseTemplate::new(503).set_body_string("Service Unavailable"))
        .mount(&harness.server)
        .await;

    let err = client.assets().get(3).await.unwrap_err();
    match err {
        MarketingCloudError::Api {
            status, message, ..
        } => {
            assert_eq!(status, 503);
            assert_eq!(message, "Service Unavailable");
        }
        other => panic!("expected Api error, got {other:?}"),
    }
}
