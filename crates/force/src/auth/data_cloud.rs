//! Data Cloud token exchange authentication.
//!
//! This module provides `DataCloudAuthenticator`, a decorator that wraps any
//! platform [`Authenticator`] and performs the two-step token exchange required
//! by Salesforce Data Cloud:
//!
//! 1. Obtain a standard Salesforce platform token (via the inner authenticator)
//! 2. Exchange it for a Data Cloud–specific token via `/services/a360/token`
//!
//! The resulting DC token targets a separate tenant URL and is managed
//! independently by its own [`TokenManager`](crate::auth::TokenManager).
//!
//! # Feature Flag
//!
//! This module requires the `data_cloud` feature flag:
//! ```toml
//! [dependencies]
//! force = { version = "...", features = ["data_cloud"] }
//! ```

use crate::auth::authenticator::Authenticator;
use crate::auth::token::{AccessToken, TokenResponse};
use crate::auth::token_manager::TokenManager;
use crate::error::{ForceError, HttpError, Result};
use async_trait::async_trait;
use serde::Deserialize;
use std::fmt;
use std::sync::Arc;

// ─── Configuration ───────────────────────────────────────────────────────────

/// Configuration for Data Cloud API access.
///
/// Controls how the token exchange is performed and which API version
/// is used for Data Cloud calls.
///
/// # Examples
///
/// ```ignore
/// use force::auth::DataCloudConfig;
///
/// // Use defaults (exchange URL derived from instance, platform API version)
/// let config = DataCloudConfig::default();
///
/// // Override the exchange URL (e.g., for testing)
/// let config = DataCloudConfig {
///     token_exchange_url: Some("https://custom.salesforce.com/services/a360/token".into()),
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Default)]
pub struct DataCloudConfig {
    /// Override for the token exchange URL.
    ///
    /// Default: `{instance_url}/services/a360/token` (derived at exchange time
    /// from the platform token's `instance_url`).
    pub token_exchange_url: Option<String>,

    /// API version override for Data Cloud API calls.
    ///
    /// Default: uses the platform `ClientConfig::api_version`.
    pub api_version: Option<String>,
}

// ─── Token Exchange Response ─────────────────────────────────────────────────

/// Raw JSON response from the Data Cloud token exchange endpoint.
///
/// Returned by `POST /services/a360/token`.
#[derive(Debug, Clone, Deserialize)]
struct DataCloudTokenResponse {
    /// The Data Cloud access token.
    pub access_token: String,

    /// The Data Cloud tenant instance URL (e.g., `https://tenant.c360a.salesforce.com`).
    pub instance_url: String,

    /// Token type (typically `"Bearer"`).
    #[serde(default = "crate::auth::token::default_token_type")]
    pub token_type: String,

    /// Token lifetime in seconds.
    #[serde(default)]
    pub expires_in: Option<u64>,
}

impl DataCloudTokenResponse {
    /// Converts this DC-specific response into the standard `TokenResponse`
    /// expected by [`AccessToken::from_response`].
    fn into_token_response(self) -> TokenResponse {
        TokenResponse {
            access_token: self.access_token,
            instance_url: self.instance_url,
            token_type: self.token_type,
            issued_at: chrono::Utc::now().timestamp_millis().to_string(),
            signature: String::new(),
            expires_in: self.expires_in,
            refresh_token: None,
        }
    }
}

// ─── Data Cloud Authenticator ────────────────────────────────────────────────

/// Decorator authenticator that performs the Data Cloud token exchange.
///
/// Wraps an existing platform [`Authenticator`] and shares its
/// [`TokenManager`] to obtain platform tokens for the exchange step.
///
/// # Type Parameters
///
/// * `A` — The underlying platform authenticator type.
pub struct DataCloudAuthenticator<A: Authenticator> {
    /// Shared platform token manager (same instance used by the platform session).
    platform_token_manager: Arc<TokenManager<A>>,

