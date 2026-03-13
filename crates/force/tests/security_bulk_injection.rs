//! Security tests for Bulk API injection vulnerabilities.
#![cfg(feature = "bulk")]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use async_trait::async_trait;
use force::api::{CreateJobRequest, JobOperation};
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::{ForceError, Result};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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
    async fn authenticate(&self) -> Result<AccessToken> {
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

    async fn refresh(&self) -> Result<AccessToken> {
        self.authenticate().await
    }
}

#[tokio::test]
async fn test_create_job_rejects_invalid_object_name() {
    let mock_server = MockServer::start().await;
    let auth = MockAuthenticator::new("test_token", &mock_server.uri());
    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .expect("Failed to build client");

    // The client should reject the invalid object name BEFORE sending the request.
    Mock::given(method("POST"))
        .and(path("/services/data/v60.0/jobs/ingest"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock_server)
        .await;

    let request = CreateJobRequest {
        object: "Account; DROP TABLE".to_string(),
        operation: JobOperation::Insert,
        content_type: None,
        external_id_field_name: None,
        line_ending: None,
        column_delimiter: None,
    };

    let result = client.bulk().create_job(request).await;
    assert!(result.is_err(), "Client should reject invalid object name");

    if let Err(ForceError::InvalidInput(msg)) = result {
        assert!(msg.contains("SObject name contains invalid characters"));
    } else {
        panic!("Expected InvalidInput error, got {result:?}");
    }
}

#[tokio::test]
async fn test_create_job_rejects_invalid_external_id_field() {
    let mock_server = MockServer::start().await;
    let auth = MockAuthenticator::new("test_token", &mock_server.uri());
    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .expect("Failed to build client");

    // Client should reject invalid field name (with dots)
    Mock::given(method("POST"))
        .and(path("/services/data/v60.0/jobs/ingest"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock_server)
        .await;

    let request = CreateJobRequest {
        object: "Account".to_string(),
        operation: JobOperation::Upsert,
        content_type: None,
        external_id_field_name: Some("Parent.ExternalId".to_string()),
        line_ending: None,
        column_delimiter: None,
    };

    let result = client.bulk().create_job(request).await;
    assert!(
        result.is_err(),
        "Client should reject invalid external ID field"
    );

    if let Err(ForceError::InvalidInput(msg)) = result {
        assert!(msg.contains("External ID field name contains invalid characters"));
    } else {
        panic!("Expected InvalidInput error, got {result:?}");
    }
}
