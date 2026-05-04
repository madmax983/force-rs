#![allow(missing_docs)]
use chrono::{DateTime, Utc};
use force::auth::{AccessToken, TokenResponse};

#[test]
fn havoc_access_token_overflow() {
    // Find the maximum valid timestamp for chrono
    let max_time = DateTime::<Utc>::MAX_UTC;
    let max_timestamp_ms = max_time.timestamp_millis();

    // Create a TokenResponse with max valid issued_at
    let response = TokenResponse {
        access_token: secrecy::SecretString::new("token".to_string().into()),
        instance_url: "url".to_string(),
        token_type: "Bearer".to_string(),
        issued_at: max_timestamp_ms.to_string(),
        signature: "sig".to_string(),
        expires_in: Some(1), // Just 1 second is enough to overflow
        refresh_token: None,
    };

    // This should NOT panic, but return a token with None expiration
    let token = AccessToken::from_response(response);

    // Assert expiration is None (overflow handled)
    assert!(
        token.expires_at().is_none(),
        "Expiration should be None on overflow"
    );

    // Assert token is not expired (since no expiration means valid forever)
    assert!(
        !token.is_expired(),
        "Token with None expiration should not be expired"
    );
}