    /// HTTP client for the exchange request (shared with the platform session).
    http_client: reqwest::Client,

    /// Data Cloud configuration.
    config: DataCloudConfig,
}

impl<A: Authenticator> DataCloudAuthenticator<A> {
    /// Creates a new Data Cloud authenticator.
    ///
    /// # Arguments
    ///
    /// * `platform_token_manager` — Shared reference to the platform token manager.
    /// * `http_client` — HTTP client for the exchange request.
    /// * `config` — Data Cloud configuration.
    pub(crate) fn new(
        platform_token_manager: Arc<TokenManager<A>>,
        http_client: reqwest::Client,
        config: DataCloudConfig,
    ) -> Self {
        Self {
            platform_token_manager,
            http_client,
            config,
        }
    }

    /// Returns the grant type used for the Data Cloud token exchange.
    fn grant_type() -> &'static str {
        "urn:salesforce:grant-type:external:cdp"
    }

    /// Returns the subject token type for the exchange.
    fn subject_token_type() -> &'static str {
        "urn:ietf:params:oauth:token-type:access_token"
    }

    /// Resolves the token exchange URL, using the configured override or
    /// deriving it from the platform token's instance URL.
    fn resolve_exchange_url(&self, platform_instance_url: &str) -> String {
        self.config
            .token_exchange_url
            .clone()
            .unwrap_or_else(|| format!("{platform_instance_url}/services/a360/token"))
    }
}

