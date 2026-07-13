//! Access token and token-response types for Marketing Cloud Engagement.
//!
//! The Marketing Cloud server-to-server (client credentials) flow returns a
//! short-lived bearer token (~20 minutes) with **no refresh token**. Alongside
//! the token, the response carries the authoritative `rest_instance_url` and
//! `soap_instance_url` base URLs that every subsequent API call must use.

use crate::error::{MarketingCloudError, Result};
use chrono::{DateTime, Duration, Utc};
use reqwest::header::HeaderValue;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

/// Safety buffer subtracted from the hard expiry to compute the soft expiry.
///
/// Tokens are proactively re-authenticated once they enter this window so that
/// in-flight requests never race a server-side expiry.
const SOFT_EXPIRY_BUFFER_SECONDS: i64 = 60;

/// Raw JSON response from `POST {auth}/v2/token`.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenResponse {
    /// The bearer access token.
    pub access_token: SecretString,

    /// Token type, always `"Bearer"` for this flow.
    #[serde(default = "default_token_type")]
    pub token_type: String,

    /// Token lifetime in seconds (Marketing Cloud reports `1080`).
    pub expires_in: u64,

    /// Base URL for subsequent REST calls (ends with a trailing slash).
    pub rest_instance_url: String,

    /// Base URL for SOAP calls (returned but not used by the REST client).
    #[serde(default)]
    pub soap_instance_url: String,

    /// Space-separated granted scopes.
    #[serde(default)]
    pub scope: String,
}

/// Default token type when the field is absent.
fn default_token_type() -> String {
    "Bearer".to_string()
}

/// A parsed, secure access token with expiry tracking and cached headers.
#[derive(Debug, Clone)]
pub struct AccessToken {
    /// The raw token value (kept secret).
    token: SecretString,

    /// Pre-computed, sensitive `Authorization: Bearer ...` header.
    auth_header: HeaderValue,

    /// Base URL for REST calls (guaranteed to end with `/`).
    rest_instance_url: String,

    /// Base URL for SOAP calls.
    soap_instance_url: String,

    /// Absolute time at which the token becomes invalid server-side.
    hard_expires_at: DateTime<Utc>,

    /// Absolute time at which the token should be proactively re-fetched.
    soft_expires_at: DateTime<Utc>,
}

impl AccessToken {
    /// Builds an [`AccessToken`] from a raw token response.
    ///
    /// # Errors
    ///
    /// Returns [`MarketingCloudError::InvalidAuthHeader`] if the token contains
    /// characters that cannot form a valid HTTP header value.
    pub fn from_response(response: TokenResponse) -> Result<Self> {
        let auth_header =
            build_auth_header(&response.token_type, response.access_token.expose_secret())
                .ok_or(MarketingCloudError::InvalidAuthHeader)?;

        let now = Utc::now();
        let lifetime = Duration::try_seconds(clamp_lifetime(response.expires_in))
            .unwrap_or_else(|| Duration::seconds(0));
        let hard_expires_at = now.checked_add_signed(lifetime).unwrap_or(now);
        let soft_expires_at = hard_expires_at
            .checked_sub_signed(Duration::seconds(SOFT_EXPIRY_BUFFER_SECONDS))
            .unwrap_or(hard_expires_at);

        Ok(Self {
            token: response.access_token,
            auth_header,
            rest_instance_url: ensure_trailing_slash(response.rest_instance_url),
            soap_instance_url: response.soap_instance_url,
            hard_expires_at,
            soft_expires_at,
        })
    }

    /// Constructs a token directly with an explicit hard expiry (test-only).
    #[cfg(test)]
    pub(crate) fn new_for_test(
        token: &str,
        rest_instance_url: &str,
        hard_expires_at: DateTime<Utc>,
    ) -> Result<Self> {
        let auth_header =
            build_auth_header("Bearer", token).ok_or(MarketingCloudError::InvalidAuthHeader)?;
        let soft_expires_at = hard_expires_at
            .checked_sub_signed(Duration::seconds(SOFT_EXPIRY_BUFFER_SECONDS))
            .unwrap_or(hard_expires_at);
        Ok(Self {
            token: SecretString::new(token.to_string().into()),
            auth_header,
            rest_instance_url: ensure_trailing_slash(rest_instance_url.to_string()),
            soap_instance_url: String::new(),
            hard_expires_at,
            soft_expires_at,
        })
    }

