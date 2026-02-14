#![cfg(feature = "nova")]
#![allow(missing_docs)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use async_trait::async_trait;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::Result;
use force::experimental::composite::{BatchRequest, SubRequest};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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
async fn test_composite_batch_success() {
    let mock_server = MockServer::start().await;
    let auth = MockAuthenticator::new("test_token", &mock_server.uri());
    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .expect("Failed to build client");

    let expected_req = json!({
        "batchRequests": [
            {
                "method": "GET",
                "url": "sobjects/Account/123",
                "referenceId": "ref1"
            }
        ],
        "haltOnError": false
    });

    let mock_resp = json!({
        "hasErrors": false,
        "results": [
            {
                "statusCode": 200,
                "result": { "Id": "123", "Name": "Test Account" }
            }
        ]
    });

    Mock::given(method("POST"))
        .and(path("/services/data/v60.0/composite/batch"))
        .and(header("Authorization", "Bearer test_token"))
        .and(body_json(expected_req))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_resp))
        .expect(1)
        .mount(&mock_server)
        .await;

    let composite = client.composite();
    let batch = BatchRequest {
        batch_requests: vec![SubRequest {
            method: "GET",
            url: "sobjects/Account/123",
            rich_input: None,
            reference_id: Some("ref1"),
        }],
        halt_on_error: false,
    };

    let response = composite.batch(&batch).await.expect("Batch request failed");

    assert!(!response.has_errors);
    assert_eq!(response.results.len(), 1);
    assert_eq!(response.results[0].status_code, 200);

    let result = response.results[0].result.as_ref().unwrap();
    assert_eq!(result["Name"], "Test Account");
}
