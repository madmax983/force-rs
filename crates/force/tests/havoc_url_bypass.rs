#![allow(missing_docs)]
#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use force::api::RestOperation;
    use force::auth::{AccessToken, Authenticator, TokenResponse};
    use force::client::builder;
    use serde::Deserialize;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[derive(Debug, Clone)]
    struct MockAuthenticator {
        instance_url: String,
    }

    impl MockAuthenticator {
        fn new(instance_url: &str) -> Self {
            Self {
                instance_url: instance_url.to_string(),
            }
        }
    }

    #[async_trait]
    impl Authenticator for MockAuthenticator {
        async fn authenticate(&self) -> force::error::Result<AccessToken> {
            Ok(AccessToken::from_response(TokenResponse {
                access_token: secrecy::SecretString::new("test_token".to_string().into()),
                instance_url: self.instance_url.clone(),
                token_type: "Bearer".to_string(),
                issued_at: "1704067200000".to_string(),
                signature: "sig".to_string(),
                expires_in: None,
                refresh_token: None,
            }))
        }

        async fn refresh(&self) -> force::error::Result<AccessToken> {
            self.authenticate().await
        }
    }

    #[derive(Debug, Deserialize)]
    struct Dummy {}

    #[tokio::test]
    async fn test_havoc_url_bypass() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/@evil.com/services/data/v60.0/query/01g"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "totalSize": 0,
                "done": true,
                "records": []
            })))
            .mount(&mock_server)
            .await;

        let instance_url = mock_server.uri();
        let auth = MockAuthenticator::new(&instance_url);
        let client = builder()
            .authenticate(auth)
            .build()
            .await
            .unwrap_or_else(|e| panic!("Failed to build client: {e}"));
        let handler = client.rest();

        // This is the SSRF payload.
        let next_records_url = "@evil.com/services/data/v60.0/query/01g";

        // Before the fix, this would create `http://127.0.0.1:port@evil.com/...`
        // which sends the request to `evil.com`.
        // After the fix, it creates `http://127.0.0.1:port/@evil.com/...`
        // which is a relative path sent to the mock server!
        let result: Result<force::types::QueryResult<Dummy>, force::error::ForceError> =
            handler.query_more::<Dummy>(next_records_url).await;

        // Because the mock server is configured to handle the relative path `/@evil.com/...`,
        // it should return Ok!
        assert!(
            result.is_ok(),
            "The request failed! Expected the relative path to hit the mock server successfully. Result: {result:?}"
        );
    }
}
