//! Integration test for robust `BatchRequest` behavior.

#![cfg(feature = "composite")]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::sync::OnceLock;

use async_trait::async_trait;
use force::api::composite::BatchRequest;
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
            access_token: secrecy::SecretString::new("token".to_string().into()),
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

async fn create_batch_builder() -> BatchRequest<MockAuthenticator> {
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

    #[test]
    fn test_soql_builder_where_in_panic(values in prop::collection::vec(".*", 1..100)) {
        // Attempt to find inputs that panic the formatting logic in SoqlQueryBuilder
        let mut builder = force::api::rest::SoqlQueryBuilder::new().select(&["Id"]).from("Account");
        builder = builder.where_in("Name", &values);
        let query = builder.build();
        assert!(query.starts_with("SELECT Id FROM Account WHERE Name IN ("));
    }

    #[test]
    fn test_escape_soql_cow_byte_mismatch_panic(s in ".*['\\\\\"].*") {
        // Find bugs in escape_soql_cow char vs byte indexing
        let escaped = force::api::rest::escape_soql(&s);
        let _ = escaped;
    }

}

#[test]
fn test_batch_builder_len_limit() {
    let rt = get_runtime();
    rt.block_on(async {
        let mut builder = create_batch_builder().await;

        for _ in 0..25 {
            builder = builder.get("Account", "001000000000001AAA").unwrap();
        }

        // 26th request should fail
        let res = builder.get("Account", "001000000000001AAA");
        assert!(res.is_err());
    });
}
