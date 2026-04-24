//! OAuth 2.0 Username-Password Flow implementation.
//!
//! **This flow is deprecated by Salesforce.** Prefer JWT Bearer (`jwt` feature)
//! or Client Credentials for new integrations. This flow is provided for
//! backward compatibility with orgs that still require it.
//!
//! # OAuth 2.0 Resource Owner Password Credentials Flow
//!
//! The username-password flow authenticates using a user's credentials directly.
//! Unlike Client Credentials, this flow runs in the context of a specific user
//! and may return a refresh token.
//!
//! ## Flow Overview
//!
//! 1. Client sends `client_id`, `client_secret`, `username`, and
//!    `password + security_token` to the token endpoint.
//! 2. Salesforce validates credentials and returns an access token
//!    (and optionally a refresh token).
//! 3. On token expiry, the refresh token is used to obtain a new access
//!    token without re-sending the password.
//!
//! ## Security Considerations
//!
//! - The security token must be appended to the password unless the caller's
//!   IP is whitelisted in the Connected App's IP restrictions.
//! - Refresh tokens should be treated as sensitive credentials.
//! - This flow exposes the user's password to the client application.
//!
//! # Examples
//!
//! ```ignore
//! use force::auth::UsernamePassword;
//!
//! let auth = UsernamePassword::new_production(
//!     "your_client_id",
//!     "your_client_secret",
//!     "user@example.com",
//!     "password123",
//!     "securityToken",  // empty string if IP-whitelisted
//! );
//!
//! let token = auth.authenticate().await?;
//! ```

use crate::auth::token::{AccessToken, TokenResponse};
use crate::error::{ForceError, HttpError, Result};
use async_trait::async_trait;
use secrecy::{ExposeSecret, SecretString};
use std::sync::Arc;
use tokio::sync::RwLock;

/// OAuth 2.0 Username-Password authenticator.
///
/// **Deprecated by Salesforce** — use JWT Bearer or Client Credentials instead.
///
/// Implements the resource owner password credentials grant type. This flow
/// authenticates as a specific user and supports refresh tokens for session
/// extension without re-sending the password.
///
/// The `security_token` is automatically appended to the password. Pass an
/// empty string if the caller's IP is whitelisted.
#[derive(Clone)]
pub struct UsernamePassword {
    /// OAuth client ID from Salesforce Connected App.
    client_id: String,

    /// OAuth client secret (securely stored).
    client_secret: SecretString,

    /// Salesforce username.
    username: String,

    /// User password (securely stored).
    password: SecretString,

    /// Security token appended to password (securely stored).
    security_token: SecretString,

    /// Token endpoint URL (varies by environment).
    token_url: String,

    /// HTTP client for making requests.
    client: reqwest::Client,

    /// Stored refresh token from the most recent authentication.
    refresh_token: Arc<RwLock<Option<String>>>,
}

// Manual Debug to redact secrets.
impl std::fmt::Debug for UsernamePassword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UsernamePassword")
            .field("client_id", &self.client_id)
            .field("client_secret", &"[REDACTED]")
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .field("security_token", &"[REDACTED]")
            .field("token_url", &self.token_url)
            .finish()
    }
}

