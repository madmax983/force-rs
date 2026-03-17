//! Regression test for `SmartIngest` batch size panic.
//!
//! Ensures that setting a huge batch size (e.g., `usize::MAX`) does not cause
//! an immediate panic due to allocation failure.

#[cfg(feature = "bulk")]
#[allow(clippy::unwrap_used)]
mod tests {
    use async_trait::async_trait;
    use force::api::bulk::JobOperation;
    use force::api::bulk::SmartIngest;
    use force::auth::{AccessToken, Authenticator, TokenResponse};
    use force::client::{ForceClient, builder};
    use force::error::Result as ForceResult;
    use futures::stream;
    use serde::Serialize;
    use std::fmt::Debug;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // Helper trait for unwrapping Result/Option in tests
    trait MustMsg<T> {
        fn must_msg(self, message: &str) -> T;
    }

    impl<T, E: Debug> MustMsg<T> for Result<T, E> {
        fn must_msg(self, message: &str) -> T {
            match self {
                Ok(value) => value,
                Err(error) => panic!("{message}: {error:?}"),
            }
        }
    }

    #[derive(Serialize, Clone, Debug)]
    struct TestRecord {
        id: String,
        name: String,
    }

    #[derive(Debug, Clone)]
    struct MockAuthenticator {
        token: String,
        instance_url: String,
    }

    impl MockAuthenticator {
        fn new(token: &str, instance_url: &str) -> Self {
            Self {
                token: token.to_string(),
                instance_url: instance_url.to_string(),
            }
        }
    }

    #[async_trait]
    impl Authenticator for MockAuthenticator {
        async fn authenticate(&self) -> ForceResult<AccessToken> {
            Ok(AccessToken::from_response(TokenResponse {
                access_token: self.token.clone(),
                instance_url: self.instance_url.clone(),
                token_type: "Bearer".to_string(),
                issued_at: "1704067200000".to_string(),
                signature: "test_sig".to_string(),
                expires_in: Some(7200),
                refresh_token: None,
            }))
        }

        async fn refresh(&self) -> ForceResult<AccessToken> {
            self.authenticate().await
        }
    }

    async fn create_test_client(mock_server_url: String) -> ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", &mock_server_url);
        builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("failed to create test client")
    }

    #[tokio::test]
    async fn test_smart_ingest_huge_batch_size_no_panic() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/ingest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "JOB_ID",
                "state": "Open",
                "operation": "insert",
                "object": "Account",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("PATCH"))
            .and(path("/services/data/v60.0/jobs/ingest/JOB_ID"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "JOB_ID",
                "state": "UploadComplete",
                "operation": "insert",
                "object": "Account",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/JOB_ID"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "JOB_ID",
                "state": "JobComplete",
                "operation": "insert",
                "object": "Account",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let stream = stream::empty::<TestRecord>();

        let result = SmartIngest::new(&handler, "Account", JobOperation::Insert)
            .batch_size(usize::MAX)
            .execute_stream(stream)
            .await;

        assert!(
            result.is_ok(),
            "Should complete successfully with empty stream even with huge batch size"
        );
    }
}
