//! Authentication for Salesforce APIs.
//!
//! This module provides traits and implementations for various OAuth 2.0 flows
//! supported by Salesforce, including:
//!
//! - Client Credentials (machine-to-machine)
//! - JWT Bearer (server-to-server with certificates)
//! - Username-Password (legacy, not recommended)
//! - Refresh Token (session extension)
//!
//! # Features
//!
//! - `jwt`: Enables JWT bearer token flow (requires `jsonwebtoken` dependency)

pub(crate) mod authenticator;
pub(crate) mod client_credentials;
#[cfg(feature = "data_cloud")]
pub(crate) mod data_cloud;
#[cfg(feature = "jwt")]
pub(crate) mod jwt_bearer;
pub(crate) mod token;
pub(crate) mod token_manager;
#[cfg(feature = "username_password")]
pub(crate) mod username_password;

pub use authenticator::Authenticator;
pub use client_credentials::ClientCredentials;
#[cfg(feature = "data_cloud")]
pub use data_cloud::{DataCloudAuthenticator, DataCloudConfig};
#[cfg(feature = "jwt")]
pub use jwt_bearer::JwtBearerFlow;
pub use token::{AccessToken, TokenResponse};
pub use token_manager::TokenManager;
#[cfg(feature = "username_password")]
pub use username_password::UsernamePassword;

// ─── Shared Auth Infrastructure ──────────────────────────────────────────────

use crate::error::{AuthenticationError, ForceError, HttpError};
use serde::Deserialize;

/// Well-known Salesforce OAuth token endpoint URLs.
pub(crate) const PRODUCTION_TOKEN_URL: &str = "https://login.salesforce.com/services/oauth2/token";
pub(crate) const SANDBOX_TOKEN_URL: &str = "https://test.salesforce.com/services/oauth2/token";
/// Well-known Salesforce login base URLs (used as JWT audience).
pub(crate) const PRODUCTION_LOGIN_URL: &str = "https://login.salesforce.com";
pub(crate) const SANDBOX_LOGIN_URL: &str = "https://test.salesforce.com";

/// OAuth error response from Salesforce token endpoints.
///
/// All Salesforce OAuth flows return this structure on error.
#[derive(Debug, Deserialize)]
pub(crate) struct OAuthErrorResponse {
    pub error: String,
    pub error_description: String,
}

/// Creates a default HTTP client for authentication requests.
///
/// Shared across all authenticator implementations to ensure consistent
/// timeout and TLS configuration.
pub(crate) fn default_auth_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|e| panic!("Failed to create secure HTTP client: {e}"))
}

/// Handles an OAuth error response from a Salesforce token endpoint.
///
/// Reads the capped body, attempts to parse it as an `OAuthErrorResponse`,
/// and returns the appropriate `ForceError`.
///
/// # Arguments
///
/// * `response` — The failed HTTP response
/// * `context` — Optional prefix for error messages (e.g., "Data Cloud token exchange failed")
pub(crate) async fn handle_oauth_error(
    response: reqwest::Response,
    context: Option<&str>,
) -> ForceError {
    let status = response.status();
    let body = match crate::http::error::read_capped_body(response, 1024 * 1024).await {
        Ok(body) => body,
        Err(e) => return ForceError::Http(e),
    };

    let error_text = if body.trim().is_empty() {
        "Unknown error".to_string()
    } else {
        body
    };

    if let Ok(oauth_error) = serde_json::from_str::<OAuthErrorResponse>(&error_text) {
        let msg = match context {
            Some(ctx) => format!(
                "{ctx}: {}: {}",
                oauth_error.error, oauth_error.error_description
            ),
            None => format!("{}: {}", oauth_error.error, oauth_error.error_description),
        };
        return ForceError::Authentication(AuthenticationError::TokenRequestFailed(msg));
    }

    let message = match context {
        Some(ctx) => format!("{ctx}: {error_text}"),
        None => error_text,
    };
    ForceError::Http(HttpError::StatusError {
        status_code: status.as_u16(),
        message,
    })
}