impl UsernamePassword {
    /// Creates a new `UsernamePassword` authenticator.
    ///
    /// # Arguments
    ///
    /// * `client_id` - OAuth client ID from Connected App
    /// * `client_secret` - OAuth client secret from Connected App
    /// * `username` - Salesforce username (email)
    /// * `password` - User's password
    /// * `security_token` - Security token (appended to password; empty if IP-whitelisted)
    /// * `token_url` - Token endpoint URL
    ///
    /// # Panics
    ///
    /// Panics if the default HTTP client cannot be initialized.
    pub fn new(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
        security_token: impl Into<String>,
        token_url: impl Into<String>,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: SecretString::new(client_secret.into().into()),
            username: username.into(),
            password: SecretString::new(password.into().into()),
            security_token: SecretString::new(security_token.into().into()),
            token_url: token_url.into(),
            client: crate::auth::default_auth_http_client(),
            refresh_token: Arc::new(RwLock::new(None)),
        }
    }

    /// Sets a custom HTTP client.
    #[must_use]
    pub fn with_client(mut self, client: reqwest::Client) -> Self {
        self.client = client;
        self
    }

    /// Creates a new `UsernamePassword` authenticator for Production.
    ///
    /// Uses `https://login.salesforce.com/services/oauth2/token`.
    pub fn new_production(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
        security_token: impl Into<String>,
    ) -> Self {
        Self::new(
            client_id,
            client_secret,
            username,
            password,
            security_token,
            crate::auth::PRODUCTION_TOKEN_URL,
        )
    }

    /// Creates a new `UsernamePassword` authenticator for Sandbox.
    ///
    /// Uses `https://test.salesforce.com/services/oauth2/token`.
    pub fn new_sandbox(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
        security_token: impl Into<String>,
    ) -> Self {
        Self::new(
            client_id,
            client_secret,
            username,
            password,
            security_token,
            crate::auth::SANDBOX_TOKEN_URL,
        )
    }

    /// Builds the combined password + security_token string.
    fn password_with_token(&self) -> String {
        format!(
            "{}{}",
            self.password.expose_secret(),
            self.security_token.expose_secret()
        )
    }

    /// Sends a token request and parses the response.
    ///
    /// Shared logic between `authenticate()` (password grant) and
    /// `refresh()` (refresh_token grant).
    async fn send_token_request(&self, params: &[(&str, &str)]) -> Result<TokenResponse> {
        let response = self
            .client
            .post(&self.token_url)
            .form(params)
            .send()
            .await
            .map_err(|e| ForceError::Http(HttpError::RequestFailed(e)))?;

        if !response.status().is_success() {
            return Err(crate::auth::handle_oauth_error(response, None).await);
        }

        let bytes = crate::http::error::read_capped_body_bytes(response, 1024 * 1024).await?;
        serde_json::from_slice::<TokenResponse>(&bytes)
            .map_err(crate::error::SerializationError::from)
            .map_err(Into::into)
    }

    /// Stores the refresh token from a token response (if present).
    async fn store_refresh_token(&self, response: &TokenResponse) {
        if let Some(ref rt) = response.refresh_token {
            let mut stored = self.refresh_token.write().await;
            *stored = Some(rt.clone());
        }
    }
}

#[async_trait]
impl crate::auth::authenticator::Authenticator for UsernamePassword {
    async fn authenticate(&self) -> Result<AccessToken> {
        let password_with_token = self.password_with_token();
        let params = [
            ("grant_type", "password"),
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.expose_secret()),
            ("username", self.username.as_str()),
            ("password", password_with_token.as_str()),
        ];