    /// Returns the raw token value.
    ///
    /// # Security
    ///
    /// Exposes the secret token. Avoid logging the returned value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.token.expose_secret()
    }

    /// Returns the cached, sensitive `Authorization` header value.
    #[must_use]
    pub const fn auth_header(&self) -> &HeaderValue {
        &self.auth_header
    }

    /// Returns the REST base URL (always ends with `/`).
    #[must_use]
    pub fn rest_instance_url(&self) -> &str {
        &self.rest_instance_url
    }

    /// Returns the SOAP base URL.
    #[must_use]
    pub fn soap_instance_url(&self) -> &str {
        &self.soap_instance_url
    }

    /// Returns the absolute hard-expiry time.
    #[must_use]
    pub const fn hard_expires_at(&self) -> DateTime<Utc> {
        self.hard_expires_at
    }

    /// Returns `true` once the token is fully expired (past its hard expiry).
    #[must_use]
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.hard_expires_at
    }

    /// Returns `true` when the token should be proactively re-fetched.
    ///
    /// This is true within the 60-second safety buffer before hard expiry.
    #[must_use]
    pub fn needs_refresh(&self) -> bool {
        Utc::now() >= self.soft_expires_at
    }
}

/// Clamps an untrusted lifetime to a sane range to avoid `Duration` overflow.
fn clamp_lifetime(expires_in: u64) -> i64 {
    i64::try_from(expires_in.min(3_000_000_000)).unwrap_or(0)
}

/// Ensures a base URL ends with exactly one trailing slash.
fn ensure_trailing_slash(mut url: String) -> String {
    if !url.ends_with('/') {
        url.push('/');
    }
    url
}

/// Builds a sensitive `Authorization` header value, returning `None` on invalid input.
fn build_auth_header(token_type: &str, token: &str) -> Option<HeaderValue> {
    let mut header = HeaderValue::from_str(&format!("{token_type} {token}")).ok()?;
    header.set_sensitive(true);
    Some(header)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn sample_response(expires_in: u64) -> TokenResponse {
        TokenResponse {
            access_token: SecretString::new("tok-abc".to_string().into()),
            token_type: "Bearer".to_string(),
            expires_in,
            rest_instance_url: "https://sub.rest.marketingcloudapis.com".to_string(),
            soap_instance_url: "https://sub.soap.marketingcloudapis.com/".to_string(),
            scope: "email_send".to_string(),
        }
    }

    #[test]
    fn from_response_populates_fields_and_adds_trailing_slash() {
        let token = AccessToken::from_response(sample_response(1080)).unwrap();
        assert_eq!(token.as_str(), "tok-abc");
        assert_eq!(
            token.rest_instance_url(),
            "https://sub.rest.marketingcloudapis.com/"
        );
        assert_eq!(
            token.soap_instance_url(),
            "https://sub.soap.marketingcloudapis.com/"
        );
        assert!(!token.is_expired());
        assert!(!token.needs_refresh());
    }

    #[test]
    fn token_response_deserializes_from_json() {
        let json = r#"{
            "access_token":"eyJ...",
            "token_type":"Bearer",
            "expires_in":1080,
            "scope":"email_send",
            "soap_instance_url":"https://s.soap.marketingcloudapis.com/",
            "rest_instance_url":"https://s.rest.marketingcloudapis.com/"
        }"#;
        let response: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.expires_in, 1080);
        assert_eq!(response.access_token.expose_secret(), "eyJ...");
    }

    #[test]
    fn expired_token_reports_expired_and_needs_refresh() {
        let past = Utc::now() - Duration::seconds(5);
        let token =
            AccessToken::new_for_test("expired", "https://x.rest.marketingcloudapis.com/", past)
                .unwrap();
        assert!(token.is_expired());
        assert!(token.needs_refresh());
    }

    #[test]
    fn token_in_soft_window_needs_refresh_but_not_hard_expired() {
        // Hard-expires in 30s, which is inside the 60s soft buffer.
        let soon = Utc::now() + Duration::seconds(30);
        let token =
            AccessToken::new_for_test("soon", "https://x.rest.marketingcloudapis.com/", soon)
                .unwrap();
        assert!(!token.is_expired());
        assert!(token.needs_refresh());
    }

    #[test]
    fn healthy_token_neither_expired_nor_needing_refresh() {
        let later = Utc::now() + Duration::minutes(10);
        let token =
            AccessToken::new_for_test("fresh", "https://x.rest.marketingcloudapis.com/", later)
                .unwrap();
        assert!(!token.is_expired());
        assert!(!token.needs_refresh());
    }

    #[test]
    fn invalid_header_characters_rejected() {
        let mut response = sample_response(1080);
        response.access_token = SecretString::new("bad\ntoken".to_string().into());
        let result = AccessToken::from_response(response);
        assert!(matches!(
            result,
            Err(MarketingCloudError::InvalidAuthHeader)
        ));
    }

    #[test]
    fn oversized_lifetime_is_clamped() {
        let token = AccessToken::from_response(sample_response(u64::MAX)).unwrap();
        // Should not panic and should be far in the future.
        assert!(!token.is_expired());
    }
}
