//! Havoc race condition test for `TokenManager::force_refresh` timestamp resolution.

#![allow(clippy::unwrap_used)]

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use force::auth::{AccessToken, Authenticator, TokenManager, TokenResponse};
    use force::error::Result;
    use std::sync::Arc as StdArc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::time::sleep;

    #[derive(Debug, Clone)]
    struct LowResAuthenticator {
        refresh_count: StdArc<AtomicUsize>,
    }

    impl LowResAuthenticator {
        fn new() -> Self {
            Self {
                refresh_count: StdArc::new(AtomicUsize::new(0)),
            }
        }
    }

    #[async_trait]
    impl Authenticator for LowResAuthenticator {
        async fn authenticate(&self) -> Result<AccessToken> {
            let count = self.refresh_count.fetch_add(1, Ordering::SeqCst);
            sleep(Duration::from_millis(50)).await;

            Ok(AccessToken::from_response(TokenResponse {
                access_token: format!("token_{count}"),
                instance_url: "https://test.salesforce.com".to_string(),
                token_type: "Bearer".to_string(),
                // FIXED timestamp to simulate low resolution or same-millisecond refreshes
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

    #[tokio::test]
    async fn test_force_refresh_timestamp_resolution_stampede() {
        let auth = LowResAuthenticator::new();
        let refresh_count = auth.refresh_count.clone();
        let manager = StdArc::new(TokenManager::new(auth));

        // Initial token fetch
        let _ = manager.token().await.unwrap();
        assert_eq!(refresh_count.load(Ordering::SeqCst), 1);

        // Spawn concurrent force_refresh tasks
        let mut handles = vec![];
        for _ in 0..10 {
            let m = manager.clone();
            handles.push(tokio::spawn(async move {
                m.force_refresh().await.unwrap();
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        let final_count = refresh_count.load(Ordering::SeqCst);

        // We expect AT MOST 2 authentications: 1 for initial token(), and 1 for the first force_refresh()
        // The other 9 should deduplicate and return early because they see a new Arc pointer in the double-check.
        // Or if the first force_refresh() gets deduplicated by the initial token() call due to race conditions
        // we might see 1.
        assert!(
            final_count == 1 || final_count == 2,
            "👺 Havoc: force_refresh triggered a stampede due to timestamp resolution! Expected 2, got {final_count}"
        );
    }
}
