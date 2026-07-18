//! Tests for chaotic soft refresh behavior in `TokenManager`

use async_trait::async_trait;
use force_marketingcloud::AccessToken;
use force_marketingcloud::Authenticator;
use force_marketingcloud::TokenManager;
use force_marketingcloud::TokenResponse;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug)]
struct FailingAuth {
    calls: AtomicUsize,
}

#[async_trait]
impl Authenticator for FailingAuth {
    async fn authenticate(
        &self,
        _account_id: Option<&str>,
    ) -> force_marketingcloud::Result<AccessToken> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        if n == 0 {
            // First call succeeds, returns a token that is soft-expired (expires in 30 seconds)
            let response = TokenResponse {
                access_token: secrecy::SecretString::new("soft-expired-token".to_string().into()),
                token_type: "Bearer".to_string(),
                expires_in: 30,
                rest_instance_url: "https://sub.rest.marketingcloudapis.com/".to_string(),
                soap_instance_url: String::new(),
                scope: String::new(),
            };
            AccessToken::from_response(response)
        } else {
            // Subsequent calls fail
            Err(force_marketingcloud::MarketingCloudError::Auth(
                "Refresh failed".to_string(),
            ))
        }
    }
}

#[tokio::test]
async fn test_marketing_cloud_soft_refresh_failure_returns_old_token() {
    let auth = Arc::new(FailingAuth {
        calls: AtomicUsize::new(0),
    });
    let manager = TokenManager::new(auth.clone());

    // First call should succeed and cache the soft-expired token
    let token1 = match manager.token(None).await {
        Ok(t) => t,
        Err(e) => panic!("Expected Ok, got {e:?}"),
    };
    assert_eq!(token1.as_str(), "soft-expired-token");

    // Second call should attempt to refresh (because it's soft-expired)
    // The refresh will fail. But since the token is only soft-expired,
    // it should return the old token instead of bubbling up the error!
    let token2_result = manager.token(None).await;

    assert!(
        token2_result.is_ok(),
        "👺 Havoc: Soft refresh failure bubbled up an error instead of returning the valid old token: {:?}",
        token2_result.err()
    );
}
