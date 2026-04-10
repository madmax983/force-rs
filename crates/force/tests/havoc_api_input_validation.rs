#![allow(clippy::unwrap_used, missing_docs)]

use async_trait::async_trait;
use force::api::RestOperation;
use force::auth::AccessToken;
use force::auth::Authenticator;
use force::client::builder;
use proptest::prelude::*;
use std::sync::LazyLock;
use tokio::runtime::Runtime;

#[derive(Debug, Clone)]
struct MyAuth;

#[async_trait]
impl Authenticator for MyAuth {
    async fn authenticate(&self) -> force::error::Result<AccessToken> {
        let resp = serde_json::from_str::<force::auth::TokenResponse>(
            r#"{
            "access_token": "test",
            "instance_url": "https://test.salesforce.com",
            "token_type": "Bearer",
            "issued_at": "1000",
            "signature": "sig"
        }"#,
        )
        .unwrap();
        Ok(AccessToken::from_response(resp))
    }
    async fn refresh(&self) -> force::error::Result<AccessToken> {
        self.authenticate().await
    }
}

// Initialize the Tokio runtime once lazily to avoid overhead of 256 runtimes per test
static RT: LazyLock<Runtime> = LazyLock::new(|| Runtime::new().unwrap());

proptest! {
    #[test]
    fn havoc_describe_never_makes_network_call_on_invalid_chars(s in "[^a-zA-Z0-9_]+") {
        RT.block_on(async {
            let auth = MyAuth;
            let client = builder().authenticate(auth).build().await.unwrap();
            let rest = client.rest();

            let result = rest.describe(&s).await;

            match result {
                Ok(_) => panic!("Havoc win! Describe executed with invalid characters!"),
                Err(e) => {
                    let error_msg = e.to_string();
                    assert!(
                        error_msg.contains("cannot be empty") || error_msg.contains("invalid characters"),
                        "Expected InvalidInput from validation, got network/HTTP error: {error_msg}"
                    );
                }
            }
        });
    }

    #[test]
    fn havoc_query_never_makes_network_call_on_huge_garbage(size in 100_001..200_000usize) {
        RT.block_on(async {
            let auth = MyAuth;
            let client = builder().authenticate(auth).build().await.unwrap();
            let rest = client.rest();

            let very_long_string = "A".repeat(size);
            let result = rest.query::<serde_json::Value>(&very_long_string).await;

            match result {
                Ok(_) => panic!("Havoc win! Query executed with garbage!"),
                Err(e) => {
                    let error_msg = e.to_string();
                    assert!(
                        error_msg.contains("too long"),
                        "Expected InvalidInput from validation, got network/HTTP error: {error_msg}"
                    );
                }
            }
        });
    }

    #[test]
    fn havoc_query_more_never_makes_network_call_on_huge_garbage(size in 100_001..200_000usize) {
        RT.block_on(async {
            let auth = MyAuth;
            let client = builder().authenticate(auth).build().await.unwrap();
            let rest = client.rest();

            let very_long_string = "A".repeat(size);
            let result = rest.query_more::<serde_json::Value>(&very_long_string).await;

            match result {
                Ok(_) => panic!("Havoc win! Query executed with garbage!"),
                Err(e) => {
                    let error_msg = e.to_string();
                    assert!(
                        error_msg.contains("too long"),
                        "Expected InvalidInput from validation, got network/HTTP error: {error_msg}"
                    );
                }
            }
        });
    }
}
