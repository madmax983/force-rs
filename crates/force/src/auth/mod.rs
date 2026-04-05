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
    let body = crate::http::error::read_capped_body(response, 1024 * 1024)
        .await
        .unwrap_or_default();

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
