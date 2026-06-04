//! OAuth 2.0 Client Credentials Flow implementation.
//!
//! This module provides the `ClientCredentials` authenticator for server-to-server
//! authentication with Salesforce using the OAuth 2.0 Client Credentials grant type.
//!
//! # OAuth 2.0 Client Credentials Flow
//!
//! The client credentials flow is designed for machine-to-machine authentication where
//! a client application needs to access its own resources or resources it has been
//! authorized to access.
//!
//! ## Flow Overview
//!
//! 1. Client sends client_id and client_secret to token endpoint
//! 2. Salesforce validates credentials
//! 3. Salesforce returns access token (no refresh token)
//!
//! ## Security Considerations
//!
//! - Client secrets must be stored securely (use `secrecy` crate)
//! - HTTPS is required for token endpoint communication
//! - Access tokens should be cached and reused until expiration
//!
//! # Examples
//!
//! ```ignore
//! use force::auth::ClientCredentials;
//!
//! let auth = ClientCredentials::new_my_domain(
//!     "your_client_id",
//!     "your_client_secret",
//!     "https://your-org.my.salesforce.com",
//! );
//!
//! let token = auth.authenticate().await?;
//! ```

use crate::auth::token::{AccessToken, TokenResponse};
use crate::error::{ForceError, HttpError, Result};
use async_trait::async_trait;
use secrecy::{ExposeSecret, SecretString};

/// OAuth 2.0 Client Credentials authenticator.
///
/// Implements the client credentials grant type for server-to-server authentication.
/// This flow does not support refresh tokens - when the access token expires,
/// the client must re-authenticate.
#[derive(Debug, Clone)]
pub struct ClientCredentials {
    /// OAuth client ID from Salesforce Connected App.
    client_id: String,

    /// OAuth client secret (securely stored).
    client_secret: SecretString,

    /// Token endpoint URL (varies by environment).
    token_url: String,

    /// HTTP client for making requests.
    client: reqwest::Client,
}

impl ClientCredentials {
    /// Creates a new `ClientCredentials` authenticator.
    ///
    /// # Arguments
    ///
    /// * `client_id` - OAuth client ID from Connected App
    /// * `client_secret` - OAuth client secret from Connected App
    /// * `token_url` - Token endpoint URL (for example,
    ///   `https://my-org.my.salesforce.com/services/oauth2/token`)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let auth = ClientCredentials::new(
    ///     "3MVG9...",
    ///     "1234567890...",
    ///     "https://my-org.my.salesforce.com/services/oauth2/token",
    /// );
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the default HTTP client cannot be initialized (e.g., due to missing TLS backend).
    pub fn new(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        token_url: impl Into<String>,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: SecretString::new(client_secret.into().into()),
            token_url: token_url.into(),
            client: crate::auth::default_auth_http_client(),
        }
    }

    /// Sets a custom HTTP client.
    ///
    /// This allows configuring timeouts, proxies, or certificates.
    #[must_use]
    pub fn with_client(mut self, client: reqwest::Client) -> Self {
        self.client = client;
        self
    }

    /// Creates a `ClientCredentials` authenticator from a Salesforce My Domain URL.
    ///
    /// Use this when your org or sandbox expects OAuth token requests through
    /// its My Domain host.
    ///
    /// # Arguments
    ///
    /// * `client_id` - OAuth client ID from Connected App
    /// * `client_secret` - OAuth client secret from Connected App
    /// * `my_domain_url` - Org My Domain base URL, for example
    ///   `https://my-org.my.salesforce.com`
    pub fn new_my_domain(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        my_domain_url: impl AsRef<str>,
    ) -> Self {
        let token_url = format!(
            "{}/services/oauth2/token",
            my_domain_url.as_ref().trim_end_matches('/')
        );
        Self::new(client_id, client_secret, token_url)
    }

    /// Creates a new `ClientCredentials` authenticator for the global production login URL.
    ///
    /// Uses the standard Salesforce production token URL:
    /// `https://login.salesforce.com/services/oauth2/token`
    ///
    /// If Salesforce returns a domain-support error for this flow, use
    /// [`Self::new_my_domain`] with the target org's My Domain URL.
    ///
    /// # Arguments
    ///
    /// * `client_id` - OAuth client ID from Connected App
    /// * `client_secret` - OAuth client secret from Connected App
    pub fn new_production(client_id: impl Into<String>, client_secret: impl Into<String>) -> Self {
        Self::new(client_id, client_secret, crate::auth::PRODUCTION_TOKEN_URL)
    }

    /// Creates a new `ClientCredentials` authenticator for the global sandbox login URL.
    ///
    /// Uses the standard Salesforce sandbox token URL:
    /// `https://test.salesforce.com/services/oauth2/token`
    ///
    /// If Salesforce returns a domain-support error for this flow, use
    /// [`Self::new_my_domain`] with the target sandbox My Domain URL.
    ///
    /// # Arguments
    ///
    /// * `client_id` - OAuth client ID from Connected App
    /// * `client_secret` - OAuth client secret from Connected App
    pub fn new_sandbox(client_id: impl Into<String>, client_secret: impl Into<String>) -> Self {
        Self::new(client_id, client_secret, crate::auth::SANDBOX_TOKEN_URL)
    }

    /// Returns the OAuth 2.0 grant type for this flow.
    pub fn grant_type(&self) -> &'static str {
        "client_credentials"
    }
}

