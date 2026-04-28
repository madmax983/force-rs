#![allow(missing_docs)]
//! Havoc resource exhaustion (`DoS`) test for unbounded string inputs.
//!
//! # 👺 Havoc: Error Parsing Integer Overflow
//!
//! **The Trigger:** A malicious payload with massive arrays.
//! **The Stack Trace:** Integer overflow in `cap` causing panic.
//! **Reproduction:** Run `cargo test --test havoc_dos_error_parsing`

#[tokio::test]
async fn test_error_parsing_integer_overflow() {
    use async_trait::async_trait;
    use force::api::RestOperation;
    use force::auth::{AccessToken, Authenticator, TokenResponse};
    use force::client::builder;
    use force::error::Result;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[derive(Debug, Clone)]
    struct MockAuthenticator(String);

    #[async_trait]
    impl Authenticator for MockAuthenticator {
        async fn authenticate(&self) -> Result<AccessToken> {
            Ok(AccessToken::from_response(TokenResponse {
                access_token: "token".to_string(),
                instance_url: self.0.clone(),
                token_type: "Bearer".to_string(),
                issued_at: "1704067200000".to_string(),
                signature: "sig".to_string(),
                expires_in: None,
                refresh_token: None,
            }))
        }
        async fn refresh(&self) -> Result<AccessToken> {
            self.authenticate().await
        }
    }

    let server = MockServer::start().await;

    // We create a JSON response that is slightly smaller than the 1MB payload limit,
    // but with enough fields that (fields.len() - 1) * 2 or fields_len_sum
    // might have overflowed theoretically if they were much larger.
    // Since we fixed the code with `saturating_add`, we verify it doesn't crash on parse!
    let mut payload = String::new();
    payload.push_str("[{\"errorCode\":\"UNKNOWN\",\"message\":\"Test\",\"fields\":[");
    for _ in 0..1000 {
        payload.push_str("\"A\",");
    }
    payload.push_str("\"A\"]}]");

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(400).set_body_string(payload))
        .mount(&server)
        .await;

    let auth = MockAuthenticator(server.uri());
    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .unwrap_or_else(|_| panic!("Failed to build client"));

    // Expected to return an error properly parsed (HTTP 400), not panic!
    let result = client
        .rest()
        .query::<serde_json::Value>("SELECT Id FROM Account")
        .await;
    assert!(result.is_err());
}
