//! Token storage and management logic.
//!
//! This module provides the `TokenManager` which handles secure storage,
//! automatic refresh, and concurrent access for OAuth tokens.

use crate::auth::authenticator::Authenticator;
use crate::auth::token::AccessToken;
use crate::error::Result;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

/// Internal state for token management.
#[derive(Debug)]
struct TokenState {
    /// The current access token (if any).
    token: Option<Arc<AccessToken>>,
}

/// Thread-safe token manager with automatic refresh.
#[derive(Debug)]
pub struct TokenManager<A: Authenticator> {
    /// The authenticator for obtaining tokens.
    authenticator: A,

    /// Thread-safe token state.
    state: Arc<RwLock<TokenState>>,

    /// Mutex to serialize refresh operations without blocking readers.
    refresh_lock: Mutex<()>,
}

impl<A: Authenticator> TokenManager<A> {
    /// Creates a new token manager with the given authenticator.
    ///
    /// # Arguments
    ///
    /// * `authenticator` - The authenticator to use for obtaining tokens
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let manager = TokenManager::new(my_authenticator);
    /// ```
    #[must_use]
    pub fn new(authenticator: A) -> Self {
        Self {
            authenticator,
            state: Arc::new(RwLock::new(TokenState { token: None })),
            refresh_lock: Mutex::new(()),
        }
    }

    /// Helper to update the token state in a thread-safe manner, returning the most up-to-date token.
    async fn update_token_state(
        &self,
        arc_token: Arc<AccessToken>,
        is_initial_auth: bool,
    ) -> Arc<AccessToken> {
        let mut state = self.state.write().await;

        if let Some(current) = &state.token {
            if current.issued_at() >= arc_token.issued_at() {
                return current.clone();
            }
            state.token = Some(arc_token.clone());
        } else if is_initial_auth {
            state.token = Some(arc_token.clone());
        }

        arc_token
    }

    /// Returns the currently stored token when it is at least as new as `fallback`.
    async fn latest_token_or(&self, fallback: Arc<AccessToken>) -> Arc<AccessToken> {
        let state = self.state.read().await;

        match &state.token {
            Some(current) if current.issued_at() >= fallback.issued_at() => current.clone(),
            _ => fallback,
        }
    }

    /// Returns the current access token as an Arc reference, refreshing if necessary.
    ///
    /// This is an internal method to avoid cloning the token for internal use.
    pub(crate) async fn get_token_arc(&self) -> Result<Arc<AccessToken>> {
        // Evaluate everything under a single read lock guard
        let (is_soft_expired, is_hard_expired, current_token) = {
            let state = self.state.read().await;
            if let Some(token) = &state.token {
                (
                    token.is_soft_expired(),
                    token.is_hard_expired(),
                    Some(token.clone()),
                )
            } else {
                (false, true, None)
            }
        }; // Read lock dropped here

        // Fast path: check if current token is valid (not soft expired)
        if let Some(token) = current_token.as_ref() {
            if !is_soft_expired && !is_hard_expired {
                return Ok(token.clone());
            }
        }

        if is_hard_expired {
            // Must block and refresh.
            // Acquire refresh lock to ensure only one thread refreshes.
            let _lock = self.refresh_lock.lock().await;

            // Double-check state (another thread might have refreshed while we waited for lock)
            {
                let state = self.state.read().await;
                if let Some(token) = &state.token {
                    if !token.is_hard_expired() {
                        return Ok(token.clone());
                    }
                }
            }

            // Perform refresh/auth
            // We need to check if we have a token to refresh, or if we need initial auth.
            let has_token = {
                let state = self.state.read().await;
                state.token.is_some()
            };

            let new_token = if has_token {
                self.authenticator.refresh().await?
            } else {
                self.authenticator.authenticate().await?
            };

            // ⚡ Bolt: Moving `new_token` directly into `Arc` avoids an unnecessary `.clone()` allocation
            // when transferring ownership, saving one heap allocation per token refresh/auth.
            let arc_token = Arc::new(new_token);
            Ok(self.update_token_state(arc_token, !has_token).await)
        } else if let Some(valid_token) = current_token {
            // Soft expired. We have a valid token (valid_token).
            // Try to acquire refresh lock.
            if let Ok(_lock) = self.refresh_lock.try_lock() {
                // We are the refresher.
                // Perform refresh.
                let refresh_result = self.authenticator.refresh().await;

                match refresh_result {
                    Ok(new_token) => {
                        // ⚡ Bolt: Moving `new_token` directly into `Arc` avoids an unnecessary `.clone()` allocation
                        // when transferring ownership, saving one heap allocation per token refresh.
                        let arc_token = Arc::new(new_token);
                        Ok(self.update_token_state(arc_token, false).await)
                    }
                    Err(_) => {
                        // Refresh failed. Return the old token which is still valid (soft expired).
                        // We swallow the error here because the user can still proceed.
                        Ok(self.latest_token_or(valid_token).await)
                    }
                }
            } else {
                // Someone else is refreshing. Return the latest published token if one is already visible.
                Ok(self.latest_token_or(valid_token).await)
            }
        } else {
            // This path should be unreachable:
            // if current_token is None, is_hard_expired would be true.
            Err(crate::error::ForceError::Authentication(
                crate::error::AuthenticationError::InvalidToken,
            ))
        }
    }