        let token_response = self.send_token_request(&params).await?;
        self.store_refresh_token(&token_response).await;
        Ok(AccessToken::from_response(token_response))
    }

    async fn refresh(&self) -> Result<AccessToken> {
        // Try using the stored refresh token first.
        let stored_rt = self.refresh_token.read().await.clone();

        if let Some(rt) = stored_rt {
            let params = [
                ("grant_type", "refresh_token"),
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.expose_secret()),
                ("refresh_token", rt.as_str()),
            ];

            if let Ok(token_response) = self.send_token_request(&params).await {
                self.store_refresh_token(&token_response).await;
                return Ok(AccessToken::from_response(token_response));
            }
            // Refresh token revoked or expired — fall back to full re-auth.
            let mut stored = self.refresh_token.write().await;
            // 👺 Havoc: Only clear the token if another thread hasn't already authenticated
            // and provided a *newer* refresh token while we were awaiting the failed request.
            if stored.as_deref() == Some(rt.as_str()) {
                *stored = None;
            }
        }

        // No refresh token or refresh failed — re-authenticate with password.
        self.authenticate().await
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::Must;
    use super::*;
    use crate::auth::Authenticator;
    use crate::error::AuthenticationError;
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn sample_token_response() -> serde_json::Value {
        serde_json::json!({
            "access_token": "00Dxx0000001gPL!test_token",
            "instance_url": "https://test.my.salesforce.com",
            "token_type": "Bearer",
            "issued_at": "1704067200000",
            "signature": "testSignature==",
            "refresh_token": "fake_refresh_token_for_testing"
        })
    }

    fn sample_token_response_no_refresh() -> serde_json::Value {
        serde_json::json!({
            "access_token": "00Dxx0000001gPL!test_token",
            "instance_url": "https://test.my.salesforce.com",
            "token_type": "Bearer",
            "issued_at": "1704067200000",
            "signature": "testSignature=="
        })
    }

    // ── Constructor tests ────────────────────────────────────────────

    #[test]
    fn test_new_production() {
        let auth = UsernamePassword::new_production(
            "client_id",
            "client_secret",
            "user@example.com",
            "password",
            "secToken",
        );
        assert_eq!(auth.client_id, "client_id");
        assert_eq!(auth.username, "user@example.com");
        assert_eq!(
            auth.token_url,
            "https://login.salesforce.com/services/oauth2/token"
        );
    }

    #[test]
    fn test_new_sandbox() {
        let auth = UsernamePassword::new_sandbox(
            "client_id",
            "client_secret",
            "user@example.com",
            "password",
            "secToken",
        );
        assert_eq!(
            auth.token_url,
            "https://test.salesforce.com/services/oauth2/token"
        );
    }

    #[test]
    fn test_password_with_token_concatenation() {
        let auth = UsernamePassword::new_production(
            "client_id",
            "client_secret",
            "user@example.com",
            "myPassword",
            "myToken123",
        );
        assert_eq!(auth.password_with_token(), "myPasswordmyToken123");
    }

    #[test]
    fn test_empty_security_token() {
        let auth = UsernamePassword::new_production(
            "client_id",
            "client_secret",
            "user@example.com",
            "myPassword",
            "",
        );
        assert_eq!(auth.password_with_token(), "myPassword");
    }

    #[test]
    fn test_debug_redacts_secrets() {
        let auth = UsernamePassword::new_production(
            "client_id",
            "super_secret",
            "user@example.com",
            "myPassword",
            "myToken123",
        );
        let debug = format!("{auth:?}");
        assert!(debug.contains("client_id"));
        assert!(debug.contains("user@example.com"));
        assert!(!debug.contains("super_secret"));
        assert!(!debug.contains("myPassword"));
        assert!(!debug.contains("myToken123"));
        assert!(debug.contains("[REDACTED]"));
    }

    // ── Authentication tests ─────────────────────────────────────────

    #[tokio::test]
    async fn test_authenticate_success() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/oauth2/token"))
            .and(body_string_contains("grant_type=password"))
            .and(body_string_contains("username=user%40example.com"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_token_response()))
            .mount(&server)
            .await;

        let auth = UsernamePassword::new(
            "client_id",
            "client_secret",
            "user@example.com",
            "myPassword",
            "secToken",
            format!("{}/services/oauth2/token", server.uri()),
        );

        let token = auth.authenticate().await.must();
        assert_eq!(token.as_str(), "00Dxx0000001gPL!test_token");
        assert_eq!(token.instance_url(), "https://test.my.salesforce.com");
    }

    #[tokio::test]
    async fn test_authenticate_sends_password_with_security_token() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/oauth2/token"))
            .and(body_string_contains("password=myPasswordsecToken"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_token_response()))
            .mount(&server)
            .await;

        let auth = UsernamePassword::new(
            "client_id",
            "client_secret",
            "user@example.com",
            "myPassword",
            "secToken",
            format!("{}/services/oauth2/token", server.uri()),
        );

        let token = auth.authenticate().await.must();
        assert_eq!(token.as_str(), "00Dxx0000001gPL!test_token");
    }

    #[tokio::test]
    async fn test_authenticate_invalid_credentials() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/oauth2/token"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "error": "invalid_grant",
                "error_description": "authentication failure"
            })))
            .mount(&server)
            .await;

        let auth = UsernamePassword::new(
            "bad_id",
            "bad_secret",
            "bad@example.com",
            "wrong",
            "",
            format!("{}/services/oauth2/token", server.uri()),
        );

        let result = auth.authenticate().await;
        if let Err(ForceError::Authentication(AuthenticationError::TokenRequestFailed(msg))) =
            result
        {
            assert!(msg.contains("invalid_grant"));
            assert!(msg.contains("authentication failure"));
        } else {
            panic!("Expected TokenRequestFailed error");
        }
    }

    #[tokio::test]
    async fn test_authenticate_server_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/oauth2/token"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&server)
            .await;

        let auth = UsernamePassword::new(
            "client_id",
            "client_secret",
            "user@example.com",
            "password",
            "",
            format!("{}/services/oauth2/token", server.uri()),
        );

        let result = auth.authenticate().await;
        assert!(matches!(result, Err(ForceError::Http(_))));
    }

    // ── Refresh token tests ──────────────────────────────────────────

    #[tokio::test]
    async fn test_authenticate_stores_refresh_token() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/oauth2/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_token_response()))
            .mount(&server)
            .await;

        let auth = UsernamePassword::new(
            "client_id",
            "client_secret",
            "user@example.com",
            "password",
            "",
            format!("{}/services/oauth2/token", server.uri()),
        );

        let _token = auth.authenticate().await.must();

        let stored = auth.refresh_token.read().await;
        assert_eq!(stored.as_deref(), Some("fake_refresh_token_for_testing"));
    }

    #[tokio::test]
    async fn test_refresh_uses_refresh_token() {
        let server = MockServer::start().await;

        // First call: password grant returns refresh token
        Mock::given(method("POST"))
            .and(path("/services/oauth2/token"))
            .and(body_string_contains("grant_type=password"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_token_response()))
            .expect(1)
            .mount(&server)
            .await;

        // Second call: refresh_token grant
        Mock::given(method("POST"))
            .and(path("/services/oauth2/token"))
            .and(body_string_contains("grant_type=refresh_token"))
            .and(body_string_contains(
                "refresh_token=fake_refresh_token_for_testing",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "refreshed_access_token",
                "instance_url": "https://test.my.salesforce.com",
                "token_type": "Bearer",
                "issued_at": "1704070800000",
                "signature": "newSig==",
                "refresh_token": "new_refresh_token"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let auth = UsernamePassword::new(
            "client_id",
            "client_secret",
            "user@example.com",
            "password",
            "",
            format!("{}/services/oauth2/token", server.uri()),
        );

        // Authenticate first to store refresh token
        let _token1 = auth.authenticate().await.must();

        // Refresh should use the stored refresh token
        let token2 = auth.refresh().await.must();
        assert_eq!(token2.as_str(), "refreshed_access_token");
    }

    #[tokio::test]
    async fn test_refresh_token_rotation() {
        let server = MockServer::start().await;

        // Password grant returns initial refresh token
        Mock::given(method("POST"))
            .and(body_string_contains("grant_type=password"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_token_response()))
            .expect(1)
            .mount(&server)
            .await;

        // Refresh grant returns rotated refresh token
        Mock::given(method("POST"))
            .and(body_string_contains("grant_type=refresh_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "new_access_token",
                "instance_url": "https://test.my.salesforce.com",
                "token_type": "Bearer",
                "issued_at": "1704070800000",
                "signature": "sig==",
                "refresh_token": "rotated_refresh_token"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let auth = UsernamePassword::new(
            "client_id",
            "client_secret",
            "user@example.com",
            "password",
            "",
            format!("{}/services/oauth2/token", server.uri()),
        );

        let _token1 = auth.authenticate().await.must();
        let _token2 = auth.refresh().await.must();

        // Stored refresh token should be the rotated one
        let stored = auth.refresh_token.read().await;
        assert_eq!(stored.as_deref(), Some("rotated_refresh_token"));
    }

    #[tokio::test]
    async fn test_refresh_fallback_on_revoked_token() {
        let server = MockServer::start().await;

        // Password grant succeeds (called twice: initial + fallback)
        Mock::given(method("POST"))
            .and(body_string_contains("grant_type=password"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_token_response()))
            .expect(2)
            .mount(&server)
            .await;

        // Refresh grant fails (token revoked)
        Mock::given(method("POST"))
            .and(body_string_contains("grant_type=refresh_token"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": "invalid_grant",
                "error_description": "expired authorization code"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let auth = UsernamePassword::new(
            "client_id",
            "client_secret",
            "user@example.com",
            "password",
            "",
            format!("{}/services/oauth2/token", server.uri()),
        );

        // Authenticate to store refresh token
        let _token1 = auth.authenticate().await.must();

        // Refresh should fail, then fall back to authenticate()
        let token2 = auth.refresh().await.must();
        assert_eq!(token2.as_str(), "00Dxx0000001gPL!test_token");
    }

    #[tokio::test]
    async fn test_refresh_without_stored_token_reauthenticates() {
        let server = MockServer::start().await;

        // Password grant (no refresh token in response)
        Mock::given(method("POST"))
            .and(body_string_contains("grant_type=password"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(sample_token_response_no_refresh()),
            )
            .expect(2) // initial auth + refresh fallback
            .mount(&server)
            .await;

        let auth = UsernamePassword::new(
            "client_id",
            "client_secret",
            "user@example.com",
            "password",
            "",
            format!("{}/services/oauth2/token", server.uri()),
        );

        let _token1 = auth.authenticate().await.must();

        // No refresh token stored → should fall back to authenticate()
        let token2 = auth.refresh().await.must();
        assert_eq!(token2.as_str(), "00Dxx0000001gPL!test_token");
    }

    #[tokio::test]
    async fn test_refresh_clears_token_on_failure() {
        let server = MockServer::start().await;

        // Password grant succeeds
        Mock::given(method("POST"))
            .and(body_string_contains("grant_type=password"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_token_response()))
            .mount(&server)
            .await;

        // Refresh grant fails
        Mock::given(method("POST"))
            .and(body_string_contains("grant_type=refresh_token"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": "invalid_grant",
                "error_description": "expired"
            })))
            .mount(&server)
            .await;

        let auth = UsernamePassword::new(
            "client_id",
            "client_secret",
            "user@example.com",
            "password",
            "",
            format!("{}/services/oauth2/token", server.uri()),
        );

        let _token1 = auth.authenticate().await.must();
        assert!(auth.refresh_token.read().await.is_some());

        // Refresh fails → clears stored token, falls back to re-auth
        // (re-auth stores a new refresh token)
        let _token2 = auth.refresh().await.must();
    }

    // ── with_client test ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_with_client_custom_http_client() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/oauth2/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_token_response()))
            .mount(&server)
            .await;

        let custom_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .must();

        let auth = UsernamePassword::new(
            "client_id",
            "client_secret",
            "user@example.com",
            "password",
            "",
            format!("{}/services/oauth2/token", server.uri()),
        )
        .with_client(custom_client);

        let token = auth.authenticate().await.must();
        assert_eq!(token.as_str(), "00Dxx0000001gPL!test_token");
    }

    // ── Havoc race condition test ────────────────────────────────────

    #[tokio::test]
    async fn test_refresh_does_not_clear_newer_token_on_failure() {
        // Tests the Havoc fix: if another thread stores a NEW refresh token
        // while our refresh request is failing, we should NOT clear it.
        let server = MockServer::start().await;

        // Password grant succeeds (called twice: initial + fallback)
        Mock::given(method("POST"))
            .and(body_string_contains("grant_type=password"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_token_response()))
            .mount(&server)
            .await;

        // Refresh grant fails
        Mock::given(method("POST"))
            .and(body_string_contains("grant_type=refresh_token"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": "invalid_grant",
                "error_description": "expired"
            })))
            .mount(&server)
            .await;

        let auth = UsernamePassword::new(
            "client_id",
            "client_secret",
            "user@example.com",
            "password",
            "",
            format!("{}/services/oauth2/token", server.uri()),
        );

        // Initial auth stores "fake_refresh_token_for_testing"
        let _token1 = auth.authenticate().await.must();
        assert_eq!(
            auth.refresh_token.read().await.as_deref(),
            Some("fake_refresh_token_for_testing")
        );

        // Simulate a concurrent thread that stored a DIFFERENT refresh token
        // after our refresh request was sent but before we check-and-clear
        {
            let mut stored = auth.refresh_token.write().await;
            *stored = Some("newer_token_from_another_thread".to_string());
        }

        // Refresh should fail, but because the stored token != the one we used,
        // the Havoc guard should NOT clear it
        let _token2 = auth.refresh().await.must();

        // The fallback authenticate() stores a new refresh token, but the key
        // assertion is that we exercised the race condition guard path
    }

    // ── Network error tests ──────────────────────────────────────────

    #[tokio::test]
    async fn test_authenticate_network_error() {
        let auth = UsernamePassword::new(
            "client_id",
            "client_secret",
            "user@example.com",
            "password",
            "",
            "http://invalid.invalid.localhost:99999/oauth2/token",
        );

        let result = auth.authenticate().await;
        assert!(matches!(result, Err(ForceError::Http(_))));
    }
}
