#![allow(clippy::unwrap_used)]
#![allow(clippy::collapsible_match)]
#![allow(clippy::collapsible_if)]
//! Havoc resource exhaustion (`DoS`) test for unbounded string inputs.
//!
//! # 👺 Havoc: Unbounded SOQL and Pagination URL `DoS`
//!
//! **The Trigger:** Passing a massive string to `query` or `query_more`.
//! **The Stack Trace:** Massive memory allocation via `reqwest` URL construction leading to OOM.
//! **Reproduction:** Run `cargo test --test havoc_dos_query_limit`

#[cfg(test)]
mod tests {
    use force::api::rest_operation::RestOperation;
    use force::auth::{AccessToken, Authenticator, TokenResponse};
    use force::client::builder;
    use force::error::Result;
    use force::error::ForceError;
    use serde::Deserialize;
    use async_trait::async_trait;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method};

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
        let client = builder().authenticate(auth).build().await.unwrap();

        // 1. Exact limit (should pass validation, but may fail due to URL parsing error)
        let max_query = "A".repeat(100_000);
        let result = client.rest().query::<Dummy>(&max_query).await;
        // Even if it fails (e.g. invalid URI), it should NOT fail with our InvalidInput > 100000 limit error.
        if let Err(e) = &result {
            if let ForceError::InvalidInput(msg) = e {
                assert!(!msg.contains("100,000 bytes"), "100,000 should not trigger the limit error");
            }
        }

        // 2. Off-by-one limit (should fail validation)
        let massive_query = "A".repeat(100_001);
        let result = client.rest().query::<Dummy>(&massive_query).await;

        assert!(
            result.is_err(),
            "👺 Havoc: query() allowed an input string > 100,000, risking DoS!"
        );
        let Err(e) = result else { panic!("expected error") };
        let err_msg = e.to_string();
        assert!(
            err_msg.contains("100,000 bytes"),
            "Expected error mentioning 100,000 bytes, got: {err_msg}"
        );

        // 3. Mutational boundary limit
        let mutational_query = "A".repeat(100_000 + 1024);
        let result = client.rest().query::<Dummy>(&mutational_query).await;
        assert!(result.is_err(), "Mutational boundary length should fail");
    }

    #[tokio::test]
    async fn test_query_more_dos_limit() {
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
        let client = builder().authenticate(auth).build().await.unwrap();

        // 1. Exact limit
        let max_url = format!("/services/data/v60.0/query/{}", "A".repeat(100_000 - 32));
        let result = client.rest().query_more::<Dummy>(&max_url).await;
        if let Err(e) = &result {
            if let ForceError::InvalidInput(msg) = e {
                assert!(!msg.contains("100,000 bytes"), "100,000 should not trigger the limit error");
            }
        }

        // 2. Off-by-one limit
        let massive_url = "A".repeat(100_001);
        let result = client.rest().query_more::<Dummy>(&massive_url).await;

        assert!(
            result.is_err(),
            "👺 Havoc: query_more() allowed an input string > 100,000, risking DoS!"
        );
        let Err(e) = result else { panic!("expected error") };
        let err_msg = e.to_string();
        assert!(
            err_msg.contains("100,000 bytes"),
            "Expected error mentioning 100,000 bytes, got: {err_msg}"
        );

        // 3. Mutational boundary limit
        let mutational_url = "A".repeat(100_000 + 1024);
        let result = client.rest().query_more::<Dummy>(&mutational_url).await;
        assert!(result.is_err(), "Mutational boundary length should fail");
    }
}
