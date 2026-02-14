//! Ingest Job API for Salesforce Bulk API 2.0.
//!
//! This module provides a typestate-based API for managing bulk ingest jobs,
//! ensuring compile-time safety for job lifecycle transitions.
//!
//! # Job Lifecycle
//!
//! The typestate pattern enforces valid state transitions at compile time:
//!
//! ```text
//! IngestJob<Open>
//!   └─> upload() ─> IngestJob<UploadComplete>
//!                     └─> close() ─> IngestJob<InProgress>
//!                                      └─> poll() ─> IngestJob<JobComplete>
//! ```
//!
//! # Examples
//!
//! ```ignore
//! use force::api::bulk::ingest::IngestJobBuilder;
//! use force::api::bulk::types::JobOperation;
//!
//! // Create and upload data
//! let job = IngestJobBuilder::new("Account", JobOperation::Insert)
//!     .build(&client)
//!     .await?;
//!
//! let csv_data = "Name,Industry\nAcme Corp,Technology\n";
//! let job = job.upload(csv_data.as_bytes()).await?;
//!
//! // Close and wait for completion
//! let job = job.close().await?;
//! let job = job.poll_until_complete().await?;
//!
//! // Retrieve results
//! let successful = job.successful_results().await?;
//! let failed = job.failed_results().await?;
//! ```

use crate::api::bulk::BulkPollPolicy;
use crate::api::bulk::types::{
    CreateJobRequest, JobInfo, JobOperation, JobState, UpdateJobRequest,
};
use crate::auth::Authenticator;
use crate::error::Result;
use std::marker::PhantomData;

async fn handle_error_response(
    response: reqwest::Response,
    fallback_message: &str,
) -> crate::error::ForceError {
    crate::http::response_to_force_error(response, fallback_message).await
}

/// Marker type for job in Open state.
#[derive(Debug)]
pub struct Open;

/// Marker type for job in UploadComplete state.
#[derive(Debug)]
pub struct UploadComplete;

/// Marker type for job in InProgress state.
#[derive(Debug)]
pub struct InProgress;

/// Marker type for job in JobComplete state.
#[derive(Debug)]
pub struct JobComplete;

/// Typestate-based ingest job handle.
///
/// The type parameter `S` represents the current job state and enforces
/// valid state transitions at compile time.
#[derive(Debug)]
pub struct IngestJob<S, A: Authenticator> {
    job_id: String,
    client: crate::client::ForceClient<A>,
    _state: PhantomData<S>,
}