impl<A: Authenticator> fmt::Debug for DataCloudAuthenticator<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DataCloudAuthenticator")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl<A: Authenticator> Authenticator for DataCloudAuthenticator<A> {
    async fn authenticate(&self) -> Result<AccessToken> {
        // Step 1: Get a fresh platform token
        let platform_token = self.platform_token_manager.token().await?;

        // Step 2: Exchange it for a Data Cloud token
        let exchange_url = self.resolve_exchange_url(platform_token.instance_url());

        let params = [
            ("grant_type", Self::grant_type()),
            ("subject_token", platform_token.as_str()),
            ("subject_token_type", Self::subject_token_type()),
        ];

        let response = self
            .http_client
            .post(&exchange_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| ForceError::Http(HttpError::RequestFailed(e)))?;

        if !response.status().is_success() {
            return Err(crate::auth::handle_oauth_error(
                response,
                Some("Data Cloud token exchange failed"),
            )
            .await);
        }

        // Parse successful DC token response
        // Limit JSON payloads to 10MB to prevent memory exhaustion (DoS)
        let limit = 10 * 1024 * 1024;
        let bytes = crate::http::error::read_capped_body_bytes(response, limit).await;

        let dc_response = serde_json::from_slice::<DataCloudTokenResponse>(&bytes)
            .map_err(crate::error::SerializationError::from)?;

        Ok(AccessToken::from_response(
            dc_response.into_token_response(),
        ))
    }

    async fn refresh(&self) -> Result<AccessToken> {
        // Data Cloud token exchange does not support refresh tokens.
        // Re-authenticate (which re-exchanges with a fresh platform token).
        self.authenticate().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Must;

    // ── DataCloudConfig tests ────────────────────────────────────────────

    #[test]
    fn test_data_cloud_config_default() {
        let config = DataCloudConfig::default();
        assert!(config.token_exchange_url.is_none());
        assert!(config.api_version.is_none());
    }

    #[test]
    fn test_data_cloud_config_with_exchange_url() {
        let config = DataCloudConfig {
            token_exchange_url: Some("https://custom.sf.com/services/a360/token".into()),
            ..Default::default()
        };
        assert_eq!(
            config.token_exchange_url.as_deref(),
            Some("https://custom.sf.com/services/a360/token")
        );
    }

    #[test]
    fn test_data_cloud_config_with_api_version() {
        let config = DataCloudConfig {
            api_version: Some("v64.0".into()),
            ..Default::default()
        };
        assert_eq!(config.api_version.as_deref(), Some("v64.0"));
    }

    #[test]
    fn test_data_cloud_config_clone() {
        let config = DataCloudConfig {
            token_exchange_url: Some("https://test.com/a360".into()),
            api_version: Some("v63.0".into()),
        };
        let cloned = config.clone();
        assert_eq!(config.token_exchange_url, cloned.token_exchange_url);
        assert_eq!(config.api_version, cloned.api_version);
    }

    #[test]
    fn test_data_cloud_config_debug() {
        let config = DataCloudConfig::default();
        let debug = format!("{config:?}");
        assert!(debug.contains("DataCloudConfig"));
    }

    // ── DataCloudTokenResponse tests ─────────────────────────────────────

    #[test]
    fn test_dc_token_response_deserialization() {
        let json = r#"{
            "access_token": "dc_token_123",
            "instance_url": "https://tenant.c360a.salesforce.com",
            "token_type": "Bearer",
            "expires_in": 7200
        }"#;

        let response: DataCloudTokenResponse = serde_json::from_str(json).must();
        assert_eq!(response.access_token, "dc_token_123");
        assert_eq!(response.instance_url, "https://tenant.c360a.salesforce.com");
        assert_eq!(response.token_type, "Bearer");
        assert_eq!(response.expires_in, Some(7200));
    }

    #[test]
    fn test_dc_token_response_minimal() {
        let json = r#"{
            "access_token": "dc_min",
            "instance_url": "https://tenant.c360a.salesforce.com"
        }"#;

        let response: DataCloudTokenResponse = serde_json::from_str(json).must();
        assert_eq!(response.access_token, "dc_min");
        assert_eq!(response.token_type, "Bearer"); // default
        assert!(response.expires_in.is_none());
    }

    #[test]
    fn test_dc_token_response_into_token_response() {
        let dc_response = DataCloudTokenResponse {
            access_token: "dc_token".into(),
            instance_url: "https://tenant.c360a.salesforce.com".into(),
            token_type: "Bearer".into(),
            expires_in: Some(3600),
        };

        let token_response = dc_response.into_token_response();
        assert_eq!(token_response.access_token, "dc_token");
        assert_eq!(
            token_response.instance_url,
            "https://tenant.c360a.salesforce.com"
        );
        assert_eq!(token_response.token_type, "Bearer");
        assert_eq!(token_response.expires_in, Some(3600));
        assert!(token_response.signature.is_empty());
        assert!(token_response.refresh_token.is_none());
        // issued_at should be a valid timestamp (numeric string)
        assert!(token_response.issued_at.parse::<i64>().is_ok());
    }

    #[test]
    fn test_dc_token_response_to_access_token() {
        let dc_response = DataCloudTokenResponse {
            access_token: "dc_access".into(),
            instance_url: "https://dc-tenant.salesforce.com".into(),
            token_type: "Bearer".into(),
            expires_in: Some(7200),
        };

        let access_token = AccessToken::from_response(dc_response.into_token_response());
        assert_eq!(access_token.as_str(), "dc_access");
        assert_eq!(
            access_token.instance_url(),
            "https://dc-tenant.salesforce.com"
        );
        assert!(access_token.expires_at().is_some());
        assert!(!access_token.is_expired());
    }

    // ── DataCloudAuthenticator unit tests ────────────────────────────────

    #[test]
    fn test_grant_type() {
        assert_eq!(
            DataCloudAuthenticator::<crate::test_support::MockAuthenticator>::grant_type(),
            "urn:salesforce:grant-type:external:cdp"
        );
    }

    #[test]
    fn test_subject_token_type() {
        assert_eq!(
            DataCloudAuthenticator::<crate::test_support::MockAuthenticator>::subject_token_type(),
            "urn:ietf:params:oauth:token-type:access_token"
        );
    }

    #[test]
    fn test_resolve_exchange_url_default() {
        let tm = Arc::new(TokenManager::new(
            crate::test_support::MockAuthenticator::new("t", "https://na1.salesforce.com"),
        ));
        let auth =
            DataCloudAuthenticator::new(tm, reqwest::Client::new(), DataCloudConfig::default());
        let url = auth.resolve_exchange_url("https://na1.salesforce.com");
        assert_eq!(url, "https://na1.salesforce.com/services/a360/token");
    }

    #[test]
    fn test_resolve_exchange_url_override() {
        let tm = Arc::new(TokenManager::new(
            crate::test_support::MockAuthenticator::new("t", "https://na1.salesforce.com"),
        ));
        let config = DataCloudConfig {
            token_exchange_url: Some("https://custom.sf.com/a360/token".into()),
            ..Default::default()
        };
        let auth = DataCloudAuthenticator::new(tm, reqwest::Client::new(), config);
        let url = auth.resolve_exchange_url("https://na1.salesforce.com");
        assert_eq!(url, "https://custom.sf.com/a360/token");
    }

    #[test]
    fn test_authenticator_debug_does_not_leak() {
        let tm = Arc::new(TokenManager::new(
            crate::test_support::MockAuthenticator::new("secret_token", "https://na1.sf.com"),
        ));
        let auth =
            DataCloudAuthenticator::new(tm, reqwest::Client::new(), DataCloudConfig::default());
        let debug = format!("{auth:?}");
        assert!(debug.contains("DataCloudAuthenticator"));
        assert!(!debug.contains("secret_token"));
    }

    // ── Wiremock integration tests ───────────────────────────────────────

    #[cfg(feature = "mock")]
    mod integration {
        use super::*;
        use crate::error::AuthenticationError;
        use crate::test_support::{MockAuthenticator, Must};
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        /// Helper: create a `DataCloudAuthenticator` pointing at the given mock server.
        fn dc_auth_for(mock_server: &MockServer) -> DataCloudAuthenticator<MockAuthenticator> {
            let platform_auth = MockAuthenticator::new("platform_token", &mock_server.uri());
            let tm = Arc::new(TokenManager::new(platform_auth));
            DataCloudAuthenticator::new(
                tm,
                reqwest::Client::new(),
                DataCloudConfig {
                    token_exchange_url: Some(format!("{}/services/a360/token", mock_server.uri())),
                    ..Default::default()
                },
            )
        }

        #[tokio::test]
        async fn test_authenticate_success() {
            let mock_server = MockServer::start().await;

            let dc_token_response = serde_json::json!({
                "access_token": "dc_token_abc",
                "instance_url": "https://tenant.c360a.salesforce.com",
                "token_type": "Bearer",
                "expires_in": 7200
            });

            Mock::given(method("POST"))
                .and(path("/services/a360/token"))
                .respond_with(ResponseTemplate::new(200).set_body_json(dc_token_response))
                .expect(1)
                .mount(&mock_server)
                .await;

            let auth = dc_auth_for(&mock_server);
            let token = auth.authenticate().await.must();

            assert_eq!(token.as_str(), "dc_token_abc");
            assert_eq!(token.instance_url(), "https://tenant.c360a.salesforce.com");
            assert_eq!(token.token_type(), "Bearer");
            assert!(token.expires_at().is_some());
        }

        #[tokio::test]
        async fn test_authenticate_oauth_error() {
            let mock_server = MockServer::start().await;

            let error_response = serde_json::json!({
                "error": "invalid_grant",
                "error_description": "token exchange not authorized"
            });

            Mock::given(method("POST"))
                .and(path("/services/a360/token"))
                .respond_with(ResponseTemplate::new(400).set_body_json(error_response))
                .mount(&mock_server)
                .await;

            let auth = dc_auth_for(&mock_server);
            let result = auth.authenticate().await;

            if let Err(ForceError::Authentication(AuthenticationError::TokenRequestFailed(msg))) =
                result
            {
                assert!(msg.contains("Data Cloud token exchange failed"));
                assert!(msg.contains("invalid_grant"));
                assert!(msg.contains("token exchange not authorized"));
            } else {
                panic!("Expected TokenRequestFailed, got: {result:?}");
            }
        }

        #[tokio::test]
        async fn test_authenticate_server_error() {
            let mock_server = MockServer::start().await;

            Mock::given(method("POST"))
                .and(path("/services/a360/token"))
                .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
                .mount(&mock_server)
                .await;

            let auth = dc_auth_for(&mock_server);
            let result = auth.authenticate().await;

            if let Err(ForceError::Http(HttpError::StatusError {
                status_code,
                message,
            })) = result
            {
                assert_eq!(status_code, 500);
                assert!(message.contains("Data Cloud token exchange failed"));
                assert!(message.contains("Internal Server Error"));
            } else {
                panic!("Expected StatusError, got: {result:?}");
            }
        }

        #[tokio::test]
        async fn test_authenticate_empty_error_body() {
            let mock_server = MockServer::start().await;

            Mock::given(method("POST"))
                .and(path("/services/a360/token"))
                .respond_with(ResponseTemplate::new(401).set_body_string(""))
                .mount(&mock_server)
                .await;

            let auth = dc_auth_for(&mock_server);
            let result = auth.authenticate().await;

            if let Err(ForceError::Http(HttpError::StatusError {
                status_code,
                message,
            })) = result
            {
                assert_eq!(status_code, 401);
                assert!(message.contains("Unknown error"));
            } else {
                panic!("Expected StatusError with empty body, got: {result:?}");
            }
        }

        #[tokio::test]
        async fn test_refresh_calls_authenticate() {
            let mock_server = MockServer::start().await;

            let dc_token_response = serde_json::json!({
                "access_token": "refreshed_dc_token",
                "instance_url": "https://tenant.c360a.salesforce.com",
                "token_type": "Bearer",
                "expires_in": 3600
            });

            Mock::given(method("POST"))
                .and(path("/services/a360/token"))
                .respond_with(ResponseTemplate::new(200).set_body_json(dc_token_response))
                .expect(2) // authenticate + refresh
                .mount(&mock_server)
                .await;

            let auth = dc_auth_for(&mock_server);

            let token1 = auth.authenticate().await.must();
            assert_eq!(token1.as_str(), "refreshed_dc_token");

            let token2 = auth.refresh().await.must();
            assert_eq!(token2.as_str(), "refreshed_dc_token");
        }

        #[tokio::test]
        async fn test_authenticate_network_error() {
            let platform_auth =
                MockAuthenticator::new("platform_token", "https://test.salesforce.com");
            let tm = Arc::new(TokenManager::new(platform_auth));
            let auth = DataCloudAuthenticator::new(
                tm,
                reqwest::Client::new(),
                DataCloudConfig {
                    token_exchange_url: Some(
                        "http://invalid.invalid.localhost:99999/a360/token".into(),
                    ),
                    ..Default::default()
                },
            );

            let result = auth.authenticate().await;
            assert!(matches!(result, Err(ForceError::Http(_))));
        }

        #[tokio::test]
        async fn test_dc_token_has_dc_instance_url() {
            let mock_server = MockServer::start().await;

            let dc_token_response = serde_json::json!({
                "access_token": "dc_token",
                "instance_url": "https://my-tenant.c360a.salesforce.com",
                "token_type": "Bearer",
                "expires_in": 7200
            });

            Mock::given(method("POST"))
                .and(path("/services/a360/token"))
                .respond_with(ResponseTemplate::new(200).set_body_json(dc_token_response))
                .mount(&mock_server)
                .await;

            let auth = dc_auth_for(&mock_server);
            let token = auth.authenticate().await.must();

            // DC token instance_url should be the tenant URL, NOT the platform URL
            assert_eq!(
                token.instance_url(),
                "https://my-tenant.c360a.salesforce.com"
            );
            assert!(
                !token.instance_url().contains("127.0.0.1"),
                "DC token should not contain mock server URL"
            );
        }
    }
}
