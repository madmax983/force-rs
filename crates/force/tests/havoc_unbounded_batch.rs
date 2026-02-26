//! Integration test for Unbounded Batch Growth vulnerability.

#![cfg(feature = "composite")]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use async_trait::async_trait;
use force::api::composite::batch::BatchBuilder;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::Result;
use std::sync::OnceLock;
use tokio::runtime::Runtime;

// Mock Authenticator (copied from havoc_batch_panic.rs)
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

fn get_runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| Runtime::new().unwrap())
}

async fn create_batch_builder() -> BatchBuilder<MockAuthenticator> {
    let auth = MockAuthenticator;
    let client = builder().authenticate(auth).build().await.expect("client");
    client.composite().batch()
}

#[test]
fn test_unbounded_growth() {
    let rt = get_runtime();
    rt.block_on(async {
        let mut builder = create_batch_builder().await;

        // Fill up the batch (25 items) - These should all succeed
        for i in 0..25 {
            builder = builder.add_request(
                "GET",
                format!("sobjects/Account/{}", i),
                None
            ).expect("Should succeed up to 25");
        }

        // Verify full
        assert!(builder.is_full());
        assert_eq!(builder.len(), 25);

        // Attack: Add 26th request.
        let result = builder.add_request(
            "GET",
            "sobjects/Account/25",
            None
        );

        // Verification: The builder REJECTED the 26th request.
        assert!(result.is_err(), "Vulnerability fixed: BatchBuilder rejected 26th request");

        let err = result.unwrap_err();
        assert!(err.to_string().contains("Batch size limit exceeded"));
    });
}
