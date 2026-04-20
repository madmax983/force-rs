#![allow(clippy::expect_used)]
#![allow(missing_docs)]
#![allow(clippy::unwrap_used)]
#![cfg(feature = "rest")]

use async_trait::async_trait;
use force::api::RestOperation;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::ForceClientBuilder;
use force::error::Result;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[derive(Debug, Clone)]
struct MockAuthenticator {
    token: String,
    instance_url: String,
}

impl MockAuthenticator {
    fn new(token: impl Into<String>, instance_url: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            instance_url: instance_url.into(),
        }
    }
}

#[async_trait]
impl Authenticator for MockAuthenticator {
    async fn authenticate(&self) -> Result<AccessToken> {
        let response = TokenResponse {
            access_token: self.token.clone(),
            instance_url: self.instance_url.clone(),
            token_type: "Bearer".to_string(),
            issued_at: "1672531200000".to_string(), // 2023-01-01
            signature: "sig".to_string(),
            expires_in: None,
            refresh_token: None,
        };
        Ok(AccessToken::from_response(response))
    }

    async fn refresh(&self) -> Result<AccessToken> {
        self.authenticate().await
    }
}

#[tokio::test]
async fn test_upsert_path_injection() {
    let mock_server = MockServer::start().await;
    let auth = MockAuthenticator::new("test_token", mock_server.uri());

    // We expect the client to properly encode the value "A/B" to "A%2FB"
    // If it doesn't, it will send a request to .../ExternalId__c/A/B
    // The mock server will not match this path if it expects encoded slash.

    // Wiremock path matching:
    // If we use `path`, it matches decoded path.
    // If we want to verify encoding, we need to check the raw request or use a regex on URI.
    // But let's start simple.
    // If client sends unencoded `/`, `reqwest` + `wiremock` might treat it as path separator.
    // `/sobjects/Account/ExternalId__c/A/B` (decoded)
    // `/sobjects/Account/ExternalId__c/A%2FB` (decoded to `/sobjects/Account/ExternalId__c/A/B`?)

    // Actually, `path` matcher normalizes.
    // Let's use `uri` matcher or check logs if it fails.
    // Or better, setup a mock that catches the UNENCODED version and fails the test.

    Mock::given(method("PATCH"))
        .and(path(
            "/services/data/v60.0/sobjects/Account/ExternalId__c/A%2FB",
        ))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "001xx000003DHP0AAO",
            "success": true,
            "created": true,
            "errors": []
        })))
        .mount(&mock_server)
        .await;

    // Also catch unencoded version to be sure
    Mock::given(method("PATCH"))
        .and(path(
            "/services/data/v60.0/sobjects/Account/ExternalId__c/A/B",
        ))
        .respond_with(ResponseTemplate::new(400)) // Fail
        .mount(&mock_server)
        .await;

    let client = ForceClientBuilder::new()
        .authenticate(auth)
        .build()
        .await
        .expect("test failed");

    let result = client
        .rest()
        .upsert(
            "Account",
            "ExternalId__c",
            "A/B", // Malicious value containing a slash
            &serde_json::json!({"Name": "Test"}),
        )
        .await;

    // We expect Ok() because the client SHOULD encode it.
    // If it fails (Err), it means it hit the 400 mock or didn't match anything.
    assert!(result.is_ok(), "Upsert failed. Result: {result:?}");
}
