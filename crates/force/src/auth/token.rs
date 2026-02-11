//! Access token management and OAuth token responses.
//!
//! This module provides types for managing OAuth access tokens, including:
//! - Secure token storage with expiration tracking
//! - Automatic token refresh with configurable buffers
//! - Thread-safe concurrent access
//! - Force refresh on 401 responses

use crate::error::Result;
use chrono::{DateTime, Duration, Utc};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

/// OAuth token response from Salesforce.
///
/// This structure represents the JSON response from Salesforce OAuth endpoints.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenResponse {
    /// The access token string.
    pub access_token: String,

    /// The instance URL for API requests.
    pub instance_url: String,

    /// Token type (typically "Bearer").
    #[serde(default = "default_token_type")]
    pub token_type: String,

    /// Issued at timestamp (Unix epoch seconds).
    pub issued_at: String,

    /// Token signature.
    #[serde(default)]
    pub signature: String,

    /// Expires in seconds (optional, not all flows provide this).
    #[serde(default)]
    pub expires_in: Option<u64>,

    /// Refresh token (optional, only for flows that support refresh).
    #[serde(default)]
    pub refresh_token: Option<String>,
}

fn default_token_type() -> String {
    "Bearer".to_string()
}

/// A secure access token with expiration tracking.
///
/// Access tokens are stored using `secrecy::Secret` to prevent accidental
/// logging or display of sensitive credentials.
#[derive(Debug, Clone)]
pub struct AccessToken {
    /// The token value (kept secret).
    token: SecretString,

    /// When the token was issued.
    issued_at: DateTime<Utc>,

    /// Token expiration time (if known).
    expires_at: Option<DateTime<Utc>>,

    /// Salesforce instance URL.
    instance_url: String,

    /// Token type (e.g., "Bearer").
    token_type: String,
}

impl AccessToken {
    /// Creates a new access token from an OAuth response.
    ///
    /// # Arguments
    ///
    /// * `response` - The OAuth token response from Salesforce
    ///
    /// # Returns
    ///
    /// A new `AccessToken` instance with expiration tracking.
    #[must_use]
    pub fn from_response(response: TokenResponse) -> Self {
        let issued_at = parse_issued_at(&response.issued_at).unwrap_or_else(|_| Utc::now());
        let expires_at = response
            .expires_in
            .map(|seconds| issued_at + Duration::seconds(i64::try_from(seconds).unwrap_or(3600)));

        Self {
            token: SecretString::new(response.access_token.into()),
            issued_at,
            expires_at,
            instance_url: response.instance_url,
            token_type: response.token_type,
        }
    }

    /// Creates a new access token with explicit values (primarily for testing).
    ///
    /// # Arguments
    ///
    /// * `token` - The token value
    /// * `instance_url` - The Salesforce instance URL
    /// * `expires_at` - Optional expiration time
    #[cfg(test)]
    pub fn new(token: String, instance_url: String, expires_at: Option<DateTime<Utc>>) -> Self {
        Self {
            token: SecretString::new(token.into()),
            issued_at: Utc::now(),
            expires_at,
            instance_url,
            token_type: "Bearer".to_string(),
        }
    }

    /// Returns the token value as a string reference.
    ///
    /// # Security
    ///
    /// This exposes the secret token value. Use with care and avoid logging.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.token.expose_secret()
    }

    /// Returns the instance URL for API requests.
    #[must_use]
    pub fn instance_url(&self) -> &str {
        &self.instance_url
    }

    /// Returns the token type (typically "Bearer").
    #[must_use]
    pub fn token_type(&self) -> &str {
        &self.token_type
    }

    /// Checks if the token is expired.
    ///
    /// Uses a 60-second buffer to proactively refresh tokens before they expire.
    /// If no expiration time is known, returns `false` (assume valid).
    ///
    /// # Returns
    ///
    /// `true` if the token is expired or will expire within 60 seconds.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.is_expired_with_buffer(Duration::seconds(60))
    }

    /// Checks if the token is expired with a custom buffer.
    ///
    /// # Arguments
    ///
    /// * `buffer` - Time buffer before actual expiration
    ///
    /// # Returns
    ///
    /// `true` if the token will expire within the buffer period.
    #[must_use]
    pub fn is_expired_with_buffer(&self, buffer: Duration) -> bool {
        self.expires_at
            .is_some_and(|expires_at| Utc::now() + buffer >= expires_at)
    }

    /// Returns when the token was issued.
    #[must_use]
    pub const fn issued_at(&self) -> DateTime<Utc> {
        self.issued_at
    }

    /// Returns when the token expires (if known).
    #[must_use]
    pub const fn expires_at(&self) -> Option<DateTime<Utc>> {
        self.expires_at
    }
}

