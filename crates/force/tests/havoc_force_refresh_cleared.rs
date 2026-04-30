#![allow(clippy::unwrap_used)]
//! 👺 Havoc: `TokenManager` revive session race condition
//!
//! **The Trigger:** Calling `clear()` concurrently while `handle_hard_refresh`
//! or `force_refresh` are yielding on `authenticate()` or `refresh()`.
//! **The Stack Trace:** No panic, but the cleared session is silently revived!
//! **Reproduction:** Run `cargo test --test havoc_force_refresh_cleared`
//! **Comment:** The `is_initial_auth = !has_token` check was vulnerable to TOCTOU.

use force::auth::{AccessToken, Authenticator, TokenManager, TokenResponse};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Barrier;

#[derive(Debug)]
struct YieldingAuthenticator {
    count: Arc<AtomicUsize>,
    wait_before_auth: Arc<Barrier>,
}

#[async_trait::async_trait]
impl Authenticator for YieldingAuthenticator {
    async fn authenticate(&self) -> force::error::Result<AccessToken> {
        self.wait_before_auth.wait().await;
        // Give time for clear() to modify state
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        let c = self.count.fetch_add(1, Ordering::SeqCst);
        Ok(AccessToken::from_response(TokenResponse {
            access_token: format!("auth_{c}"),
            instance_url: "url".into(),
            token_type: "Bearer".into(),
            issued_at: chrono::Utc::now().timestamp_millis().to_string(),
            signature: "sig".into(),
            expires_in: None,
            refresh_token: None,
        }))
    }
    async fn refresh(&self) -> force::error::Result<AccessToken> {
        self.authenticate().await
    }
}

#[tokio::test]
async fn test_havoc_handle_hard_refresh_cleared() {
    let barrier = Arc::new(Barrier::new(2));
    let auth = YieldingAuthenticator {
        count: Arc::new(AtomicUsize::new(0)),
        wait_before_auth: barrier.clone(),
    };
    let manager = Arc::new(TokenManager::new(auth));

    // We start with NO token.
    let m2 = manager.clone();

    // 1. Call token(). It calls get_token_arc(). state is None -> calls handle_hard_refresh.
    // Acquires refresh lock.
    // Checks state -> None.
    // has_token -> false.
    // Calls authenticate(), which blocks on the barrier.
    let handle = tokio::spawn(async move { m2.token().await });

    barrier.wait().await;

    // NOW it is inside authenticate().
    // 2. Let's clear the token!
    manager.clear().await;

    // 3. NOW token() finishes `authenticate()`.
    // It calls `update_token_state` with `is_initial_auth` = `!has_token` = `true`!
    // But since the fix, `update_token_state` checks `clear_count` instead.
    // The `clear_count` was incremented, so it correctly returns an InvalidToken error.
    let res = handle.await.unwrap();

    assert!(
        matches!(
            res,
            Err(force::error::ForceError::Authentication(
                force::error::AuthenticationError::InvalidToken
            ))
        ),
        "Expected InvalidToken error to prevent reviving cleared session, got {res:?}",
    );
}
