//! Integration test for robust `BatchBuilder` behavior.

#![cfg(feature = "composite")]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::sync::OnceLock;

use async_trait::async_trait;
use force::api::composite::BatchBuilder;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::Result;
use proptest::prelude::*;
use tokio::runtime::Runtime;

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

fn get_runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| Runtime::new().unwrap())
}

async fn create_batch_builder() -> BatchBuilder<MockAuthenticator> {
    let auth = MockAuthenticator;
    let client = builder().authenticate(auth).build().await.expect("client");
    client.composite().batch()
}

proptest! {
    #[test]
    fn test_batch_builder_robustness(s in "\\PC*") {
        let rt = get_runtime();
        rt.block_on(async {
            let builder = create_batch_builder().await;

            // HAVOC FIX: This should now return Result, not panic.
            let result = builder.get(&s, "001000000000001AAA");

            // Just ensure it doesn't panic.
            let _ = result;
        });
    }
}
