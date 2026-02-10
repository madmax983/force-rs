use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::config::{ClientConfig, Environment};
use force::error::Result;
use async_trait::async_trait;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use std::time::Duration;

#[derive(Debug, Clone)]
struct MockAuth {
    instance_url: String,
}

#[async_trait]
impl Authenticator for MockAuth {
    async fn authenticate(&self) -> Result<AccessToken> {
        Ok(AccessToken::from_response(TokenResponse {
            access_token: "mock_token".to_string(),
            instance_url: self.instance_url.clone(),
            token_type: "Bearer".to_string(),
            issued_at: "1704067200000".to_string(),
            signature: "sig".to_string(),
            expires_in: None,
            refresh_token: None,
        }))
    }

    async fn refresh(&self) -> Result<AccessToken> {
        self.authenticate().await
    }
}

/// Regression test for exponential backoff overflow.
///
/// Previously, `exponential_backoff` used `2_u128.pow(attempt)` where `attempt` could be
/// up to `u32::MAX`. This caused a panic on overflow when `attempt >= 128`.
/// This test verifies that the backoff calculation is capped safely and does not panic
/// even with a large number of retries.
#[tokio::test]
async fn test_exponential_backoff_overflow_regression() {
    // Pause time to skip sleeps
    tokio::time::pause();

    let mock_server = MockServer::start().await;

    // Simulate 503 Service Unavailable forever
    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/query"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&mock_server)
        .await;

    let auth = MockAuth {
        instance_url: mock_server.uri(),
    };

    let config = ClientConfig {
        api_version: "v60.0".to_string(),
        environment: Environment::Custom(mock_server.uri()),
        timeout: Duration::from_secs(100000), // Long timeout to avoid timeout during time travel
        max_retries: 200, // Enough to trigger panic (> 127)
    };

    let client = builder()
        .config(config)
        .authenticate(auth)
        .build()
        .await
        .unwrap();

    // Spawn the request in a separate task so we can advance time
    let handle = tokio::spawn(async move {
        client.query::<serde_json::Value>("SELECT Id FROM Account").await
    });

    // Advance time repeatedly to skip sleeps
    for _ in 0..300 {
        if handle.is_finished() {
            break;
        }
        // Advance by enough time to cover the max backoff (30s) + small buffer
        tokio::time::advance(Duration::from_secs(35)).await;
        tokio::task::yield_now().await;
    }

    let result = handle.await;

    match result {
        Ok(res) => {
             match res {
                 Ok(_) => panic!("Request succeeded unexpectedly"),
                 Err(e) => println!("Request failed with error (as expected if no panic): {:?}", e),
             }
        },
        Err(e) => {
            if e.is_panic() {
                // Let the panic propagate to fail the test
                std::panic::resume_unwind(e.into_panic());
            } else {
                panic!("Task failed with non-panic error: {:?}", e);
            }
        }
    }
}
