#![allow(missing_docs)]
#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use force::api::RestOperation;
    use force::auth::{AccessToken, Authenticator, TokenResponse};
    use force::client::builder;
    use serde::Deserialize;
    use wiremock::MockServer;

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
    async fn test_ssrf_backslash_bypass() {
        let mock_server = MockServer::start().await;

        let instance_url = mock_server.uri();
        let auth = MockAuthenticator::new(&instance_url);
        let client = builder()
            .authenticate(auth)
            .build()
            .await
            .unwrap_or_else(|e| panic!("Failed to build client: {e}"));
        let handler = client.rest();

        // This is the new backslash SSRF payload.
        let next_records_url = r"\\evil.com/services/data/v60.0/query/01g";

        let result: force::error::Result<force::types::QueryResult<Dummy>> =
            handler.query_more::<Dummy>(next_records_url).await;

        // Ensure that the security blocks it (either the new validate_url_path check or the request sender's security check for origin mismatch)
        let Err(force::error::ForceError::InvalidInput(msg)) = result else {
            panic!("Expected InvalidInput error blocking the backslash SSRF attack!");
        };
        // The error could be the new validate_url_path error or the existing strict origin check if it's evaluated later.
        assert!(
            msg.contains("URL path contains invalid path traversal characters")
                || msg.contains("URL path resolves to a different host")
                || msg.contains("Security Error: nextRecordsUrl origin")
        );
    }
}
