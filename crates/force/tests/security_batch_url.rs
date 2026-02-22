//! Security tests for Batch API URL construction.

#![allow(clippy::expect_used)]

use async_trait::async_trait;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::Result as ForceResult;
use serde_json::json;
use wiremock::matchers::{body_string_contains, method, path};
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
    async fn authenticate(&self) -> ForceResult<AccessToken> {
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

    async fn refresh(&self) -> ForceResult<AccessToken> {
        self.authenticate().await
    }
}

#[tokio::test]
async fn test_batch_request_params_encoding() {
    let mock_server = MockServer::start().await;

    // We expect a POST to /composite/batch
    // And the body should contain the encoded URL
    Mock::given(method("POST"))
        .and(path("/services/data/v60.0/composite/batch"))
        .and(body_string_contains(
            "query?q=SELECT+Id+FROM+Account+WHERE+Name+%3D+%27Acme+%26+Co%27",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "hasErrors": false,
            "results": [
                {
                    "statusCode": 200,
                    "result": {"totalSize": 1, "done": true, "records": []}
                }
            ]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let auth = MockAuthenticator::new("token", &mock_server.uri());
    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .expect("failed to build client");

    let result = client
        .composite()
        .batch()
        .add_request_with_params(
            "GET",
            "query",
            &[("q", "SELECT Id FROM Account WHERE Name = 'Acme & Co'")],
            None,
        )
        .execute()
        .await;

    assert!(result.is_ok());
}
