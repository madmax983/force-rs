//! 👺 Havoc: `AccessToken` Expiration Overflow Test

use force::auth::{AccessToken, TokenResponse};

#[test]
fn havoc_access_token_overflow() {
    let _ = AccessToken::from_response(TokenResponse {
        access_token: secrecy::SecretString::new("test".to_string().into()),
        instance_url: "http://example.com".to_string(),
        token_type: "Bearer".to_string(),
        issued_at: "1704070800000".to_string(),
        signature: "sig".to_string(),
        expires_in: Some(u64::MAX), // Try to trigger the panic
        refresh_token: None,
    });
}