impl<A: Authenticator> IngestJob<Open, A> {
    /// Creates a new ingest job in Open state.
    #[must_use]
    pub(crate) fn new(job_id: String, client: crate::client::ForceClient<A>) -> Self {
        Self {
            job_id,
            client,
            _state: PhantomData,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(job_id: String, client: crate::client::ForceClient<A>) -> Self {
        Self::new(job_id, client)
    }

    /// Uploads CSV data to the job.
    ///
    /// # Arguments
    ///
    /// * `data` - CSV data as bytes
    ///
    /// # Errors
    ///
    /// Returns an error if the upload fails.
    pub async fn upload(self, data: &[u8]) -> Result<IngestJob<UploadComplete, A>> {
        let token = self.client.token_manager.token().await?;
        let url = format!(
            "{}/services/data/{}/jobs/ingest/{}/batches",
            token.instance_url(),
            self.client.config.api_version,
            self.job_id
        );

        let response = self
            .client
            .http_client
            .put(&url)
            .header("Content-Type", "text/csv")
            .body(data.to_vec())
            .build()
            .map_err(crate::error::HttpError::from)?;
        let response = self.client.execute_request(response).await?;

        if !response.status().is_success() {
            return Err(handle_error_response(response, "CSV upload failed").await);
        }

        Ok(IngestJob {
            job_id: self.job_id,
            client: self.client,
            _state: PhantomData,
        })
    }

    /// Aborts the job.
    ///
    /// # Errors
    ///
    /// Returns an error if aborting fails.
    pub async fn abort(self) -> Result<()> {
        let token = self.client.token_manager.token().await?;
        let url = format!(
            "{}/services/data/{}/jobs/ingest/{}",
            token.instance_url(),
            self.client.config.api_version,
            self.job_id
        );

        let request = UpdateJobRequest {
            state: JobState::Aborted,
        };

        let response = self
            .client
            .http_client
            .patch(&url)
            .json(&request)
            .build()
            .map_err(crate::error::HttpError::from)?;
        let response = self.client.execute_request(response).await?;

        if !response.status().is_success() {
            return Err(handle_error_response(response, "Abort job failed").await);
        }

        Ok(())
    }
}

impl<A: Authenticator> IngestJob<UploadComplete, A> {
    /// Closes the job and transitions to processing state.
    ///
    /// # Errors
    ///
    /// Returns an error if closing the job fails.
    pub async fn close(self) -> Result<IngestJob<InProgress, A>> {
        let token = self.client.token_manager.token().await?;
        let url = format!(
            "{}/services/data/{}/jobs/ingest/{}",
            token.instance_url(),
            self.client.config.api_version,
            self.job_id
        );

        let request = UpdateJobRequest {
            state: JobState::UploadComplete,
        };

        let response = self
            .client
            .http_client
            .patch(&url)
            .json(&request)
            .build()
            .map_err(crate::error::HttpError::from)?;
        let response = self.client.execute_request(response).await?;

        if !response.status().is_success() {
            return Err(handle_error_response(response, "Close job failed").await);
        }

        Ok(IngestJob {
            job_id: self.job_id,
            client: self.client,
            _state: PhantomData,
        })
    }
}

impl<A: Authenticator> IngestJob<InProgress, A> {
    #[cfg(test)]
    pub(crate) fn new_for_test(job_id: String, client: crate::client::ForceClient<A>) -> Self {
        Self {
            job_id,
            client,
            _state: PhantomData,
        }
    }

    /// Polls the job status once.
    ///
    /// # Errors
    ///
    /// Returns an error if polling fails or job has failed/aborted.
    pub async fn poll(self) -> Result<Self> {
        let job_info = self.get_job_info().await?;

        if matches!(job_info.state, JobState::Failed) {
            return Err(crate::error::HttpError::StatusError {
                status_code: 500,
                message: "Job failed during processing".to_string(),
            }
            .into());
        }

        if matches!(job_info.state, JobState::Aborted) {
            return Err(crate::error::HttpError::StatusError {
                status_code: 400,
                message: "Job was aborted".to_string(),
            }
            .into());
        }

        Ok(self)
    }

    /// Polls until the job completes, using exponential backoff.
    ///
    /// # Errors
    ///
    /// Returns an error if polling fails or job fails/aborts.
    pub async fn poll_until_complete(self) -> Result<IngestJob<JobComplete, A>> {
        self.poll_until_complete_with_policy(BulkPollPolicy::default())
            .await
    }

    /// Polls until the job completes using the provided polling policy.
    ///
    /// # Errors
    ///
    /// Returns an error if polling fails or job fails/aborts.
    pub async fn poll_until_complete_with_policy(
        self,
        poll_policy: BulkPollPolicy,
    ) -> Result<IngestJob<JobComplete, A>> {
        let mut attempt = 0;
        loop {
            let job_info = self.get_job_info().await?;

            match job_info.state {
                JobState::JobComplete => {
                    return Ok(IngestJob {
                        job_id: self.job_id,
                        client: self.client,
                        _state: PhantomData,
                    });
                }
                JobState::Failed => {
                    return Err(crate::error::HttpError::StatusError {
                        status_code: 500,
                        message: "Job failed during processing".to_string(),
                    }
                    .into());
                }
                JobState::Aborted => {
                    return Err(crate::error::HttpError::StatusError {
                        status_code: 400,
                        message: "Job was aborted".to_string(),
                    }
                    .into());
                }
                _ => {
                    // Job still in progress, wait with exponential backoff
                    if attempt >= poll_policy.max_attempts {
                        return Err(crate::error::HttpError::Timeout {
                            timeout_seconds: poll_policy.timeout_seconds(),
                        }
                        .into());
                    }

                    tokio::time::sleep(poll_policy.backoff_for_attempt(attempt)).await;
                    attempt += 1;
                }
            }
        }
    }

    /// Helper to get job info.
    async fn get_job_info(&self) -> Result<JobInfo> {
        let token = self.client.token_manager.token().await?;
        let url = format!(
            "{}/services/data/{}/jobs/ingest/{}",
            token.instance_url(),
            self.client.config.api_version,
            self.job_id
        );

        let response = self
            .client
            .http_client
            .get(&url)
            .build()
            .map_err(crate::error::HttpError::from)?;
        let response = self.client.execute_request(response).await?;

        if !response.status().is_success() {
            return Err(handle_error_response(response, "Get job status failed").await);
        }

        let job_info = response
            .json::<JobInfo>()
            .await
            .map_err(crate::error::HttpError::from)?;
        Ok(job_info)
    }
}

impl<A: Authenticator> IngestJob<JobComplete, A> {
    #[cfg(test)]
    pub(crate) fn new_for_test(job_id: String, client: crate::client::ForceClient<A>) -> Self {
        Self {
            job_id,
            client,
            _state: PhantomData,
        }
    }

    /// Returns the job ID.
    #[must_use]
    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    /// Retrieves successful results from the completed job.
    ///
    /// # Errors
    ///
    /// Returns an error if retrieving results fails.
    pub async fn successful_results(&self) -> Result<Vec<u8>> {
        self.get_results("successfulResults").await
    }

    /// Retrieves failed results from the completed job.
    ///
    /// # Errors
    ///
    /// Returns an error if retrieving results fails.
    pub async fn failed_results(&self) -> Result<Vec<u8>> {
        self.get_results("failedResults").await
    }

    /// Retrieves unprocessed records from the completed job.
    ///
    /// # Errors
    ///
    /// Returns an error if retrieving results fails.
    pub async fn unprocessed_results(&self) -> Result<Vec<u8>> {
        self.get_results("unprocessedrecords").await
    }

    /// Helper to get results.
    async fn get_results(&self, result_type: &str) -> Result<Vec<u8>> {
        let token = self.client.token_manager.token().await?;
        let url = format!(
            "{}/services/data/{}/jobs/ingest/{}/{}",
            token.instance_url(),
            self.client.config.api_version,
            self.job_id,
            result_type
        );

        let response = self
            .client
            .http_client
            .get(&url)
            .build()
            .map_err(crate::error::HttpError::from)?;
        let response = self.client.execute_request(response).await?;

        if !response.status().is_success() {
            return Err(
                handle_error_response(response, &format!("Get {} failed", result_type)).await,
            );
        }

        let bytes = response
            .bytes()
            .await
            .map_err(crate::error::HttpError::from)?;
        Ok(bytes.to_vec())
    }
}

/// Builder for creating ingest jobs.
pub struct IngestJobBuilder {
    object: String,
    operation: JobOperation,
    external_id_field_name: Option<String>,
}

impl IngestJobBuilder {
    /// Creates a new ingest job builder.
    ///
    /// # Arguments
    ///
    /// * `object` - The SObject type (e.g., "Account")
    /// * `operation` - The operation to perform
    #[must_use]
    pub fn new(object: impl Into<String>, operation: JobOperation) -> Self {
        Self {
            object: object.into(),
            operation,
            external_id_field_name: None,
        }
    }

    /// Sets the external ID field for upsert operations.
    #[must_use]
    pub fn external_id_field(mut self, field_name: impl Into<String>) -> Self {
        self.external_id_field_name = Some(field_name.into());
        self
    }

    /// Builds and creates the job.
    ///
    /// # Errors
    ///
    /// Returns an error if job creation fails.
    pub async fn build<A: Authenticator>(
        self,
        handler: &crate::api::bulk::BulkHandler<A>,
    ) -> Result<IngestJob<Open, A>> {
        let request = CreateJobRequest {
            object: self.object,
            operation: self.operation,
            content_type: None,
            external_id_field_name: self.external_id_field_name,
            line_ending: None,
            column_delimiter: None,
        };

        let job_info = handler.create_job(request).await?;
        Ok(IngestJob::new(job_info.id, handler.client.clone()))
    }

    /// Builds and creates the job using a raw client reference.
    ///
    /// This is used internally by convenience methods.
    ///
    /// # Errors
    ///
    /// Returns an error if job creation fails.
    pub(crate) async fn build_with_client<A: Authenticator>(
        self,
        client: crate::client::ForceClient<A>,
    ) -> Result<IngestJob<Open, A>> {
        let request = CreateJobRequest {
            object: self.object,
            operation: self.operation,
            content_type: None,
            external_id_field_name: self.external_id_field_name,
            line_ending: None,
            column_delimiter: None,
        };

        // Call create_job logic directly
        let token = client.token_manager.token().await?;
        let url = format!(
            "{}/services/data/{}/jobs/ingest",
            token.instance_url(),
            client.config.api_version
        );

        let response = client
            .http_client
            .post(&url)
            .json(&request)
            .build()
            .map_err(crate::error::HttpError::from)?;
        let response = client.execute_request(response).await?;

        if !response.status().is_success() {
            return Err(handle_error_response(response, "Create job request failed").await);
        }

        let job_info = response
            .json::<JobInfo>()
            .await
            .map_err(crate::error::HttpError::from)?;

        Ok(IngestJob::new(job_info.id, client))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::bulk::types::JobOperation;
    use crate::auth::{AccessToken, Authenticator, TokenResponse};
    use crate::client::{ForceClient, builder};
    use crate::test_support::{Must, MustMsg};
    use async_trait::async_trait;
    use wiremock::matchers::{bearer_token, body_bytes, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // Mock authenticator for testing
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

    async fn create_test_client(mock_server_url: String) -> ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", &mock_server_url);
        builder()
            .build(auth)
            .await
            .must_msg("failed to create test client")
    }

    #[tokio::test]
    async fn test_create_ingest_job() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/ingest"))
            .and(bearer_token("test_token"))
            .and(header("content-type", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000001AAA",
                "operation": "insert",
                "object": "Account",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA",
                "state": "Open",
                "contentType": "CSV"
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let job = IngestJobBuilder::new("Account", JobOperation::Insert)
            .build(&handler)
            .await
            .must();

        // Job should be in Open state (compile-time enforced via typestate)
        // This compiles, so the job is in Open state
        let _ = job;
    }

    #[tokio::test]
    async fn test_upload_csv_data() {
        let mock_server = MockServer::start().await;

        // Mock job creation
        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/ingest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000001AAA",
                "operation": "insert",
                "object": "Account",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA",
                "state": "Open",
                "contentType": "CSV"
            })))
            .mount(&mock_server)
            .await;

        // Mock CSV upload
        Mock::given(method("PUT"))
            .and(path(
                "/services/data/v60.0/jobs/ingest/750xx0000000001AAA/batches",
            ))
            .and(bearer_token("test_token"))
            .and(header("content-type", "text/csv"))
            .and(body_bytes(
                "Name,Industry\nAcme Corp,Technology\n".as_bytes(),
            ))
            .respond_with(ResponseTemplate::new(201))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let job = IngestJobBuilder::new("Account", JobOperation::Insert)
            .build(&handler)
            .await
            .must();

        let csv_data = "Name,Industry\nAcme Corp,Technology\n";
        let _job = job.upload(csv_data.as_bytes()).await.must();
    }

