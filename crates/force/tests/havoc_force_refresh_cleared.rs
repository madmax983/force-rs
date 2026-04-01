#![allow(clippy::unwrap_used)]
//! Havoc race test: `force_refresh` while state is cleared
#[cfg(test)]
mod tests {
    use force::auth::{Authenticator, TokenManager, AccessToken, TokenResponse};
    use force::error::ForceError;

    use async_trait::async_trait;
    use std::sync::Arc;
    use tokio::sync::Barrier;
    use tokio::time::{sleep, Duration};

    #[derive(Debug)]
    struct RaceAuth {
        barrier: Arc<Barrier>,
    }

    #[async_trait]
    impl Authenticator for RaceAuth {
        async fn authenticate(&self) -> Result<AccessToken, ForceError> {
            Ok(AccessToken::from_response(TokenResponse {
                access_token: "token".to_string(),
                instance_url: "url".to_string(),
                token_type: "Bearer".to_string(),
                issued_at: "1000".to_string(),
                signature: "sig".to_string(),
                expires_in: None,
                refresh_token: None,
            }))
        }

        async fn refresh(&self) -> Result<AccessToken, ForceError> {
            // Wait at the barrier so the other thread can clear the state
            self.barrier.wait().await;
            Ok(AccessToken::from_response(TokenResponse {
                access_token: "refreshed".to_string(),
                instance_url: "url".to_string(),
                token_type: "Bearer".to_string(),
                issued_at: "2000".to_string(),
                signature: "sig".to_string(),
                expires_in: None,
                refresh_token: None,
            }))
        }
    }

    #[tokio::test]
    async fn test_force_refresh_cleared_race() {
        let barrier = Arc::new(Barrier::new(2));
        let auth = RaceAuth { barrier: barrier.clone() };
        let manager = Arc::new(TokenManager::new(auth));

        // Setup initial token
        let _ = manager.token().await.unwrap();

        let m1 = manager.clone();
        let b = barrier.clone();

        // Thread 1: Start force refresh
        let handle1 = tokio::spawn(async move {
            m1.force_refresh().await
        });

        // Let Thread 1 get past the first few await points and hit the refresh barrier
        sleep(Duration::from_millis(50)).await;

        // Thread 2: Clear the token state
        let m2 = manager.clone();
        let handle2 = tokio::spawn(async move {
            m2.clear().await;
        });

        handle2.await.unwrap();

        // Now let Thread 1 continue with its refresh
        b.wait().await;

        let result = handle1.await.unwrap();

        // It should either return an error or succeed. But since it had a token
        // when it started (has_token = true), it called refresh(), and then it will call
        // update_token_state with is_initial_auth = false.
        // update_token_state returns an InvalidToken error when state is cleared and is_initial_auth = false!
        assert!(
            result.is_err(),
            "Expected InvalidToken error when state is cleared during force_refresh, got: {result:?}"
        );
    }
}
