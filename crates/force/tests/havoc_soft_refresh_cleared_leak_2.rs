//! 👺 Havoc: `TokenManager` soft-refresh clear race condition
//!
//! **The Trigger:** A soft-expired token attempts a background refresh while
//! another thread clears the token manager state. The background refresh then fails
//! and returns the old snapshot.
//! **The Stack Trace:** The session is revived despite being explicitly cleared,
//! returning a valid token when it should be `InvalidToken`.
//! **Reproduction:** Run `cargo test --test havoc_soft_refresh_cleared_leak_2`

#![allow(clippy::unwrap_used)]
use force::auth::{AccessToken, Authenticator, TokenManager, TokenResponse};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Barrier;

#[derive(Debug)]
struct YieldingAuthenticator {
    count: Arc<AtomicUsize>,
    wait_before_auth: Arc<Barrier>,
    should_fail: bool,
}

#[async_trait::async_trait]
impl Authenticator for YieldingAuthenticator {
    async fn authenticate(&self) -> force::error::Result<AccessToken> {
        let c = self.count.fetch_add(1, Ordering::SeqCst);
        Ok(AccessToken::from_response(TokenResponse {
            access_token: format!("auth_{c}"),
            instance_url: "url".into(),
            token_type: "Bearer".into(),
            issued_at: chrono::Utc::now().timestamp_millis().to_string(),
            signature: "sig".into(),
            expires_in: Some(30), // Soft expired
            refresh_token: None,
        }))
    }
    async fn refresh(&self) -> force::error::Result<AccessToken> {
        self.wait_before_auth.wait().await;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        if self.should_fail {
            Err(force::error::ForceError::Authentication(
                force::error::AuthenticationError::TokenRefreshFailed("failed".into()),
            ))
        } else {
            self.authenticate().await
        }
    }
}

#[tokio::test]
async fn test_havoc_handle_soft_refresh_cleared_leak() {
    let barrier = Arc::new(Barrier::new(2));
    let auth = YieldingAuthenticator {
        count: Arc::new(AtomicUsize::new(0)),
        wait_before_auth: barrier.clone(),
        should_fail: true,
    };
    let manager = Arc::new(TokenManager::new(auth));

    // Force an initial token that is softly expired
    manager.token().await.unwrap();

    let m2 = manager.clone();

    // 1. Call token(). It calls get_token_arc(). state is softly expired -> calls handle_soft_refresh.
    // Acquires refresh lock.
    // Calls refresh(), which blocks on the barrier.
    let handle = tokio::spawn(async move { m2.token().await });

    barrier.wait().await;

    // NOW it is inside refresh().
    // 2. Let's clear the token!
    manager.clear().await;

    // 3. NOW token() finishes `refresh()` and fails.
    // It calls `self.latest_token_or(valid_token).await`
    // Since state is cleared, it returns the OLD valid_token!
    let res = handle.await;

    assert!(
        matches!(
            res.unwrap(),
            Err(force::error::ForceError::Authentication(
                force::error::AuthenticationError::InvalidToken
            ))
        ),
        "Expected InvalidToken error to prevent reviving cleared session, got success!"
    );
}
