#![allow(clippy::expect_used)]
//! Audit tests for token expiration logic.

use chrono::{Duration, Utc};
use force::auth::{AccessToken, TokenResponse};

#[test]
fn test_expires_in_inconsistency_repro() {
    // Case A: u64::MAX (via overflow logic) -> 1 hour
    let response_overflow = TokenResponse {
        access_token: "overflow_token".to_string(),
        instance_url: "https://test.salesforce.com".to_string(),
        token_type: "Bearer".to_string(),
        issued_at: Utc::now().timestamp_millis().to_string(),
        signature: "sig".to_string(),
        expires_in: Some(u64::MAX), // Should be capped
        refresh_token: None,
    };
    let token_overflow = AccessToken::from_response(response_overflow);

    // Assert FIXED behavior: overflow returns None (infinite), consistent with large values
    assert!(
        token_overflow.expires_at().is_none(),
        "u64::MAX should result in infinite validity"
    );

    // Case B: 4 Billion (via cap logic) -> Infinite (None)
    let response_large = TokenResponse {
        access_token: "large_token".to_string(),
        instance_url: "https://test.salesforce.com".to_string(),
        token_type: "Bearer".to_string(),
        issued_at: Utc::now().timestamp_millis().to_string(),
        signature: "sig".to_string(),
        expires_in: Some(4_000_000_000), // > 3B cap
        refresh_token: None,
    };
    let token_large = AccessToken::from_response(response_large);

    // Assert consistent behavior: explicit large value returns None (infinite)
    assert!(
        token_large.expires_at().is_none(),
        "4B should result in infinite validity"
    );
}

#[test]
fn test_invalid_issued_at_extends_validity_repro() {
    // Simulate a token issued 1 hour ago
    let past = Utc::now() - Duration::hours(1);
    let _ = past.timestamp_millis().to_string(); // Keep for context, but unused

    // But provide garbage "issued_at" string
    let response_garbage = TokenResponse {
        access_token: "garbage_token".to_string(),
        instance_url: "https://test.salesforce.com".to_string(),
        token_type: "Bearer".to_string(),
        issued_at: "not_a_timestamp".to_string(), // INVALID
        signature: "sig".to_string(),
        expires_in: Some(3600), // 1 hour validity
        refresh_token: None,
    };

    let token = AccessToken::from_response(response_garbage);

    // Assert current behavior: issued_at defaults to NOW (not the past)
    // This effectively extends the token validity by 1 hour (since it was really issued 1 hour ago)
    let issued_at = token.issued_at();
    let now = Utc::now();

    // Check it's within 1 second of now
    let diff = (now - issued_at).num_seconds().abs();
    assert!(diff < 5, "Invalid issued_at defaults to Utc::now()");
}
