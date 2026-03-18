//! Shared gRPC metadata helper for Salesforce Pub/Sub API authentication.
//!
//! The Pub/Sub API requires three metadata headers on **every** RPC call:
//! - `accesstoken` — OAuth2 bearer token
//! - `instanceurl` — Salesforce instance URL
//! - `tenantid` — Org ID (18-char)
//!
//! This module centralises header construction so that `handler`, `subscriber`,
//! and `publisher` all use a single consistent implementation.

use force::auth::AccessToken;

use crate::error::{PubSubError, Result};

/// Build the three required gRPC metadata headers for the Salesforce Pub/Sub API.
///
/// # Errors
///
/// Returns [`PubSubError::Config`] if any header value contains characters that
/// are not valid in an ASCII HTTP/2 metadata value (e.g., control characters or
/// non-ASCII bytes in the token or URL).
///
/// # Example
///
/// ```ignore
/// let meta = build_metadata(&token, "https://org.my.salesforce.com", "00Dxx000001gEREAY")?;
/// request.metadata_mut().extend(meta);
/// ```
pub fn build_metadata(
    token: &AccessToken,
    instance_url: &str,
    tenant_id: &str,
) -> Result<tonic::metadata::MetadataMap> {
    let mut map = tonic::metadata::MetadataMap::new();

    map.insert(
        "accesstoken",
        token
            .as_str()
            .parse()
            .map_err(|_| PubSubError::Config("invalid token characters".to_string()))?,
    );
    map.insert(
        "instanceurl",
        instance_url
            .parse()
            .map_err(|_| PubSubError::Config("invalid instance URL characters".to_string()))?,
    );
    map.insert(
        "tenantid",
        tenant_id
            .parse()
            .map_err(|_| PubSubError::Config("invalid tenant ID characters".to_string()))?,
    );

    Ok(map)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use force::auth::TokenResponse;

    fn make_token(value: &str, url: &str) -> AccessToken {
        AccessToken::from_response(TokenResponse {
            access_token: value.to_string(),
            instance_url: url.to_string(),
            token_type: "Bearer".to_string(),
            issued_at: "1704067200000".to_string(),
            signature: String::new(),
            expires_in: None,
            refresh_token: None,
        })
    }

    #[test]
    fn test_build_metadata_happy_path() {
        let token = make_token("test-token-value", "https://org.my.salesforce.com");
        let meta = build_metadata(
            &token,
            "https://org.my.salesforce.com",
            "00Dxx0000001gEREAY",
        )
        .expect("build_metadata should succeed");

        assert_eq!(
            meta.get("accesstoken").map(|v| v.to_str().unwrap_or("")),
            Some("test-token-value")
        );
        assert_eq!(
            meta.get("instanceurl").map(|v| v.to_str().unwrap_or("")),
            Some("https://org.my.salesforce.com")
        );
        assert_eq!(
            meta.get("tenantid").map(|v| v.to_str().unwrap_or("")),
            Some("00Dxx0000001gEREAY")
        );
    }

    #[test]
    fn test_build_metadata_all_headers_present() {
        let token = make_token("abc123", "https://example.my.salesforce.com");
        let meta = build_metadata(
            &token,
            "https://example.my.salesforce.com",
            "00Dxx0000001XXXXX",
        )
        .expect("should succeed");

        // All three headers must be present
        assert!(
            meta.get("accesstoken").is_some(),
            "accesstoken header missing"
        );
        assert!(
            meta.get("instanceurl").is_some(),
            "instanceurl header missing"
        );
        assert!(meta.get("tenantid").is_some(), "tenantid header missing");
    }

    #[test]
    fn test_build_metadata_invalid_token_chars() {
        // Tokens with newlines are invalid HTTP/2 header values
        let token = make_token("bad\ntoken", "https://org.my.salesforce.com");
        let result = build_metadata(
            &token,
            "https://org.my.salesforce.com",
            "00Dxx0000001gEREAY",
        );

        assert!(result.is_err(), "should fail with invalid token");
        assert!(matches!(result.unwrap_err(), PubSubError::Config(_)));
    }

    #[test]
    fn test_build_metadata_invalid_tenant_id_chars() {
        let token = make_token("good-token", "https://org.my.salesforce.com");
        // Tenant ID with a newline character
        let result = build_metadata(
            &token,
            "https://org.my.salesforce.com",
            "00Dxx\n0000001gEREAY",
        );

        assert!(result.is_err(), "should fail with invalid tenant id");
        assert!(matches!(result.unwrap_err(), PubSubError::Config(_)));
    }
}
