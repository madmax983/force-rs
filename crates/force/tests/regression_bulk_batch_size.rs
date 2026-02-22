//! Regression tests for Bulk API batch size validation.
//!
//! Ensures that `process_csv_batches` and `SmartIngest` correctly handle invalid batch sizes.

#![cfg(feature = "bulk")]

use async_trait::async_trait;
use force::api::bulk::csv::{process_csv_batches, serialize_to_csv};
use force::api::bulk::types::JobOperation;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::Result;
use serde::{Deserialize, Serialize};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// Custom Authenticator for testing since MockAuthenticator is internal
#[derive(Debug)]
struct TestAuthenticator {
    instance_url: String,
}

#[async_trait]
impl Authenticator for TestAuthenticator {
    async fn authenticate(&self) -> Result<AccessToken> {
        Ok(AccessToken::from_response(TokenResponse {
            access_token: "test_token".to_string(),
            instance_url: self.instance_url.clone(),
            token_type: "Bearer".to_string(),
            issued_at: "1000".to_string(),
            signature: "sig".to_string(),
            expires_in: None,
            refresh_token: None,
        }))
    }

    async fn refresh(&self) -> Result<AccessToken> {
        self.authenticate().await
    }
}

#[test]
fn test_process_csv_batches_errors_on_zero_batch_size() {
    #[derive(Serialize, Deserialize, Debug)]
    struct Record {
        id: String,
    }

    let records = vec![Record { id: "1".to_string() }];
    let mut csv_data = Vec::new();
    serialize_to_csv(&records, &mut csv_data).unwrap();

    // Convert to slice for reading
    let reader = &csv_data[..];

    // This should error if batch_size is 0
    let result = process_csv_batches::<Record, _, _>(reader, 0, |_| Ok(()));

    // CURRENT BEHAVIOR: Ok(()) (Silent failure to process)
    // DESIRED BEHAVIOR: Err(ForceError::InvalidInput)
    if result.is_ok() {
        panic!("Expected error for batch_size 0, but got Ok");
    }
}

#[tokio::test]
async fn test_smart_ingest_errors_on_zero_batch_size() {
    let mock_server = MockServer::start().await;
    let auth = TestAuthenticator { instance_url: mock_server.uri() };

    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .unwrap();

    let handler = client.bulk();

    #[derive(Serialize, Clone)]
    struct Record {
        id: String,
    }

    let records = vec![Record { id: "1".to_string() }];
    let stream = futures::stream::iter(records);

    // Mock failure to catch if it proceeds to HTTP call
    Mock::given(method("POST"))
        .and(path("/services/data/v60.0/jobs/ingest"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    let result = handler.smart_ingest("Account", JobOperation::Insert)
        .batch_size(0)
        .execute_stream(stream)
        .await;

    // CURRENT BEHAVIOR: Tries to execute (and fails on mock)
    // DESIRED BEHAVIOR: Err(ForceError::InvalidInput) with specific message

    match result {
        Err(force::error::ForceError::InvalidInput(msg)) => {
            if !msg.contains("Batch size must be greater than 0") {
                 panic!("Unexpected error message: {}", msg);
            }
        }
        Err(e) => panic!("Expected InvalidInput error (batch size 0), got {:?}", e),
        Ok(_) => panic!("Expected error, got Ok"),
    }
}
