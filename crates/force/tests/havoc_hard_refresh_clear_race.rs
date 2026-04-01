#![allow(clippy::unwrap_used)]
//! Havoc race test: hard refresh clear race
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
                expires_in: Some(0), // Instantly hard expired
                refresh_token: None,
            }))
        }

        async fn refresh(&self) -> Result<AccessToken, ForceError> {
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
    async fn test_hard_refresh_clear_race() {
        let barrier = Arc::new(Barrier::new(2));
        let auth = RaceAuth { barrier: barrier.clone() };
        let manager = Arc::new(TokenManager::new(auth));

        // Setup initial hard expired token
        let _ = manager.token().await.unwrap();

        let m1 = manager.clone();

        let handle1 = tokio::spawn(async move {
            m1.token().await
        });

        sleep(Duration::from_millis(50)).await;

        let m2 = manager.clone();
        let handle2 = tokio::spawn(async move {
            m2.clear().await;
        });

        handle2.await.unwrap();

        barrier.wait().await;
        let result = handle1.await.unwrap();

        assert!(result.is_err(), "Expected InvalidToken error");
    }
}