#[cfg(test)]
#[cfg(feature = "mock")]
mod tests {
    use crate::test_support::Must;
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_handle_oauth_error_json_without_context() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/error"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "error": "invalid_client",
                "error_description": "client identifier invalid"
            })))
            .mount(&mock_server)
            .await;

        let client = reqwest::Client::new();
        let res = client
            .get(format!("{}/error", mock_server.uri()))
            .send()
            .await
            .must();

        let err = handle_oauth_error(res, None).await;
        assert_eq!(
            err.to_string(),
            "authentication failed: OAuth token request failed: invalid_client: client identifier invalid"
        );
    }

    #[tokio::test]
    async fn test_handle_oauth_error_json_with_context() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/error"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "error": "invalid_client",
                "error_description": "client identifier invalid"
            })))
            .mount(&mock_server)
            .await;

        let client = reqwest::Client::new();
        let res = client
            .get(format!("{}/error", mock_server.uri()))
            .send()
            .await
            .must();

        let err = handle_oauth_error(res, Some("My context")).await;
        assert_eq!(
            err.to_string(),
            "authentication failed: OAuth token request failed: My context: invalid_client: client identifier invalid"
        );
    }

    #[tokio::test]
    async fn test_handle_oauth_error_non_json_without_context() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/error"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock_server)
            .await;

        let client = reqwest::Client::new();
        let res = client
            .get(format!("{}/error", mock_server.uri()))
            .send()
            .await
            .must();

        let err = handle_oauth_error(res, None).await;
        assert_eq!(
            err.to_string(),
            "HTTP request failed: HTTP 500: Internal Server Error"
        );
    }

    #[tokio::test]
    async fn test_handle_oauth_error_non_json_with_context() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/error"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock_server)
            .await;

        let client = reqwest::Client::new();
        let res = client
            .get(format!("{}/error", mock_server.uri()))
            .send()
            .await
            .must();

        let err = handle_oauth_error(res, Some("My context")).await;
        assert_eq!(
            err.to_string(),
            "HTTP request failed: HTTP 500: My context: Internal Server Error"
        );
    }

    #[tokio::test]
    async fn test_handle_oauth_error_empty_body() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/error"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&mock_server)
            .await;

        let client = reqwest::Client::new();
        let res = client
            .get(format!("{}/error", mock_server.uri()))
            .send()
            .await
            .must();

        let err = handle_oauth_error(res, None).await;
        assert_eq!(
            err.to_string(),
            "HTTP request failed: HTTP 401: Unknown error"
        );
    }

    #[tokio::test]
    async fn test_handle_oauth_error_truncates_large_body() {
        let mock_server = wiremock::MockServer::start().await;
        let large_body = "A".repeat(1024 * 1024 + 100);
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/error"))
            .respond_with(wiremock::ResponseTemplate::new(400).set_body_string(large_body))
            .mount(&mock_server)
            .await;

        let client = reqwest::Client::new();
        let res = client
            .get(format!("{}/error", mock_server.uri()))
            .send()
            .await
            .must();

        let err = handle_oauth_error(res, None).await;
        assert_eq!(
            err.to_string(),
            "HTTP request failed: response payload exceeded the safety limit of 1048576 bytes"
        );
    }

    #[tokio::test]
    async fn test_handle_oauth_error_does_not_truncate_medium_body() {
        let mock_server = wiremock::MockServer::start().await;
        // Using a length of 5000 chars, which is less than 1MB but more than 2048 (mutant 1024 + 1024)
        let medium_body = "A".repeat(5000);
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/error"))
            .respond_with(wiremock::ResponseTemplate::new(400).set_body_string(medium_body.clone()))
            .mount(&mock_server)
            .await;

        let client = reqwest::Client::new();
        let res = client
            .get(format!("{}/error", mock_server.uri()))
            .send()
            .await
            .must();

        let err = handle_oauth_error(res, None).await;
        assert_eq!(
            err.to_string(),
            format!("HTTP request failed: HTTP 400: {}", medium_body)
        );
    }

    #[tokio::test]
    async fn test_default_auth_http_client_timeout() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/timeout"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_delay(std::time::Duration::from_secs(31)),
            )
            .mount(&mock_server)
            .await;

        let client = default_auth_http_client();
        let result = client
            .get(format!("{}/timeout", mock_server.uri()))
            .send()
            .await;
        assert!(result.is_err());
    }
}
