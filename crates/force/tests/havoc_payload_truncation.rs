#![allow(missing_docs)]
#![cfg(feature = "mock")]
#![cfg(feature = "rest")]
//! Havoc test for Payload Truncation DoS vulnerability.
//!
//! # 👺 Havoc: Silent payload truncation DoS
//!
//! **The Trigger:** A massive malicious payload (larger than limits) is returned by the server.
//! **The Stack Trace:** Previously it would truncate silently leading to Serde "trailing characters"
//! or missing data, effectively breaking logic without an explicit HTTP limits error.
//! **Reproduction:** Run `cargo test test_havoc_payload_truncation`.
//! **Comment:** You thought truncating a JSON mid-string was safe? Serde disagreed. I broke it.

use force::client::builder;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use async_trait::async_trait;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use serde::Deserialize;
use force::types::QueryResult;
use force::api::rest_operation::RestOperation;

#[derive(Debug)]
struct MockAuthenticator {
    token: String,
    instance_url: String,
}

#[async_trait]
impl Authenticator for MockAuthenticator {
    async fn authenticate(&self) -> force::error::Result<AccessToken> {
        Ok(AccessToken::from_response(TokenResponse {
            access_token: self.token.clone(),
            instance_url: self.instance_url.clone(),
            token_type: "Bearer".to_string(),
            issued_at: "1704067200000".to_string(),
            signature: "sig".to_string(),
            expires_in: None,
            refresh_token: None,
        }))
    }

    async fn refresh(&self) -> force::error::Result<AccessToken> {
        self.authenticate().await
    }
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct TestAccount {
    #[serde(rename = "Id")]
    id: String,
}

#[tokio::test]
async fn test_havoc_payload_truncation() {
    let mock_server = MockServer::start().await;
    let auth = MockAuthenticator {
        token: "test_token".to_string(),
        instance_url: mock_server.uri(),
    };

    // Generate a payload larger than the 1MB limit for `response_to_force_error`.
    let large_body = "A".repeat(2 * 1024 * 1024);

    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/query"))
        .respond_with(ResponseTemplate::new(400).set_body_string(large_body))
        .mount(&mock_server)
        .await;

    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .expect("Failed to build client");

    let result: force::error::Result<QueryResult<TestAccount>> =
        client.rest().query("SELECT Id FROM Account").await;

    if let Err(e) = result {
        let err_str = e.to_string();
        println!("Error: {err_str:?}");
        assert!(err_str.contains("HTTP 400"));
        // The body should be completely absent or unparsed because `read_capped_body` returns `""`
        // due to the invalid input limit error handled by `unwrap_or_default()`.
        // Thus, the fallback message "query failed" (or similar) will be present.
        // As long as it doesn't panic on `serde_json::from_slice` due to silent truncation, we pass.
    } else {
        panic!("👺 Havoc: Expected payload parsing or network limit error!");
    }
}
