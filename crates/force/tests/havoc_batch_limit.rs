//! Havoc test for BatchBuilder limits.
//!
//! Verifies that the BatchBuilder strictly enforces the 25-request limit
//! defined by the Salesforce Composite API.
//!
//! "If I can crash it, I win." - Havoc

#![cfg(feature = "composite")]

use async_trait::async_trait;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::{ForceClient, builder};
use force::error::Result as ForceResult;
use proptest::prelude::*;

#[derive(Debug, Clone)]
struct MockAuthenticator;

#[async_trait]
impl Authenticator for MockAuthenticator {
    async fn authenticate(&self) -> ForceResult<AccessToken> {
        Ok(AccessToken::from_response(TokenResponse {
            access_token: "mock_token".to_string(),
            instance_url: "https://mock.salesforce.com".to_string(),
            token_type: "Bearer".to_string(),
            issued_at: "1704067200000".to_string(),
            signature: "sig".to_string(),
            expires_in: None,
            refresh_token: None,
        }))
    }

    async fn refresh(&self) -> ForceResult<AccessToken> {
        self.authenticate().await
    }
}

async fn create_client() -> ForceClient<MockAuthenticator> {
    builder()
        .authenticate(MockAuthenticator)
        .build()
        .await
        .expect("Failed to build client")
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))] // Don't need thousands of runs for this
    #[test]
    fn test_batch_limit_enforcement(n in 26..50usize) {
        // We need a runtime for async client creation
        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(async {
            let client = create_client().await;
            let mut batch = client.composite().batch();

            // Add 25 valid requests
            for i in 0..25 {
                // Must be valid SFID (18 chars)
                let id = format!("001000000000{:03}AAA", i);
                let res = batch.get("Account", &id);
                prop_assert!(res.is_ok(), "Request {} should succeed: {:?}", i, res.err());
                batch = res.unwrap();
            }

            // Attempt to add the 26th (and beyond)
            // HAVOC CHECK:
            // Currently, this is EXPECTED TO FAIL (i.e., return Ok) because the code is broken.
            // The test passes if the code is correct (returns Err).

            let overflow_id = "001000000000999AAA";
            let res = batch.get("Account", overflow_id);

            // This assertion ensures that we get an error when exceeding the limit
            prop_assert!(res.is_err(), "👺 Havoc: BatchBuilder allowed more than 25 requests! Buffer overflow imminent. Got Ok instead of Err.");

            Ok(())
        })?;
    }
}
