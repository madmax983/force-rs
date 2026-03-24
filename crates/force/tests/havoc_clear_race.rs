#![allow(missing_docs)]

use force::auth::TokenResponse;
use force::auth::{AccessToken, Authenticator, TokenManager};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::time::{Duration, sleep};

#[derive(Debug)]
struct SlowAuthenticator {
    auth_count: Arc<AtomicUsize>,
    refresh_count: Arc<AtomicUsize>,
}

impl SlowAuthenticator {
    fn new() -> Self {
        Self {
            auth_count: Arc::new(AtomicUsize::new(0)),
            refresh_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[async_trait::async_trait]
impl Authenticator for SlowAuthenticator {
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
        // Sleep to ensure `clear()` happens while we are refreshing.
        sleep(Duration::from_millis(100)).await;
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
async fn test_havoc_clear_race_condition() {
    let auth = SlowAuthenticator::new();
    let manager = Arc::new(TokenManager::new(auth));

    // First get a token
    #[allow(clippy::unwrap_used)]
    let token = manager.token().await.unwrap();
    assert_eq!(token.as_str(), "auth_token_0");

    let manager_clone = Arc::clone(&manager);

    // Spawn a task that does a force refresh
    let refresh_task = tokio::spawn(async move {
        let _ = manager_clone.force_refresh().await;
    });

    // Wait a little bit to ensure refresh_task has started and is sleeping
    sleep(Duration::from_millis(20)).await;

    // Concurrently clear the token. This simulates a user explicitly logging out
    // or manually clearing state because of some unrecoverable error.
    manager.clear().await;

    // Await the refresh task. It finishes its sleep and then updates the TokenManager state.
    #[allow(clippy::unwrap_used)]
    refresh_task.await.unwrap();

    // Now, if we try to get token, it SHOULD authenticate again since we cleared it.
    #[allow(clippy::unwrap_used)]
    let token2 = manager.token().await.unwrap();

    // If there is a race condition, the `refresh_task` would have overwritten the `None` state
    // from `clear()` with its newly generated token, which resurrects the session!
    // It should be `auth_token_1` because `authenticate` was called again.
    // If it's `refresh_token_0`, we have resurrected a cleared session!
    assert_eq!(
        token2.as_str(),
        "auth_token_1",
        "The TokenManager resurrected a cleared session!"
    );
}
