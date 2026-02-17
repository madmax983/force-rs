#![cfg(feature = "composite")]
//! Integration tests for Composite API.

use async_trait::async_trait;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::Result;
use serde_json::json;
use wiremock::matchers::{bearer_token, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// Mock authenticator
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
    async fn authenticate(&self) -> Result<AccessToken> {
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

    async fn refresh(&self) -> Result<AccessToken> {
        self.authenticate().await
    }
}

#[tokio::test]
async fn test_composite_batch_execution() {
    let mock_server = MockServer::start().await;
    let auth = MockAuthenticator::new("test_token", &mock_server.uri());

    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .expect("failed to build client");

    // Mock successful batch response
    let response_body = json!({
        "hasErrors": false,
        "results": [
            {
                "statusCode": 200,
                "result": {
                    "Id": "001000000000001AAA",
                    "Name": "Acme"
                }
            },
            {
                "statusCode": 201,
                "result": {
                    "id": "003000000000001AAA",
                    "success": true,
                    "errors": []
                }
            }
        ]
    });

    Mock::given(method("POST"))
        .and(path("/services/data/v60.0/composite/batch"))
        .and(bearer_token("test_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    // Execute batch
    let batch_response = client
        .composite()
        .batch()
        .get("Account", "001000000000001AAA")
        .post("Contact", json!({"LastName": "Doe"}))
        .execute()
        .await
        .expect("batch execution failed");

    assert!(!batch_response.has_errors);
    assert_eq!(batch_response.results.len(), 2);

    // Check first result (GET Account)
    let res1 = &batch_response.results[0];
    assert_eq!(res1.status_code, 200);
    let account = res1.result.as_ref().unwrap();
    assert_eq!(account["Name"], "Acme");

    // Check second result (POST Contact)
    let res2 = &batch_response.results[1];
    assert_eq!(res2.status_code, 201);
    let contact_res = res2.result.as_ref().unwrap();
    assert_eq!(contact_res["success"], true);
}

#[tokio::test]
async fn test_composite_batch_failure_handling() {
    let mock_server = MockServer::start().await;
    let auth = MockAuthenticator::new("test_token", &mock_server.uri());

    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .expect("failed to build client");

    // Mock partial failure response
    let response_body = json!({
        "hasErrors": true,
        "results": [
            {
                "statusCode": 404,
                "result": [
                    {
                        "errorCode": "NOT_FOUND",
                        "message": "The requested resource does not exist"
                    }
                ]
            }
        ]
    });

    Mock::given(method("POST"))
        .and(path("/services/data/v60.0/composite/batch"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    // Execute batch
    let batch_response = client
        .composite()
        .batch()
        .get("Account", "invalid_id")
        .execute()
        .await
        .expect("batch execution failed");

    assert!(batch_response.has_errors);
    assert_eq!(batch_response.results.len(), 1);

    let res1 = &batch_response.results[0];
    assert_eq!(res1.status_code, 404);
}
