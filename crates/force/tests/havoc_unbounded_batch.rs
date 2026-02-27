//! Havoc test for unbounded batch growth.
//!
//! This test verifies that `BatchBuilder` now correctly enforces the limit of 25 requests.

#![cfg(feature = "composite")]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use async_trait::async_trait;
use force::api::composite::batch::BatchBuilder;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::{ForceError, Result};

// Mock Authenticator
#[derive(Debug, Clone)]
struct MockAuthenticator;

#[async_trait]
impl Authenticator for MockAuthenticator {
    async fn authenticate(&self) -> Result<AccessToken> {
        let response = TokenResponse {
            access_token: "token".to_string(),
            instance_url: "https://test.salesforce.com".to_string(),
            token_type: "Bearer".to_string(),
            issued_at: "1704067200000".to_string(),
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

async fn create_batch_builder() -> BatchBuilder<MockAuthenticator> {
    let auth = MockAuthenticator;
    let client = builder().authenticate(auth).build().await.expect("client");
    client.composite().batch()
}

#[tokio::test]
async fn test_batch_builder_enforces_request_limit() {
    let mut builder = create_batch_builder().await;

    // Add 25 requests (allowed)
    for i in 0..25 {
        builder = builder
            .get("Account", &format!("001000000000{i:03}AAA"))
            .expect("Failed to add valid request");
    }

    assert_eq!(builder.len(), 25);
    assert!(builder.is_full());

    // Try to add 26th request (should fail)
    let result = builder.get("Account", "001000000000026AAA");

    assert!(result.is_err());

    match result {
        Err(ForceError::InvalidInput(msg)) => {
            assert!(msg.contains("limit of 25 requests reached"));
        }
        _ => panic!("Expected ForceError::InvalidInput when exceeding batch limit"),
    }
}