/// Parses the `issued_at` timestamp from Salesforce OAuth response.
///
/// The `issued_at` field is a Unix timestamp in milliseconds as a string.
fn parse_issued_at(issued_at: &str) -> Result<DateTime<Utc>> {
    let timestamp_ms = issued_at.parse::<i64>().map_err(|_| {
        crate::error::ForceError::Serialization(crate::error::SerializationError::InvalidFormat(
            format!("invalid issued_at timestamp: {issued_at}"),
        ))
    })?;

    let timestamp_secs = timestamp_ms / 1000;
    DateTime::from_timestamp(timestamp_secs, 0).ok_or_else(|| {
        crate::error::ForceError::Serialization(crate::error::SerializationError::InvalidFormat(
            format!("timestamp out of range: {timestamp_ms}"),
        ))
    })
}

/// Manages access tokens with automatic refresh and thread-safe concurrent access.
///
/// `TokenManager` wraps an `Authenticator` and provides:
/// - Lazy authentication (authenticates on first token request)
/// - Automatic token refresh when expired
/// - Thread-safe concurrent token access via `RwLock`
/// - Force refresh capability for handling 401 responses
///
/// # Examples
///
/// ```ignore
/// use force::auth::{TokenManager, Authenticator};
///
/// async fn use_token_manager<A: Authenticator>(auth: A) -> Result<()> {
///     let manager = TokenManager::new(auth);
///
///     // Get current token (auto-refreshes if expired)
///     let token = manager.token().await?;
///
///     // Force refresh on 401
///     let new_token = manager.force_refresh().await?;
///
///     Ok(())
/// }
/// ```
use std::sync::Arc;
use tokio::sync::RwLock;

/// Internal state for token management.
#[derive(Debug)]
struct TokenState {
    /// The current access token (if any).
    token: Option<AccessToken>,
}

/// Thread-safe token manager with automatic refresh.
#[derive(Debug)]
pub struct TokenManager<A: crate::auth::Authenticator> {
    /// The authenticator for obtaining tokens.
    authenticator: A,

    /// Thread-safe token state.
    state: Arc<RwLock<TokenState>>,
}

impl<A: crate::auth::Authenticator> TokenManager<A> {
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
        // Fast path: check if current token is valid
        {
            let state = self.state.read().await;
            if let Some(token) = &state.token
                && !token.is_expired()
            {
                return Ok(token.clone());
            }
        } // Read lock dropped here

        // Slow path: refresh or authenticate
        let mut state = self.state.write().await;

        // Double-check after acquiring write lock (another thread might have refreshed)
        if let Some(token) = &state.token
            && !token.is_expired()
        {
            return Ok(token.clone());
        }

        // Token is expired or doesn't exist, refresh it
        let new_token = if state.token.is_some() {
            self.authenticator.refresh().await?
        } else {
            self.authenticator.authenticate().await?
        };

        state.token = Some(new_token.clone());
        Ok(new_token)
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

        {
            let mut state = self.state.write().await;
            state.token = Some(new_token.clone());
        } // Write lock dropped here

        Ok(new_token)
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
use crate::test_support::Must;
    use super::*;
    use crate::auth::Authenticator;
    use async_trait::async_trait;
    use std::sync::Arc as StdArc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_token_response_deserialization() {
        let json = r#"{
            "access_token": "00D123456789!token",
            "instance_url": "https://example.my.salesforce.com",
            "token_type": "Bearer",
            "issued_at": "1704067200000",
            "signature": "signature_value"
        }"#;