    /// Returns the current access token, refreshing if necessary.
    ///
    /// This method:
    /// 1. Checks if a token exists and is still valid
    /// 2. Refreshes the token if expired
    /// 3. Authenticates if no token exists
    ///
    /// # Errors
    ///
    /// Returns an error if authentication or refresh fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let token = manager.token().await?;
    /// println!("Instance URL: {}", token.instance_url());
    /// ```
    pub async fn token(&self) -> Result<AccessToken> {
        let arc_token = self.get_token_arc().await?;
        Ok((*arc_token).clone())
    }

    /// Forces a token refresh regardless of expiration status.
    ///
    /// This is useful for handling 401 responses where the server has invalidated
    /// the token but the client doesn't know it yet.
    ///
    /// # Errors
    ///
    /// Returns an error if the refresh attempt fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Got a 401 response, force refresh the token
    /// let new_token = manager.force_refresh().await?;
    /// ```
    pub async fn force_refresh(&self) -> Result<AccessToken> {
        let new_token = self.authenticator.refresh().await?;
        // ⚡ Bolt: Moving `new_token` directly into `Arc` avoids an unnecessary `.clone()` allocation
        // when transferring ownership, saving one heap allocation per force refresh.
        let arc_token = Arc::new(new_token);
        let final_token = self.update_token_state(arc_token, false).await;
        Ok((*final_token).clone())
    }