#[async_trait]
impl crate::auth::authenticator::Authenticator for ClientCredentials {
    async fn authenticate(&self) -> Result<AccessToken> {
        // Build form parameters for token request
        let params = [
            ("grant_type", self.grant_type()),
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.expose_secret()),
        ];

        // Make POST request to token endpoint
        let response = self
            .client
            .post(&self.token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| ForceError::Http(HttpError::RequestFailed(e)))?;

        if !response.status().is_success() {
            return Err(crate::auth::handle_oauth_error(response, None).await);
        }

        // Parse successful token response
        let bytes = crate::http::error::read_capped_body_bytes(response, 1024 * 1024).await?;
        let token_response = serde_json::from_slice::<TokenResponse>(&bytes)
            .map_err(crate::error::SerializationError::from)?;

        Ok(AccessToken::from_response(token_response))
    }

    async fn refresh(&self) -> Result<AccessToken> {
        // Client credentials flow does not support refresh tokens.
        // We must re-authenticate to get a new access token.
        self.authenticate().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "mock")]
    use crate::auth::Authenticator;
    #[cfg(feature = "mock")]
    use crate::error::AuthenticationError;
    #[cfg(feature = "mock")]
    use crate::test_utils::must::Must;

    // RED PHASE - Write failing tests first

    #[test]
    fn test_client_credentials_new() {
        let auth = ClientCredentials::new(
            "test_client_id",
            "test_client_secret",
            "https://example.my.salesforce.com/services/oauth2/token",
        );

        assert_eq!(auth.client_id, "test_client_id");
        assert_eq!(
            auth.token_url,
            "https://example.my.salesforce.com/services/oauth2/token"
        );
    }

    #[test]
    fn test_client_credentials_new_my_domain_builds_token_url() {
        let auth = ClientCredentials::new_my_domain(
            "test_client_id",
            "test_client_secret",
            "https://example.my.salesforce.com/",
        );

        assert_eq!(auth.client_id, "test_client_id");
        assert_eq!(
            auth.token_url,
            "https://example.my.salesforce.com/services/oauth2/token"
        );
    }

    #[test]
    fn test_with_client_configures_custom_client() {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(42))
            .build()
            .unwrap_or_else(|_| panic!("msg"));
        let auth =
            ClientCredentials::new_production("client_id", "client_secret").with_client(client);
        assert_eq!(auth.client_id, "client_id");
    }

    #[test]
    fn test_grant_type() {
        let auth = ClientCredentials::new(
            "client_id",
            "client_secret",
            "https://example.my.salesforce.com/services/oauth2/token",
        );

        assert_eq!(auth.grant_type(), "client_credentials");
    }

    #[test]
    fn test_client_secret_is_secret() {
        let auth = ClientCredentials::new(
            "client_id",
            "my_secret",
            "https://login.salesforce.com/services/oauth2/token",
        );

        // Verify secret is properly wrapped
        assert_eq!(auth.client_secret.expose_secret(), "my_secret");

        // Debug output should not reveal the secret
        let debug_output = format!("{:?}", auth);
        assert!(!debug_output.contains("my_secret"));
    }

    // Integration tests with wiremock

    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn test_authenticate_success_with_wiremock() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        let token_response = serde_json::json!({
            "access_token": "00Dxx0000001gPL!test_token",
            "instance_url": "https://test.my.salesforce.com",
            "id": "https://login.salesforce.com/id/00Dxx0000001gPL/005xx000001Swi",
            "token_type": "Bearer",
            "issued_at": "1704067200000",
            "signature": "testSignature=="
        });

        Mock::given(method("POST"))
            .and(path("/services/oauth2/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(token_response))
            .mount(&mock_server)
            .await;

        let auth = ClientCredentials::new(
            "test_client_id",
            "test_client_secret",
            format!("{}/services/oauth2/token", mock_server.uri()),
        );

        let token = auth.authenticate().await.must();
        assert_eq!(token.as_str(), "00Dxx0000001gPL!test_token");
        assert_eq!(token.instance_url(), "https://test.my.salesforce.com");
        assert_eq!(token.token_type(), "Bearer");
    }

    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn test_authenticate_invalid_credentials_with_wiremock() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        let error_response = serde_json::json!({
            "error": "invalid_client_id",
            "error_description": "client identifier invalid"
        });

        Mock::given(method("POST"))
            .and(path("/services/oauth2/token"))
            .respond_with(ResponseTemplate::new(400).set_body_json(error_response))
            .mount(&mock_server)
            .await;

        let auth = ClientCredentials::new(
            "invalid_client_id",
            "invalid_secret",
            format!("{}/services/oauth2/token", mock_server.uri()),
        );

        let result = auth.authenticate().await;

        if let Err(ForceError::Authentication(AuthenticationError::TokenRequestFailed(msg))) =
            result
        {
            assert!(msg.contains("invalid_client_id"));
            assert!(msg.contains("client identifier invalid"));
        } else {
            panic!("Expected TokenRequestFailed error");
        }
    }

    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn test_refresh_calls_authenticate_with_wiremock() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        let token_response = serde_json::json!({
            "access_token": "refreshed_token",
            "instance_url": "https://test.salesforce.com",
            "token_type": "Bearer",
            "issued_at": "1704067200000",
            "signature": "sig=="
        });

        Mock::given(method("POST"))
            .and(path("/services/oauth2/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(token_response))
            .expect(2) // Should be called twice: authenticate + refresh
            .mount(&mock_server)
            .await;

        let auth = ClientCredentials::new(
            "test_client",
            "test_secret",
            format!("{}/services/oauth2/token", mock_server.uri()),
        );

        // First authenticate
        let _token1 = auth.authenticate().await.must();

        // Then refresh (should call authenticate again since client_credentials doesn't support refresh)
        let token2 = auth.refresh().await.must();
        assert_eq!(token2.as_str(), "refreshed_token");
    }

    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn test_authenticate_network_error() {
        // Use an invalid URL to trigger network error
        let auth = ClientCredentials::new(
            "test_client",
            "test_secret",
            "http://invalid.invalid.localhost:99999/oauth2/token",
        );

        let result = auth.authenticate().await;

        assert!(matches!(result, Err(ForceError::Http(_))));
    }

    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn test_authenticate_error_payload_too_large() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        // Generate a payload that exceeds the limit
        // (1024 * 1024 + 1024 to intentionally break boundary false confidence)
        let large_body = "A".repeat(1024 * 1024 + 1024);

        Mock::given(method("POST"))
            .and(path("/services/oauth2/token"))
            .respond_with(ResponseTemplate::new(400).set_body_string(large_body))
            .mount(&mock_server)
            .await;

        let auth = ClientCredentials::new(
            "test_client",
            "test_secret",
            format!("{}/services/oauth2/token", mock_server.uri()),
        );

        let result = auth.authenticate().await;

        if let Err(ForceError::Http(HttpError::PayloadTooLarge { limit_bytes })) = result {
            assert_eq!(limit_bytes, 1024 * 1024);
        } else {
            panic!("Expected PayloadTooLarge error");
        }
    }

    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn test_authenticate_http_error_without_oauth_response() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/oauth2/token"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock_server)
            .await;

        let auth = ClientCredentials::new(
            "test_client",
            "test_secret",
            format!("{}/services/oauth2/token", mock_server.uri()),
        );

        let result = auth.authenticate().await;

        if let Err(ForceError::Http(HttpError::StatusError {
            status_code,
            message,
        })) = result
        {
            assert_eq!(status_code, 500);
            assert!(message.contains("Internal Server Error"));
        } else {
            panic!("Expected HttpError::StatusError");
        }
    }
}