        let response: TokenResponse = serde_json::from_str(json).must();
        assert_eq!(response.access_token, "00D123456789!token");
        assert_eq!(response.instance_url, "https://example.my.salesforce.com");
        assert_eq!(response.token_type, "Bearer");
    }

    #[test]
    fn test_token_response_with_expires_in() {
        let json = r#"{
            "access_token": "token123",
            "instance_url": "https://test.salesforce.com",
            "issued_at": "1704067200000",
            "expires_in": 7200
        }"#;

        let response: TokenResponse = serde_json::from_str(json).must();
        assert_eq!(response.expires_in, Some(7200));
        assert_eq!(response.token_type, "Bearer"); // default value
    }

    #[test]
    fn test_access_token_from_response() {
        let response = TokenResponse {
            access_token: "test_token".to_string(),
            instance_url: "https://example.salesforce.com".to_string(),
            token_type: "Bearer".to_string(),
            issued_at: "1704067200000".to_string(),
            signature: String::new(),
            expires_in: Some(3600),
            refresh_token: None,
        };

        let token = AccessToken::from_response(response);
        assert_eq!(token.as_str(), "test_token");
        assert_eq!(token.instance_url(), "https://example.salesforce.com");
        assert_eq!(token.token_type(), "Bearer");
        assert!(token.expires_at().is_some());
    }

    #[test]
    fn test_access_token_is_expired() {
        // Token that expired 1 hour ago
        let expires_at = Utc::now() - Duration::hours(1);
        let token = AccessToken::new(
            "expired_token".to_string(),
            "https://test.salesforce.com".to_string(),
            Some(expires_at),
        );

        assert!(token.is_expired());
    }

    #[test]
    fn test_access_token_not_expired() {
        // Token that expires in 2 hours
        let expires_at = Utc::now() + Duration::hours(2);
        let token = AccessToken::new(
            "valid_token".to_string(),
            "https://test.salesforce.com".to_string(),
            Some(expires_at),
        );

        assert!(!token.is_expired());
    }

    #[test]
    fn test_access_token_expiring_soon() {
        // Token that expires in 30 seconds (within 60s buffer)
        let expires_at = Utc::now() + Duration::seconds(30);
        let token = AccessToken::new(
            "expiring_token".to_string(),
            "https://test.salesforce.com".to_string(),
            Some(expires_at),
        );

        assert!(token.is_expired()); // Should be considered expired due to buffer
    }

    #[test]
    fn test_access_token_no_expiration() {
        // Token without expiration should never be considered expired
        let token = AccessToken::new(
            "no_expiry_token".to_string(),
            "https://test.salesforce.com".to_string(),
            None,
        );

        assert!(!token.is_expired());
    }

    #[test]
    fn test_access_token_custom_buffer() {
        // Token expires in 5 minutes
        let expires_at = Utc::now() + Duration::minutes(5);
        let token = AccessToken::new(
            "token".to_string(),
            "https://test.salesforce.com".to_string(),
            Some(expires_at),
        );

        // Should not be expired with 1 minute buffer
        assert!(!token.is_expired_with_buffer(Duration::minutes(1)));

        // Should be expired with 10 minute buffer
        assert!(token.is_expired_with_buffer(Duration::minutes(10)));
    }

    #[test]
    fn test_parse_issued_at_valid() {
        let timestamp = "1704067200000"; // 2024-01-01 00:00:00 UTC
        let result = parse_issued_at(timestamp);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_issued_at_invalid() {
        let timestamp = "not_a_number";
        let result = parse_issued_at(timestamp);
        assert!(result.is_err());
    }

    // Mock authenticator for testing TokenManager
    #[derive(Debug)]
    struct MockAuthenticator {
        auth_count: StdArc<AtomicUsize>,
        refresh_count: StdArc<AtomicUsize>,
        should_fail: bool,
    }

    impl MockAuthenticator {
        fn new() -> Self {
            Self {
                auth_count: StdArc::new(AtomicUsize::new(0)),
                refresh_count: StdArc::new(AtomicUsize::new(0)),
                should_fail: false,
            }
        }

        fn with_failure() -> Self {
            Self {
                auth_count: StdArc::new(AtomicUsize::new(0)),
                refresh_count: StdArc::new(AtomicUsize::new(0)),
                should_fail: true,
            }
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
                *token = AccessToken::new(
                    "expired_token".to_string(),
                    "https://test.salesforce.com".to_string(),
                    Some(Utc::now() - Duration::hours(1)),
                );
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
            state.token = Some(AccessToken::new(
                "expired".to_string(),
                "https://test.salesforce.com".to_string(),
                Some(Utc::now() - Duration::hours(1)),
            ));
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
}
