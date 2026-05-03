//! Integration test for `SmartIngest` multi-batch behavior.
//! Ensures that optimizations to buffer allocation don't break multi-batch uploads.

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
    use wiremock::matchers::{body_string, method, path};
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
                access_token: secrecy::SecretString::new(self.token.clone().into()),
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
    async fn test_smart_ingest_multi_batch_allocation() {
        let mock_server = MockServer::start().await;

        // Mock: Create Job
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
            .expect(1)
            .mount(&mock_server)
            .await;

        // Bulk API 2.0 allows only one upload per job. The small batch size
        // exercises internal buffer flushes, but the HTTP contract is a
        // single PUT containing all rows with one header.
        Mock::given(method("PUT"))
            .and(path("/services/data/v60.0/jobs/ingest/JOB_ID/batches"))
            .and(body_string("id,name\n001,Batch1\n002,Batch2\n003,Batch3\n"))
            .respond_with(ResponseTemplate::new(201))
            .expect(1)
            .mount(&mock_server)
            .await;

        // Mock: Close Job
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
            .expect(1)
            .mount(&mock_server)
            .await;

        // Mock: Poll (Complete)
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
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let records = vec![
            TestRecord {
                id: "001".to_string(),
                name: "Batch1".to_string(),
            },
            TestRecord {
                id: "002".to_string(),
                name: "Batch2".to_string(),
            },
            TestRecord {
                id: "003".to_string(),
                name: "Batch3".to_string(),
            },
        ];
        let stream = stream::iter(records);

        // Batch size 1 forces three internal buffer flushes, while Bulk API
        // 2.0 still receives one upload for the job.
        let result = SmartIngest::new(&handler, "Account", JobOperation::Insert)
            .batch_size(1)
            .execute_stream(stream)
            .await;

        assert!(result.is_ok());
    }
}
