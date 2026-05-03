//! Access token and response types.
//!
//! This module defines the core data structures for Salesforce OAuth tokens:
//! - `TokenResponse`: The raw JSON response from Salesforce.
//! - `AccessToken`: A secure, parsed representation with expiration tracking.

use crate::error::{HttpError, Result};
use chrono::{DateTime, Duration, Utc};
use reqwest::header::HeaderValue;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

/// OAuth token response from Salesforce.
///
/// This structure represents the JSON response from Salesforce OAuth endpoints.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenResponse {
    /// The access token string.
    pub access_token: SecretString,

    /// The instance URL for API requests.
    pub instance_url: String,

    /// Token type (typically "Bearer").
    #[serde(default = "default_token_type")]
    pub token_type: String,

    /// Issued at timestamp (Unix epoch milliseconds).
    /// Not all flows return this field (e.g., JWT Bearer Flow omits it).
    #[serde(default = "default_issued_at")]
    pub issued_at: String,

    /// Token signature.
    #[serde(default)]
    pub signature: String,

    /// Expires in seconds (optional, not all flows provide this).
    #[serde(default)]
    pub expires_in: Option<u64>,

    /// Refresh token (optional, only for flows that support refresh).
    #[serde(default)]
    pub refresh_token: Option<SecretString>,
}

pub fn default_token_type() -> String {
    "Bearer".to_string()
}