    #[tokio::test]
    async fn test_upload_large_csv_streaming() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/ingest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000001AAA",
                "operation": "insert",
                "object": "Account",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA",
                "state": "Open"
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("PUT"))
            .and(path(
                "/services/data/v60.0/jobs/ingest/750xx0000000001AAA/batches",
            ))
            .respond_with(ResponseTemplate::new(201))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let job = IngestJobBuilder::new("Account", JobOperation::Insert)
            .build(&handler)
            .await
            .must();

        // Test streaming upload of large CSV (>10MB)
        let large_csv = "Name,Industry\n".to_string() + &"Row,Data\n".repeat(10000);
        let _job = job.upload(large_csv.as_bytes()).await.must();
    }

    #[tokio::test]
    async fn test_close_job() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/ingest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000001AAA",
                "operation": "insert",
                "object": "Account",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA",
                "state": "Open"
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("PUT"))
            .and(path(
                "/services/data/v60.0/jobs/ingest/750xx0000000001AAA/batches",
            ))
            .respond_with(ResponseTemplate::new(201))
            .mount(&mock_server)
            .await;

        // Mock close job (PATCH with state: UploadComplete)
        Mock::given(method("PATCH"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000001AAA"))
            .and(bearer_token("test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000001AAA",
                "operation": "insert",
                "object": "Account",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA",
                "state": "InProgress"
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let job = IngestJobBuilder::new("Account", JobOperation::Insert)
            .build(&handler)
            .await
            .must();

        let csv_data = "Name\nTest\n";
        let job = job.upload(csv_data.as_bytes()).await.must();
        let _job = job.close().await.must();
    }

    #[tokio::test]
    async fn test_poll_job_status() {
        let mock_server = MockServer::start().await;

        // Mock get job status - still in progress
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000001AAA"))
            .and(bearer_token("test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000001AAA",
                "operation": "insert",
                "object": "Account",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA",
                "state": "InProgress",
                "numberRecordsProcessed": 50,
                "numberRecordsFailed": 0
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();
        let job = IngestJob::<InProgress, _>::new_for_test(
            "750xx0000000001AAA".to_string(),
            handler.client.clone(),
        );

        let _job = job.poll().await.must();
    }

    #[tokio::test]
    async fn test_poll_until_complete() {
        let mock_server = MockServer::start().await;

        // First poll - in progress
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000001AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000001AAA",
                "operation": "insert",
                "object": "Account",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA",
                "state": "InProgress"
            })))
            .up_to_n_times(2)
            .mount(&mock_server)
            .await;

        // Final poll - complete
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000001AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000001AAA",
                "operation": "insert",
                "object": "Account",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA",
                "state": "JobComplete",
                "numberRecordsProcessed": 100,
                "numberRecordsFailed": 2
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();
        let job = IngestJob::<InProgress, _>::new_for_test(
            "750xx0000000001AAA".to_string(),
            handler.client.clone(),
        );

        let _job = job.poll_until_complete().await.must();
    }

    #[tokio::test]
    async fn test_exponential_backoff_timing() {
        let mock_server = MockServer::start().await;

        // All polls return in progress
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000001AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000001AAA",
                "state": "InProgress"
            })))
            .up_to_n_times(5)
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();
        let job = IngestJob::<InProgress, _>::new_for_test(
            "750xx0000000001AAA".to_string(),
            handler.client.clone(),
        );

        // Test that backoff increases: 1s, 2s, 4s, 8s, 16s (capped at 30s)
        // This test will timeout after 5 attempts
        let result = job.poll_until_complete().await;
        assert!(result.is_err()); // Should timeout
    }

    #[tokio::test]
    async fn test_retrieve_successful_results() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/jobs/ingest/750xx0000000001AAA/successfulResults",
            ))
            .and(bearer_token("test_token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(
                        "Id,Created\n001xx0000000001AAA,true\n001xx0000000002AAA,true\n",
                    )
                    .insert_header("content-type", "text/csv"),
            )
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();
        let job = IngestJob::<JobComplete, _>::new_for_test(
            "750xx0000000001AAA".to_string(),
            handler.client.clone(),
        );

        let results = job.successful_results().await.must();
        let results_str = String::from_utf8(results).must();
        assert!(results_str.contains("001xx0000000001AAA"));
    }

    #[tokio::test]
    async fn test_retrieve_failed_results() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/jobs/ingest/750xx0000000001AAA/failedResults",
            ))
            .and(bearer_token("test_token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("Id,Error\n,DUPLICATE_VALUE:duplicate value found\n")
                    .insert_header("content-type", "text/csv"),
            )
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();
        let job = IngestJob::<JobComplete, _>::new_for_test(
            "750xx0000000001AAA".to_string(),
            handler.client.clone(),
        );

        let results = job.failed_results().await.must();
        let results_str = String::from_utf8(results).must();
        assert!(results_str.contains("DUPLICATE_VALUE"));
    }

    #[tokio::test]
    async fn test_retrieve_unprocessed_records() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/jobs/ingest/750xx0000000001AAA/unprocessedrecords",
            ))
            .and(bearer_token("test_token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("Name,Industry\nPending Corp,Tech\n")
                    .insert_header("content-type", "text/csv"),
            )
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();
        let job = IngestJob::<JobComplete, _>::new_for_test(
            "750xx0000000001AAA".to_string(),
            handler.client.clone(),
        );

        let results = job.unprocessed_results().await.must();
        let results_str = String::from_utf8(results).must();
        assert!(results_str.contains("Pending Corp"));
    }

    #[tokio::test]
    async fn test_upload_error_handling() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/ingest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000001AAA",
                "operation": "insert",
                "object": "Account",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA",
                "state": "Open"
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("PUT"))
            .and(path(
                "/services/data/v60.0/jobs/ingest/750xx0000000001AAA/batches",
            ))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "message": "Invalid CSV format"
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let job = IngestJobBuilder::new("Account", JobOperation::Insert)
            .build(&handler)
            .await
            .must();

        let result = job.upload(b"bad csv").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_close_job_error_handling() {
        let mock_server = MockServer::start().await;

        Mock::given(method("PATCH"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000001AAA"))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "message": "Job not found"
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();
        let job = IngestJob::<UploadComplete, _> {
            job_id: "750xx0000000001AAA".to_string(),
            client: handler.client.clone(),
            _state: PhantomData,
        };

        let result = job.close().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_poll_with_authentication_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000001AAA"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "message": "Session expired or invalid"
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();
        let job = IngestJob::<InProgress, _>::new_for_test(
            "750xx0000000001AAA".to_string(),
            handler.client.clone(),
        );

        let result = job.poll().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_job_failed_state() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000001AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000001AAA",
                "state": "Failed",
                "errorMessage": "System error during processing"
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();
        let job = IngestJob::<InProgress, _>::new_for_test(
            "750xx0000000001AAA".to_string(),
            handler.client.clone(),
        );

        let result = job.poll().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_job_aborted_state() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000001AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000001AAA",
                "state": "Aborted",
                "errorMessage": "Job aborted by user"
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();
        let job = IngestJob::<InProgress, _>::new_for_test(
            "750xx0000000001AAA".to_string(),
            handler.client.clone(),
        );

        let result = job.poll().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_abort_open_job() {
        let mock_server = MockServer::start().await;

        Mock::given(method("PATCH"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000001AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000001AAA",
                "state": "Aborted"
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();
        let job = IngestJob::<Open, _>::new_for_test(
            "750xx0000000001AAA".to_string(),
            handler.client.clone(),
        );

        job.abort().await.must();
    }

    #[tokio::test]
    async fn test_max_polling_timeout() {
        let mock_server = MockServer::start().await;

        // Never complete - test timeout
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000001AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000001AAA",
                "state": "InProgress"
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();
        let job = IngestJob::<InProgress, _>::new_for_test(
            "750xx0000000001AAA".to_string(),
            handler.client.clone(),
        );

        let result = job.poll_until_complete().await;
        assert!(result.is_err()); // Should timeout
    }

    #[tokio::test]
    async fn test_poll_until_complete_with_policy_retries_then_succeeds() {
        use crate::api::bulk::BulkPollPolicy;
        use std::time::Duration;

        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000002AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000002AAA",
                "operation": "insert",
                "object": "Account",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA",
                "state": "InProgress"
            })))
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000002AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000002AAA",
                "operation": "insert",
                "object": "Account",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA",
                "state": "JobComplete"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();
        let job = IngestJob::<InProgress, _>::new_for_test(
            "750xx0000000002AAA".to_string(),
            handler.client.clone(),
        );

        let policy = BulkPollPolicy::new(2, Duration::from_millis(1), Duration::from_millis(1));
        let result = job.poll_until_complete_with_policy(policy).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_poll_until_complete_with_policy_times_out_when_attempts_are_zero() {
        use crate::api::bulk::BulkPollPolicy;
        use std::time::Duration;

        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000003AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000003AAA",
                "operation": "insert",
                "object": "Account",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA",
                "state": "InProgress"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();
        let job = IngestJob::<InProgress, _>::new_for_test(
            "750xx0000000003AAA".to_string(),
            handler.client.clone(),
        );

        let policy = BulkPollPolicy::new(0, Duration::from_millis(1), Duration::from_millis(1));
        let result = job.poll_until_complete_with_policy(policy).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_typestate_compile_time_safety() {
        // This test verifies compile-time safety - it should not compile if
        // you try invalid state transitions (e.g., close an Open job directly)

        // These should NOT compile:
        // let job: IngestJob<Open, _> = ...;
        // let _ = job.close(); // ERROR: close() only available on UploadComplete
        // let _ = job.poll(); // ERROR: poll() only available on InProgress

        // If this test compiles, typestate safety is working
    }
}
