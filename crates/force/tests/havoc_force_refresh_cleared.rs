#![allow(clippy::expect_used)]
#![allow(missing_docs)]

use force::auth::TokenResponse;
use force::auth::{AccessToken, Authenticator, TokenManager};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug)]
struct DummyAuthenticator {
    auth_count: Arc<AtomicUsize>,
    refresh_count: Arc<AtomicUsize>,
}

impl DummyAuthenticator {
    fn new() -> Self {
        Self {
            auth_count: Arc::new(AtomicUsize::new(0)),
            refresh_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[async_trait::async_trait]
impl Authenticator for DummyAuthenticator {
    async fn authenticate(&self) -> force::error::Result<AccessToken> {
        let count = self.auth_count.fetch_add(1, Ordering::SeqCst);
        let resp = TokenResponse {
            access_token: format!("auth_token_{count}"),
            instance_url: "https://test.salesforce.com".to_string(),
            token_type: "Bearer".to_string(),
            issued_at: chrono::Utc::now().timestamp_millis().to_string(),
            signature: String::new(),
            expires_in: None,
            refresh_token: None,
        };
        Ok(AccessToken::from_response(resp))
    }

    async fn refresh(&self) -> force::error::Result<AccessToken> {
        let count = self.refresh_count.fetch_add(1, Ordering::SeqCst);
        let resp = TokenResponse {
            access_token: format!("refresh_token_{count}"),
            instance_url: "https://test.salesforce.com".to_string(),
            token_type: "Bearer".to_string(),
            issued_at: chrono::Utc::now().timestamp_millis().to_string(),
            signature: String::new(),
            expires_in: None,
            refresh_token: None,
        };
        Ok(AccessToken::from_response(resp))
    }
}

#[tokio::test]
async fn test_havoc_force_refresh_cleared() {
    let auth = DummyAuthenticator::new();
    let manager = Arc::new(TokenManager::new(auth));

    // First get a token

    let token = manager.token().await.expect("Test setup failure");
    assert_eq!(token.as_str(), "auth_token_0");

    // Clear the token
    manager.clear().await;

    // Call force_refresh. This should NOT return an InvalidToken error.
    // It should authenticate and return a new token.

    let token2 = manager.force_refresh().await.expect("Test setup failure");

    // Since the state was None, it should have called authenticate()
    assert_eq!(token2.as_str(), "auth_token_1");
}
