//! 👺 Havoc: Marketing Cloud Token Stall
//!
//! Tests that concurrent requests during the soft-expiry window
//! stall on the single-flight lock instead of using the still-valid token.

use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use force_marketingcloud::{AccessToken, Authenticator, TokenManager};
use secrecy::SecretString;

#[allow(
    clippy::unwrap_used,
    clippy::significant_drop_tightening,
    clippy::manual_string_new
)]
#[derive(Debug)]
struct StallingAuth {
    calls: AtomicUsize,
    first_call: std::sync::Mutex<bool>,
}

#[allow(
    clippy::unwrap_used,
    clippy::significant_drop_tightening,
    clippy::manual_string_new
)]
#[async_trait]
impl Authenticator for StallingAuth {
    async fn authenticate(
        &self,
        _account_id: Option<&str>,
    ) -> force_marketingcloud::Result<AccessToken> {
        self.calls.fetch_add(1, Ordering::SeqCst);

        let mut is_first = false;
        {
            let mut first = self.first_call.lock().unwrap();
            if *first {
                *first = false;
                is_first = true;
            }
        }

        if is_first {
            // First call: return a token that is in the soft-expiry window.
            // Soft-expiry buffer is 60s. So 30s < 60s -> needs_refresh() is true.
            let response = force_marketingcloud::TokenResponse {
                access_token: SecretString::new("soft_token".to_string().into()),
                token_type: "Bearer".to_string(),
                expires_in: 30, // Expires in 30 seconds
                rest_instance_url: "https://test.rest.marketingcloudapis.com/".to_string(),
                soap_instance_url: "".to_string(),
                scope: "".to_string(),
            };
            Ok(AccessToken::from_response(response).unwrap())
        } else {
            // Second call: simulate a slow network refresh
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            // Return a completely fresh token
            let response = force_marketingcloud::TokenResponse {
                access_token: SecretString::new("fresh_token".to_string().into()),
                token_type: "Bearer".to_string(),
                expires_in: 3600,
                rest_instance_url: "https://test.rest.marketingcloudapis.com/".to_string(),
                soap_instance_url: "".to_string(),
                scope: "".to_string(),
            };
            Ok(AccessToken::from_response(response).unwrap())
        }
    }
}

#[allow(clippy::unwrap_used)]
#[tokio::test]
async fn test_mc_token_stall() {
    let auth = Arc::new(StallingAuth {
        calls: AtomicUsize::new(0),
        first_call: std::sync::Mutex::new(true),
    });
    let manager = Arc::new(TokenManager::new(auth.clone()));

    // 1. Fetch initial token. Returns the soft-expired token immediately.
    let t1 = manager.token(None).await.unwrap();
    assert_eq!(t1.as_str(), "soft_token");
    assert_eq!(auth.calls.load(Ordering::SeqCst), 1);

    // 2. Now the cache has a soft-expired token.
    // Spawn a background task that will trigger the slow refresh.
    let m1 = manager.clone();
    let handle1 = tokio::spawn(async move { m1.token(None).await.unwrap() });

    // Wait a tiny bit to ensure handle1 acquires the single-flight lock
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // 3. 👺 Havoc: While the refresh is in progress, any new request SHOULD
    // immediately get the still-valid soft_token instead of stalling.
    let start = std::time::Instant::now();
    let t2 = manager.token(None).await.unwrap();
    let elapsed = start.elapsed();

    // The stall happens because `token()` tries to acquire the lock when `cached_valid()` returns `None`.
    assert!(
        elapsed < std::time::Duration::from_millis(100),
        "👺 Havoc: Token request stalled for {elapsed:?} due to single-flight blocking on soft-expiry!"
    );

    // Ensure we actually got the old token
    assert_eq!(
        t2.as_str(),
        "soft_token",
        "👺 Havoc: Waited for the new token instead of returning the valid old one!"
    );

    let _ = handle1.await.unwrap();
}
