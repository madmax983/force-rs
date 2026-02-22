//! Havoc test for `SmartIngest` OOM protection.
//!
//! Verifies that `SmartIngest` enforces a reasonable upper bound on `batch_size`
//! (e.g., 50,000 records) to preventing memory exhaustion when users request
//! huge batch sizes.
//!
//! "If I can crash it, I win." - Havoc

#![cfg(feature = "bulk")]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use async_trait::async_trait;
use force::api::bulk::smart_ingest::SmartIngest;
use force::api::bulk::types::JobOperation;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::{ForceClient, builder};
use force::error::Result as ForceResult;
use futures::stream;
use serde::Serialize;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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
            signature: "sig".to_string(),
            expires_in: None,
            refresh_token: None,
        }))
    }

    async fn refresh(&self) -> ForceResult<AccessToken> {
        self.authenticate().await
    }
}

async fn create_client(mock_server_url: String) -> ForceClient<MockAuthenticator> {
    let auth = MockAuthenticator::new("test_token", &mock_server_url);
    builder()
        .authenticate(auth)
        .build()
        .await
        .expect("Failed to build client")
}

#[tokio::test]
async fn test_smart_ingest_enforces_limit() {
    let mock_server = MockServer::start().await;

    // 1. Create Job
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

    // 2. Upload Batch
    // We expect exactly 2 calls if the limit is enforced (50k + 10k = 60k).
    // Currently (BROKEN), it will be 1 call (60k).
    // HAVOC: We assert the safe behavior.
    Mock::given(method("PUT"))
        .and(path("/services/data/v60.0/jobs/ingest/JOB_ID/batches"))
        .respond_with(ResponseTemplate::new(201))
        .expect(2) // The critical assertion: must split into 2 batches
        .mount(&mock_server)
        .await;

    // 3. Close Job
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

    // 4. Poll Status
    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/jobs/ingest/JOB_ID"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "JOB_ID",
            "state": "JobComplete", // Skip straight to complete
            "operation": "insert",
            "object": "Account",
            "createdDate": "2024-01-01T00:00:00.000Z",
            "createdById": "005xx0000000001AAA",
            "numberRecordsProcessed": 60000,
            "numberRecordsFailed": 0
        })))
        .mount(&mock_server)
        .await;

    let client = create_client(mock_server.uri()).await;
    let handler = client.bulk();

    // Generate 60,000 records
    let records: Vec<TestRecord> = (0..60_000)
        .map(|i| TestRecord {
            id: format!("ID_{i}"),
            name: format!("Name_{i}"),
        })
        .collect();

    let stream = stream::iter(records);

    // Request 100,000 batch size (unsafe!)
    // The implementation SHOULD clamp this to 50,000.
    let result = SmartIngest::new(&handler, "Account", JobOperation::Insert)
        .batch_size(100_000)
        .execute_stream(stream)
        .await;

    assert!(result.is_ok(), "Ingest failed: {:?}", result.err());
}
