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

    /// The sequence generation. Helps prevent old authentications from overwriting `clear()`.
    generation: u64,
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
            state: Arc::new(RwLock::new(TokenState {
                token: None,
                generation: 0,
            })),
            refresh_lock: Mutex::new(()),
        }
    }

    /// Returns the current access token as an Arc reference, refreshing if necessary.
    ///
    /// This is an internal method to avoid cloning the token for internal use.
    pub(crate) async fn get_token_arc(&self) -> Result<Arc<AccessToken>> {
        // Fast path: check if current token is valid (not soft expired)
        {
            let state = self.state.read().await;
            if let Some(token) = &state.token {
                if !token.is_soft_expired() {
                    return Ok(token.clone());
                }
            }
        } // Read lock dropped here

        // Determine if we are in a "soft expired" state (valid but old)
        // or "hard expired/missing" state (invalid).
        let (is_hard_expired, current_token) = {
            let state = self.state.read().await;
            if let Some(token) = &state.token {
                (token.is_hard_expired(), Some(token.clone()))
            } else {
                (true, None)
            }
        };

        if is_hard_expired {
            // Must block and refresh.
            // Record the generation before we yield/block.
            let start_generation = {
                let state = self.state.read().await;
                state.generation
            };

            // Acquire refresh lock to ensure only one thread refreshes.
            let _lock = self.refresh_lock.lock().await;

            // Double-check state (another thread might have refreshed while we waited for lock)
            {
                let state = self.state.read().await;
                // If generation changed due to clear(), we still want to proceed, but if token is valid, use it.
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

            let arc_token = Arc::new(new_token.clone());
            {
                let mut state = self.state.write().await;

                // If generation changed while we were fetching the token, it means clear() was called.
                // We must NOT resurrect the token! We will discard our token and return an error (or just use it?).
                // Wait, if clear() was called, the user wanted to log out. We should respect that and
                // not set the token, but we already have one. We can just return it without saving it,
                // or we can save it. Wait, the point of clear() is to remove access. If we return the token,
                // we're still giving access. Let's discard the token and return an error or try again.
                // Actually, if clear() was called, we should just NOT save it, but we already minted it.
                // It's safer to discard and return an error. Let's return a specific error or InvalidToken.
                if state.generation != start_generation {
                    return Err(crate::error::ForceError::Authentication(
                        crate::error::AuthenticationError::InvalidToken, // Represents "session cleared"
                    ));
                }

                // Check if a concurrent operation already updated the token to a newer one
                if let Some(current) = &state.token {
                    if current.issued_at() > new_token.issued_at() {
                        return Ok(current.clone());
                    }
                }

                state.token = Some(arc_token.clone());
            }
            Ok(arc_token)
        } else if let Some(valid_token) = current_token {
            // Soft expired. We have a valid token (valid_token).

            // Record generation before try_lock (or just before yielding to `refresh().await`)
            let start_generation = {
                let state = self.state.read().await;
                state.generation
            };

            // Try to acquire refresh lock.
            if let Ok(_lock) = self.refresh_lock.try_lock() {
                // We are the refresher.
                // Perform refresh.
                let refresh_result = self.authenticator.refresh().await;

                if let Ok(new_token) = refresh_result {
                    let arc_token = Arc::new(new_token.clone());
                    let mut state = self.state.write().await;

                    // Check if clear() happened. If so, discard token and return InvalidToken.
                    if state.generation != start_generation {
                        return Err(crate::error::ForceError::Authentication(
                            crate::error::AuthenticationError::InvalidToken,
                        ));
                    }

                    // Check if a concurrent operation already updated the token to a newer one
                    if let Some(current) = &state.token {
                        if current.issued_at() > new_token.issued_at() {
                            return Ok(current.clone());
                        }
                    }

                    state.token = Some(arc_token.clone());
                    Ok(arc_token)
                } else {
                    // Refresh failed. Return the old token which is still valid (soft expired).
                    // We swallow the error here because the user can still proceed.
                    // However, if clear() happened during refresh, old valid_token shouldn't be used either!
                    let state = self.state.read().await;
                    if state.generation != start_generation {
                        return Err(crate::error::ForceError::Authentication(
                            crate::error::AuthenticationError::InvalidToken,
                        ));
                    }
                    Ok(valid_token)
                }
            } else {
                // Someone else is refreshing. Return current token immediately.
                Ok(valid_token)
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
        // Capture generation before waiting
        let start_generation = {
            let state = self.state.read().await;
            state.generation
        };

        let new_token = self.authenticator.refresh().await?;
        let arc_token = Arc::new(new_token.clone());

        {
            let mut state = self.state.write().await;

            if state.generation != start_generation {
                return Err(crate::error::ForceError::Authentication(
                    crate::error::AuthenticationError::InvalidToken,
                ));
            }

            // Check if current token is newer than the one we just got.
            // This protects against race conditions where a concurrent `get_token` call
            // might have refreshed the token while we were waiting for the refresh.
            if let Some(current) = &state.token {
                if current.issued_at() > new_token.issued_at() {
                    return Ok(current.as_ref().clone());
                }
            }

            state.token = Some(arc_token);
        } // Write lock dropped here

        Ok(new_token)
    }

    /// Clears the current token, forcing re-authentication on next access.
    ///
    /// This is useful for explicit logout or when you know the token is invalid.
    pub async fn clear(&self) {
        let mut state = self.state.write().await;
        state.token = None;
        state.generation = state.generation.wrapping_add(1);
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
        assert!(result.is_err());

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
        assert!(result.is_err());

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
}
