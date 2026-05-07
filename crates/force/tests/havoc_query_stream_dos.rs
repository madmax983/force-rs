//! 👺 Havoc: Unbounded SOQL and Pagination URL `DoS`

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use force::api::RestOperation;
    use force::auth::{AccessToken, Authenticator, TokenResponse};
    use force::client::builder;
    use force::error::ForceError;
    use force::error::Result;
    use serde::Deserialize;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[derive(Debug, Clone)]
    struct MockAuthenticator(String);

    #[async_trait]
    impl Authenticator for MockAuthenticator {
        async fn authenticate(&self) -> Result<AccessToken> {
            Ok(AccessToken::from_response(TokenResponse {
                access_token: secrecy::SecretString::new("token".to_string().into()),
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

    #[derive(Deserialize, Debug)]
    struct Dummy {}

    #[tokio::test]
    async fn test_query_dos_limit() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "totalSize": 0,
                "done": true,
                "records": []
            })))
            .mount(&server)
            .await;

        let auth = MockAuthenticator(server.uri());
        let client = builder()
            .authenticate(auth)
            .build()
            .await
            .unwrap_or_else(|_| panic!("Failed to build client"));

        // 1. Exact limit (should pass validation, but may fail due to URL parsing error)
        let max_query = "A".repeat(100_000);
        let result = client.rest().query::<Dummy>(&max_query).await;
        // Even if it fails (e.g. invalid URI), it should NOT fail with our InvalidInput > 100000 limit error.
        if let Err(ForceError::InvalidInput(msg)) = &result {
            assert!(
                !msg.contains("100,000 bytes"),
                "100,000 should not trigger the limit error"
            );
        }

        // 2. Off-by-one limit (should fail validation)
        let massive_query = "A".repeat(100_001);
        let result = client.rest().query::<Dummy>(&massive_query).await;

        let Err(err) = result else {
            panic!("Expected an error but got Ok");
        };
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("100,000 bytes"),
            "Expected error mentioning 100,000 bytes, got: {err_msg}"
        );
    }
}
