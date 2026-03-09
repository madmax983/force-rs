//! Havoc race condition test for `TokenManager::clear` vs concurrent `get_token`.
//!
//! This test demonstrates that calling `clear()` while a slow `authenticate()`
//! or `refresh()` is in progress will fail to actually clear the token,
//! because the slow network request completes and blindly overwrites the `None`
//! state with the newly minted token, resurrecting the session and leaking
//! access to a logged-out user.

#![allow(clippy::unwrap_used)]

use async_trait::async_trait;
use chrono::Utc;
use force::auth::Authenticator;
use force::auth::TokenManager;
use force::auth::{AccessToken, TokenResponse};
use force::error::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::time::Duration;

#[derive(Debug)]
struct MockAuthenticator {
    auth_count: Arc<AtomicUsize>,
}

#[async_trait]
impl Authenticator for MockAuthenticator {
    async fn authenticate(&self) -> Result<AccessToken> {
        self.auth_count.fetch_add(1, Ordering::SeqCst);
        // Simulate a slow network request
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(AccessToken::from_response(TokenResponse {
            access_token: "resurrected_token".to_string(),
            instance_url: "https://test.salesforce.com".to_string(),
            token_type: "Bearer".to_string(),
            issued_at: Utc::now().timestamp_millis().to_string(),
            signature: "sig".to_string(),
            expires_in: None,
            refresh_token: None,
        }))
    }

    async fn refresh(&self) -> Result<AccessToken> {
        self.authenticate().await
    }
}

#[tokio::test]
async fn test_clear_vs_concurrent_refresh() {
    let auth_count = Arc::new(AtomicUsize::new(0));
    let auth = MockAuthenticator {
        auth_count: auth_count.clone(),
    };
    let manager = Arc::new(TokenManager::new(auth));

    // Thread 1: Triggers a slow authentication (100ms)
    let m1 = manager.clone();
    let t1 = tokio::spawn(async move {
        let _ = m1.token().await;
    });

    // Wait 50ms so the network request is definitely in flight
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Thread 2: The user clicks "Logout" and clears the token manager
    let m2 = manager.clone();
    let t2 = tokio::spawn(async move {
        m2.clear().await;
    });

    // Wait for both to finish
    let _ = t1.await;
    let _ = t2.await;

    // Now, if `clear()` was successful, the token state should be `None`.
    // Calling `token()` again should trigger a NEW authentication request.
    // If the vulnerability exists, the pending network request from Thread 1
    // will have blindly overwritten the `None` with `Some("resurrected_token")`,
    // and `auth_count` will still be 1!

    let t3_start = tokio::time::Instant::now();
    let _token = manager.token().await.unwrap();
    let t3_duration = t3_start.elapsed();

    let final_auth_count = auth_count.load(Ordering::SeqCst);

    assert!(
        !(final_auth_count == 1 && t3_duration < Duration::from_millis(50)),
        "👺 Havoc: VULNERABILITY DETECTED! Token resurrected from the dead. `clear()` was overwritten by a pending slow network request."
    );

    assert_eq!(
        final_auth_count, 2,
        "Expected `clear()` to force a new authentication."
    );
}