/// Default `issued_at` for flows that don't return it (e.g., JWT Bearer).
/// Returns the current time as a Unix-epoch-milliseconds string.
fn default_issued_at() -> String {
    Utc::now().timestamp_millis().to_string()
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

    /// Pre-computed Authorization header value.
    ///
    /// This avoids formatting and parsing the header on every request,
    /// significantly improving performance on hot paths.
    auth_header: Option<HeaderValue>,
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
        let expires_at = calculate_expiration(issued_at, response.expires_in);
        let auth_header = create_auth_header(&response.token_type, response.access_token.expose_secret());

        Self {
            token: response.access_token,
            issued_at,
            expires_at,
            instance_url: response.instance_url,
            token_type: response.token_type,
            auth_header,
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
        let auth_header = create_auth_header("Bearer", &token);

        Self {
            token: SecretString::new(token.into()),
            issued_at: Utc::now(),
            expires_at,
            instance_url,
            token_type: "Bearer".to_string(),
            auth_header,
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

    /// Checks if the token is expired (alias for [`is_soft_expired`]).
    ///
    /// Prefer using `is_soft_expired()` or `is_hard_expired()` for clarity.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.is_soft_expired()
    }

    /// Checks if the token is "hard" expired, meaning it is absolutely invalid.
    ///
    /// This uses a 0-second buffer — the token must be past its expiration time.
    #[must_use]
    pub fn is_hard_expired(&self) -> bool {
        self.is_expired_with_buffer(Duration::zero())
    }

    /// Checks if the token is "soft" expired, meaning it should be refreshed proactively.
    ///
    /// Uses a 60-second buffer: returns `true` if the token will expire within 60 seconds.
    /// If no expiration time is known, returns `false` (assume valid).
    #[must_use]
    pub fn is_soft_expired(&self) -> bool {
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

    /// Returns the pre-computed Authorization header value.
    ///
    /// This method returns a reference to the cached header value, which
    /// avoids allocation and parsing on every call.
    ///
    /// # Errors
    ///
    /// Returns an error if the header could not be constructed (e.g. invalid characters).
    pub fn auth_header(&self) -> std::result::Result<&HeaderValue, HttpError> {
        self.auth_header
            .as_ref()
            .ok_or_else(|| HttpError::InvalidUrl("invalid authorization header".to_string()))
    }
}

/// Calculates expiration time from issued_at + expires_in seconds.
fn calculate_expiration(
    issued_at: DateTime<Utc>,
    expires_in: Option<u64>,
) -> Option<DateTime<Utc>> {
    expires_in.and_then(|seconds| {
        // Cap duration to ~100 years (3B seconds) to prevent overflow in Duration::try_seconds
        if seconds > 3_000_000_000 {
            return None;
        }
        // Safe to cast because we checked <= 3B, which fits in i64 (max ~9e18)
        let Ok(seconds_i64) = i64::try_from(seconds) else {
            return None;
        };
        let duration = Duration::try_seconds(seconds_i64)?;
        issued_at.checked_add_signed(duration)
    })
}

/// Creates a secure Authorization header value.
fn create_auth_header(token_type: &str, access_token: &str) -> Option<HeaderValue> {
    let mut header = HeaderValue::from_str(&format!("{} {}", token_type, access_token)).ok();
    if let Some(h) = &mut header {
        h.set_sensitive(true);
    }
    header
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

    DateTime::from_timestamp_millis(timestamp_ms).ok_or_else(|| {
        crate::error::ForceError::Serialization(crate::error::SerializationError::InvalidFormat(
            format!("timestamp out of range: {timestamp_ms}"),
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;

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
        assert_eq!(response.access_token.expose_secret(), "00D123456789!token");
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
            access_token: SecretString::new("test_token".to_string().into()),
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
    fn test_access_token_hard_vs_soft_expiry() {
        // Token expires in 30 seconds
        let expires_at = Utc::now() + Duration::seconds(30);
        let token = AccessToken::new(
            "token".to_string(),
            "https://test.salesforce.com".to_string(),
            Some(expires_at),
        );

        // Soft expired? Yes, because buffer is 60s (30s < 60s)
        assert!(token.is_soft_expired());

        // Hard expired? No, because it is valid for 30s more (30s > 0s)
        assert!(!token.is_hard_expired());

        // Completely expired token
        let past_expiration = Utc::now() - Duration::seconds(1);
        let expired_token = AccessToken::new(
            "expired".to_string(),
            "https://test.salesforce.com".to_string(),
            Some(past_expiration),
        );

        assert!(expired_token.is_soft_expired());
        assert!(expired_token.is_hard_expired());
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
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(err.to_string().contains(""));
    }

    #[test]
    fn test_parse_issued_at_negative() {
        // Test pre-1970 timestamp
        let timestamp = "-1000"; // 1969-12-31 23:59:59 UTC
        let result = parse_issued_at(timestamp);
        assert!(result.is_ok());
        let dt = result.must();
        // Since parse_issued_at discards milliseconds and uses 0 for nanos,
        // -1000ms / 1000 = -1s.
        // DateTime::from_timestamp(-1, 0) is 1969-12-31 23:59:59.
        assert_eq!(dt.timestamp(), -1);
    }

    #[test]
    fn test_parse_issued_at_empty() {
        let timestamp = "";
        let result = parse_issued_at(timestamp);
        assert!(matches!(
            result,
            Err(crate::error::ForceError::Serialization(_))
        ));
    }

    #[test]
    fn test_access_token_expires_in_overflow() {
        let response = TokenResponse {
            access_token: SecretString::new("test_token".to_string().into()),
            instance_url: "https://example.salesforce.com".to_string(),
            token_type: "Bearer".to_string(),
            issued_at: "1704067200000".to_string(),
            signature: String::new(),
            expires_in: Some(u64::MAX), // Should be capped
            refresh_token: None,
        };

        let token = AccessToken::from_response(response);
        // Should be None (infinite validity) because u64::MAX > 3B cap
        assert!(token.expires_at.is_none());
    }

    #[test]
    fn test_access_token_expires_in_cap() {
        // Test value that fits in i64 but exceeds cap
        let large_seconds = 4_000_000_000_u64; // 4 billion > 3 billion
        let response = TokenResponse {
            access_token: SecretString::new("test_token".to_string().into()),
            instance_url: "https://example.salesforce.com".to_string(),
            token_type: "Bearer".to_string(),
            issued_at: "1704067200000".to_string(),
            signature: String::new(),
            expires_in: Some(large_seconds),
            refresh_token: None,
        };

        let token = AccessToken::from_response(response);
        // Should be None due to cap
        assert!(token.expires_at.is_none());
    }

    #[test]
    fn test_access_token_expires_in_cap_boundary() {
        // Test value that is exactly the cap (3 billion)
        let boundary_seconds = 3_000_000_000_u64;
        let response = TokenResponse {
            access_token: SecretString::new("test_token".to_string().into()),
            instance_url: "https://example.salesforce.com".to_string(),
            token_type: "Bearer".to_string(),
            issued_at: "1704067200000".to_string(),
            signature: String::new(),
            expires_in: Some(boundary_seconds),
            refresh_token: None,
        };

        let token = AccessToken::from_response(response);
        // Should be Some because 3B <= 3B
        assert!(token.expires_at.is_some());
    }

    #[test]
    fn test_parse_issued_at_milliseconds_precision() {
        // Timestamp with 500ms: 1704067200500
        let timestamp = "1704067200500";
        let result = parse_issued_at(timestamp).must();

        // This fails if precision is lost (it becomes 0)
        assert_eq!(result.timestamp_subsec_millis(), 500);
    }

    #[test]
    fn test_access_token_from_response_invalid_issued_at() {
        let response = TokenResponse {
            access_token: SecretString::new("test_token".to_string().into()),
            instance_url: "https://example.salesforce.com".to_string(),
            token_type: "Bearer".to_string(),
            issued_at: "garbage".to_string(),
            signature: String::new(),
            expires_in: Some(3600),
            refresh_token: None,
        };

        let token = AccessToken::from_response(response);
        // Verify fallback to approximately now
        let now = Utc::now();
        let diff = (now - token.issued_at).num_seconds().abs();
        assert!(
            diff < 5,
            "Should fallback to current time when issued_at is invalid"
        );
    }

    #[test]
    fn test_access_token_invalid_header_chars() {
        let response = TokenResponse {
            access_token: SecretString::new("token\nwith\nnewlines".to_string().into()),
            instance_url: "https://example.salesforce.com".to_string(),
            token_type: "Bearer".to_string(),
            issued_at: "1704067200000".to_string(),
            signature: String::new(),
            expires_in: Some(3600),
            refresh_token: None,
        };

        let token = AccessToken::from_response(response);
        // Verify auth_header() returns error instead of panic or invalid header
        assert!(token.auth_header().is_err());
    }
}
