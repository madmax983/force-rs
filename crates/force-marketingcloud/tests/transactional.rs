//! Transactional Messaging happy-path and error-path tests.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::TestHarness;
use force_marketingcloud::{MarketingCloudError, Recipient, SendEmailRequest, SendSmsRequest};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn send_email_posts_expected_body_and_bearer_header() {
    let harness = TestHarness::start().await;
    let client = harness.client();

    let expected_body = json!({
        "definitionKey": "transactional_welcome",
        "recipient": {
            "contactKey": "contact-001",
            "to": "jane@example.com",
            "attributes": { "FirstName": "Jane" }
        }
    });

    Mock::given(method("POST"))
        .and(path("/messaging/v1/email/messages/MSG-KEY-123"))
        .and(header("authorization", "Bearer mock-access-token"))
        .and(header("content-type", "application/json"))
        .and(body_json(expected_body))
        .respond_with(ResponseTemplate::new(202).set_body_json(json!({
            "requestId": "req-1",
            "responses": [ { "messageKey": "MSG-KEY-123", "status": "queued" } ]
        })))
        .expect(1)
        .mount(&harness.server)
        .await;

    let mut attributes = serde_json::Map::new();
    attributes.insert("FirstName".to_string(), json!("Jane"));
    let request = SendEmailRequest::new(
        "transactional_welcome",
        Recipient::new("contact-001", "jane@example.com").with_attributes(attributes),
    );

    let response = client
        .transactional()
        .send_email("MSG-KEY-123", &request)
        .await
        .unwrap();

    assert_eq!(response.request_id.as_deref(), Some("req-1"));
    let items = response.responses.unwrap();
    assert_eq!(items[0].message_key.as_deref(), Some("MSG-KEY-123"));
    assert_eq!(items[0].status.as_deref(), Some("queued"));
}

#[tokio::test]
async fn send_sms_posts_to_sms_path() {
    let harness = TestHarness::start().await;
    let client = harness.client();

    Mock::given(method("POST"))
        .and(path("/messaging/v1/sms/messages/SMS-1"))
        .respond_with(ResponseTemplate::new(202).set_body_json(json!({ "requestId": "sms-req" })))
        .expect(1)
        .mount(&harness.server)
        .await;

    let request = SendSmsRequest::new(
        "sms_otp_definition",
        Recipient::new("contact-001", "+15551234567"),
    );
    let response = client
        .transactional()
        .send_sms("SMS-1", &request)
        .await
        .unwrap();
    assert_eq!(response.request_id.as_deref(), Some("sms-req"));
}

#[tokio::test]
async fn email_status_returns_raw_value() {
    let harness = TestHarness::start().await;
    let client = harness.client();

    Mock::given(method("GET"))
        .and(path("/messaging/v1/email/messages/MSG-KEY-9"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "status": "sent" })))
        .mount(&harness.server)
        .await;

    let status = client
        .transactional()
        .email_status("MSG-KEY-9")
        .await
        .unwrap();
    assert_eq!(status["status"], json!("sent"));
}

#[tokio::test]
async fn send_email_maps_error_envelope() {
    let harness = TestHarness::start().await;
    let client = harness.client();

    Mock::given(method("POST"))
        .and(path("/messaging/v1/email/messages/BAD"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "requestId": "err-req",
            "errors": [ {
                "responseCode": 400,
                "errorCode": "validation.email.subject_empty",
                "message": "subject is empty"
            } ]
        })))
        .mount(&harness.server)
        .await;

    let request = SendEmailRequest::new("def", Recipient::new("c", "x@example.com"));
    let err = client
        .transactional()
        .send_email("BAD", &request)
        .await
        .unwrap_err();

    match err {
        MarketingCloudError::Api {
            status,
            message,
            error_code,
            request_id,
            ..
        } => {
            assert_eq!(status, 400);
            assert_eq!(message, "subject is empty");
            assert_eq!(
                error_code.as_deref(),
                Some("validation.email.subject_empty")
            );
            assert_eq!(request_id.as_deref(), Some("err-req"));
        }
        other => panic!("expected Api error, got {other:?}"),
    }
}