    /// Clears the current token, forcing re-authentication on next access.
    ///
    /// This is useful for explicit logout or when you know the token is invalid.
    pub async fn clear(&self) {
        let mut state = self.state.write().await;
        state.token = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::authenticator::Authenticator;
    use crate::test_support::Must;
    use async_trait::async_trait;
    use chrono::{Duration, Utc};
    use std::sync::Arc as StdArc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Mock authenticator for testing TokenManager
    #[derive(Debug)]
    struct MockAuthenticator {
        auth_count: StdArc<AtomicUsize>,
        refresh_count: StdArc<AtomicUsize>,
        should_fail: bool,
        refresh_delay: Option<std::time::Duration>,
    }

    impl MockAuthenticator {
        fn new() -> Self {
            Self {
                auth_count: StdArc::new(AtomicUsize::new(0)),
                refresh_count: StdArc::new(AtomicUsize::new(0)),
                should_fail: false,
                refresh_delay: None,
            }
        }

        fn with_failure() -> Self {
            Self {
                auth_count: StdArc::new(AtomicUsize::new(0)),
                refresh_count: StdArc::new(AtomicUsize::new(0)),
                should_fail: true,
                refresh_delay: None,
            }
        }

        fn with_delay(mut self, delay: std::time::Duration) -> Self {
            self.refresh_delay = Some(delay);
            self
        }

        fn auth_count(&self) -> usize {
            self.auth_count.load(Ordering::SeqCst)
        }

        fn refresh_count(&self) -> usize {
            self.refresh_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl Authenticator for MockAuthenticator {
        async fn authenticate(&self) -> Result<AccessToken> {
            if let Some(delay) = self.refresh_delay {
                tokio::time::sleep(delay).await;
            }

            self.auth_count.fetch_add(1, Ordering::SeqCst);

            if self.should_fail {
                return Err(crate::error::ForceError::Authentication(
                    crate::error::AuthenticationError::InvalidCredentials(
                        "mock auth failed".to_string(),
                    ),
                ));
            }

            Ok(AccessToken::new(
                format!("auth_token_{}", self.auth_count()),
                "https://test.salesforce.com".to_string(),
                Some(Utc::now() + Duration::hours(2)),
            ))
        }

        async fn refresh(&self) -> Result<AccessToken> {
            if let Some(delay) = self.refresh_delay {
                tokio::time::sleep(delay).await;
            }

            self.refresh_count.fetch_add(1, Ordering::SeqCst);

            if self.should_fail {
                return Err(crate::error::ForceError::Authentication(
                    crate::error::AuthenticationError::TokenRefreshFailed(
                        "mock refresh failed".to_string(),
                    ),
                ));
            }

            Ok(AccessToken::new(
                format!("refresh_token_{}", self.refresh_count()),
                "https://test.salesforce.com".to_string(),
                Some(Utc::now() + Duration::hours(2)),
            ))
        }
    }

    #[tokio::test]
    async fn test_token_manager_initial_auth() {
        let auth = MockAuthenticator::new();
        let manager = TokenManager::new(auth);

        let token = manager.token().await.must();
        assert_eq!(token.as_str(), "auth_token_1");
        assert_eq!(manager.authenticator.auth_count(), 1);
        assert_eq!(manager.authenticator.refresh_count(), 0);
    }

    #[tokio::test]
    async fn test_token_manager_reuses_valid_token() {
        let auth = MockAuthenticator::new();
        let manager = TokenManager::new(auth);

        // First call authenticates
        let token1 = manager.token().await.must();
        assert_eq!(manager.authenticator.auth_count(), 1);

        // Second call reuses token
        let token2 = manager.token().await.must();
        assert_eq!(manager.authenticator.auth_count(), 1); // No new auth
        assert_eq!(token1.as_str(), token2.as_str());
    }

    #[tokio::test]
    async fn test_token_manager_refreshes_expired_token() {
        let auth = MockAuthenticator::new();
        let manager = TokenManager::new(auth);

        // Get initial token
        let _token1 = manager.token().await.must();
        assert_eq!(manager.authenticator.auth_count(), 1);

        // Manually expire the token
        {
            let mut state = manager.state.write().await;
            if let Some(token) = &mut state.token {
                // Create an expired token
                *token = Arc::new(AccessToken::new(
                    "expired_token".to_string(),
                    "https://test.salesforce.com".to_string(),
                    Some(Utc::now() - Duration::hours(1)),
                ));
            }
        }

        // Next call should refresh
        let token2 = manager.token().await.must();
        assert_eq!(manager.authenticator.auth_count(), 1); // No new auth
        assert_eq!(manager.authenticator.refresh_count(), 1); // Refreshed once
        assert_eq!(token2.as_str(), "refresh_token_1");
    }

    #[tokio::test]
    async fn test_token_manager_force_refresh() {
        let auth = MockAuthenticator::new();
        let manager = TokenManager::new(auth);

        // Get initial token
        let token1 = manager.token().await.must();
        assert_eq!(token1.as_str(), "auth_token_1");

        // Force refresh even though token is valid
        let token2 = manager.force_refresh().await.must();
        assert_eq!(token2.as_str(), "refresh_token_1");
        assert_eq!(manager.authenticator.refresh_count(), 1);
    }

    #[tokio::test]
    async fn test_token_manager_clear() {
        let auth = MockAuthenticator::new();
        let manager = TokenManager::new(auth);

        // Get initial token
        let _token1 = manager.token().await.must();
        assert_eq!(manager.authenticator.auth_count(), 1);

        // Clear the token
        manager.clear().await;

        // Next call should authenticate again
        let _token2 = manager.token().await.must();
        assert_eq!(manager.authenticator.auth_count(), 2); // Auth called again
    }

    #[tokio::test]
    async fn test_token_manager_concurrent_access() {
        let auth = MockAuthenticator::new();
        let manager = StdArc::new(TokenManager::new(auth));

        // Spawn multiple tasks trying to get tokens concurrently
        let mut handles = vec![];
        for _ in 0..10 {
            let manager_clone = StdArc::clone(&manager);
            let handle = tokio::spawn(async move { manager_clone.token().await });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            let result = handle.await.must();
            assert!(result.is_ok());
        }

        // Should only authenticate once despite concurrent requests
        assert_eq!(manager.authenticator.auth_count(), 1);
    }

    #[tokio::test]
    async fn test_token_manager_auth_failure() {
        let auth = MockAuthenticator::with_failure();
        let manager = TokenManager::new(auth);

        let result = manager.token().await;

        if let Err(crate::error::ForceError::Authentication(
            crate::error::AuthenticationError::InvalidCredentials(msg),
        )) = result
        {
            assert_eq!(msg, "mock auth failed");
        } else {
            panic!("Expected InvalidCredentials error");
        }
    }

    #[tokio::test]
    async fn test_token_manager_refresh_failure() {
        let auth = MockAuthenticator::with_failure();
        let manager = TokenManager::new(auth);

        // Manually set an expired token
        {
            let mut state = manager.state.write().await;
            state.token = Some(Arc::new(AccessToken::new(
                "expired".to_string(),
                "https://test.salesforce.com".to_string(),
                Some(Utc::now() - Duration::hours(1)),
            )));
        }

        let result = manager.token().await;

        if let Err(crate::error::ForceError::Authentication(
            crate::error::AuthenticationError::TokenRefreshFailed(msg),
        )) = result
        {
            assert_eq!(msg, "mock refresh failed");
        } else {
            panic!("Expected TokenRefreshFailed error");
        }
    }

    #[tokio::test]
    async fn test_token_manager_concurrent_refresh_only_one_request() {
        let auth = MockAuthenticator::new().with_delay(std::time::Duration::from_millis(50));
        let manager = StdArc::new(TokenManager::new(auth));

        // 1. Initial auth to set a token
        let _ = manager.token().await.must();
        assert_eq!(manager.authenticator.auth_count(), 1);

        // 2. Manually expire the token
        {
            let mut state = manager.state.write().await;
            if let Some(token) = &mut state.token {
                *token = Arc::new(AccessToken::new(
                    "expired_token".to_string(),
                    "https://test.salesforce.com".to_string(),
                    Some(Utc::now() - Duration::hours(1)),
                ));
            }
        }

        // 3. Spawn concurrent tasks requesting token
        let mut handles = vec![];
        for _ in 0..50 {
            let manager_clone = StdArc::clone(&manager);
            handles.push(tokio::spawn(
                async move { manager_clone.token().await.must() },
            ));
        }

        // 4. Verify results
        for handle in handles {
            let token = handle.await.must();
            // Should get the refreshed token
            assert_eq!(token.as_str(), "refresh_token_1");
        }

        // 5. Assert refresh was called EXACTLY once
        assert_eq!(
            manager.authenticator.refresh_count(),
            1,
            "Should have refreshed exactly once despite concurrent load"
        );
        // Auth count should remain 1 (from initial setup)
        assert_eq!(manager.authenticator.auth_count(), 1);
    }

    #[tokio::test]
    async fn test_token_manager_force_refresh_protects_against_overwrite() {
        let auth = MockAuthenticator::new();
        let manager = TokenManager::new(auth);

        // 1. Manually set a token with a FUTURE issued_at to simulate a concurrent refresh finishing later
        let future_ts = (Utc::now() + Duration::hours(1)).timestamp_millis();
        let response = crate::auth::token::TokenResponse {
            access_token: "future_token".to_string(),
            instance_url: "https://test.salesforce.com".to_string(),
            token_type: "Bearer".to_string(),
            issued_at: future_ts.to_string(),
            signature: String::new(),
            expires_in: None,
            refresh_token: None,
        };
        let future_token = AccessToken::from_response(response);

        {
            let mut state = manager.state.write().await;
            state.token = Some(StdArc::new(future_token));
        }

        // 2. Call force_refresh
        // The mock authenticator returns a token with `issued_at` roughly NOW (older than future_token).
        let result = manager.force_refresh().await.must();

        // 3. Assert that the result matches the FUTURE token, proving we kept the newer one
        assert_eq!(result.as_str(), "future_token");

        // 4. Verify state also has the future token
        let state_token = manager.token().await.must();
        assert_eq!(state_token.as_str(), "future_token");
    }

    #[tokio::test]
    async fn test_token_manager_hard_refresh_protects_against_overwrite() {
        let auth = MockAuthenticator::new().with_delay(std::time::Duration::from_millis(50));
        let manager = StdArc::new(TokenManager::new(auth));

        // Let the manager start fetching the initial token (hard refresh path)
        let manager_clone = manager.clone();
        let handle = tokio::spawn(async move { manager_clone.token().await.must() });

        // Sleep to ensure the task has acquired the refresh lock and is awaiting `authenticator.authenticate()`
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Concurrently inject a FUTURE token
        let future_ts = (Utc::now() + Duration::hours(1)).timestamp_millis();
        let response = crate::auth::token::TokenResponse {
            access_token: "future_token".to_string(),
            instance_url: "https://test.salesforce.com".to_string(),
            token_type: "Bearer".to_string(),
            issued_at: future_ts.to_string(),
            signature: String::new(),
            expires_in: None,
            refresh_token: None,
        };
        let future_token = AccessToken::from_response(response);

        {
            let mut state = manager.state.write().await;
            state.token = Some(StdArc::new(future_token));
        }

        // Wait for the task to finish
        let result = handle.await.must();

        // The manager's task should have realized a newer token was injected and returned it
        // instead of its own "newly generated" one.
        assert_eq!(result.as_str(), "future_token");

        // Verify state also has the future token
        let final_token = manager.token().await.must();
        assert_eq!(final_token.as_str(), "future_token");
    }

    #[tokio::test]
    async fn test_token_manager_soft_refresh_protects_against_overwrite() {
        let auth = MockAuthenticator::new().with_delay(std::time::Duration::from_millis(50));
        let manager = StdArc::new(TokenManager::new(auth));

        // 1. Manually inject a SOFT EXPIRED token
        // It expires in 30 seconds, which is less than the 60s buffer, so it's soft expired.
        let soft_token = AccessToken::new(
            "soft_token".to_string(),
            "https://test.salesforce.com".to_string(),
            Some(Utc::now() + Duration::seconds(30)),
        );

        {
            let mut state = manager.state.write().await;
            state.token = Some(StdArc::new(soft_token));
        }

        // 2. Trigger token() which will see it's soft expired and call refresh(), taking 50ms
        let manager_clone = manager.clone();
        let handle = tokio::spawn(async move { manager_clone.token().await.must() });

        // 3. Wait 10ms to ensure the spawn starts and begins sleeping
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // 4. Concurrently inject a FUTURE token
        let future_ts = (Utc::now() + Duration::hours(1)).timestamp_millis();
        let future_response = crate::auth::token::TokenResponse {
            access_token: "future_token".to_string(),
            instance_url: "https://test.salesforce.com".to_string(),
            token_type: "Bearer".to_string(),
            issued_at: future_ts.to_string(),
            signature: String::new(),
            expires_in: None,
            refresh_token: None,
        };
        let future_token = AccessToken::from_response(future_response);

        {
            let mut state = manager.state.write().await;
            state.token = Some(StdArc::new(future_token));
        }

        // 5. Await result
        let result = handle.await.must();

        // Should return the injected future token, not the newly refreshed one
        assert_eq!(result.as_str(), "future_token");

        // Verify state also has the future token
        let final_token = manager.token().await.must();
        assert_eq!(final_token.as_str(), "future_token");
    }

    #[tokio::test]
    async fn test_token_manager_equality_overwrites() {
        // We need the mock to generate a token with a specific timestamp to ensure equality.
        // Wait, MockAuthenticator just uses `Utc::now()`.
        // If we inject a token that has a timestamp slightly in the past, the new token will be newer (>),
        // which tests the overwrite. But we specifically want to test EQUALITY (==).
        // Since `TokenResponse` parses `issued_at` exactly, let's force the authenticator to use
        // a known timestamp.

        // Actually, we can just let `force_refresh` return its token,
        // and before the `force_refresh` writes to state, we inject a token with the EXACT SAME TIMESTAMP.
        // But doing it concurrently is hard because we don't control the exact MS the Mock uses.
        // Let's create a special `EqualityAuthenticator` for this specific test.
        #[derive(Debug)]
        struct EqAuth(i64);
        #[async_trait]
        impl Authenticator for EqAuth {
            async fn authenticate(&self) -> Result<AccessToken> {
                let response = crate::auth::token::TokenResponse {
                    access_token: "new_token".to_string(),
                    instance_url: "https://test.salesforce.com".to_string(),
                    token_type: "Bearer".to_string(),
                    issued_at: self.0.to_string(),
                    signature: String::new(),
                    expires_in: None,
                    refresh_token: None,
                };
                Ok(AccessToken::from_response(response))
            }
            async fn refresh(&self) -> Result<AccessToken> {
                self.authenticate().await
            }
        }

        let fixed_ts = Utc::now().timestamp_millis();
        let eq_auth = EqAuth(fixed_ts);
        let eq_manager = TokenManager::new(eq_auth);

        // Inject a token with the EXACT SAME timestamp
        let response = crate::auth::token::TokenResponse {
            access_token: "old_token".to_string(),
            instance_url: "https://test.salesforce.com".to_string(),
            token_type: "Bearer".to_string(),
            issued_at: fixed_ts.to_string(),
            signature: String::new(),
            expires_in: None,
            refresh_token: None,
        };
        let old_token = AccessToken::from_response(response);

        {
            let mut state = eq_manager.state.write().await;
            state.token = Some(StdArc::new(old_token));
        }

        // Call force_refresh. The new token will have the same `issued_at`.
        // Since `old.issued_at >= new.issued_at` is true, it SHOULD NOT overwrite
        // and return the old token ("old_token").
        let result = eq_manager.force_refresh().await.must();
        assert_eq!(
            result.as_str(),
            "old_token",
            "Equality should not trigger an overwrite in force_refresh"
        );

        // Now let's test equality overwrite for hard expiration (line 114)
        let hard_eq_manager = TokenManager::new(EqAuth(fixed_ts));
        // Inject hard expired token with same timestamp
        // The mock will return a token with this exact timestamp
        let response = crate::auth::token::TokenResponse {
            access_token: "hard_old_token".to_string(),
            instance_url: "https://test.salesforce.com".to_string(),
            token_type: "Bearer".to_string(),
            issued_at: fixed_ts.to_string(),
            signature: String::new(),
            expires_in: Some(0), // Hard expired immediately
            refresh_token: None,
        };
        let hard_old_token = AccessToken::from_response(response);
        {
            let mut state = hard_eq_manager.state.write().await;
            state.token = Some(StdArc::new(hard_old_token));
        }

        let result = hard_eq_manager.token().await.must();
        assert_eq!(
            result.as_str(),
            "hard_old_token",
            "Equality should not trigger an overwrite in hard refresh"
        );

        // Now let's test equality overwrite for soft expiration (line 146)
        let soft_eq_manager = TokenManager::new(EqAuth(fixed_ts));
        // Inject soft expired token with same timestamp
        let response = crate::auth::token::TokenResponse {
            access_token: "soft_old_token".to_string(),
            instance_url: "https://test.salesforce.com".to_string(),
            token_type: "Bearer".to_string(),
            issued_at: fixed_ts.to_string(),
            signature: String::new(),
            expires_in: Some(30), // Expires in 30s -> Soft expired
            refresh_token: None,
        };
        let soft_old_token = AccessToken::from_response(response);
        {
            let mut state = soft_eq_manager.state.write().await;
            state.token = Some(StdArc::new(soft_old_token));
        }

        let result = soft_eq_manager.token().await.must();
        assert_eq!(
            result.as_str(),
            "soft_old_token",
            "Equality should not trigger an overwrite in soft refresh"
        );
    }
}
