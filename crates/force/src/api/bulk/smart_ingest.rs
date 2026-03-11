//! Smart Ingest - High-level streaming bulk ingest.
//!
//! This module provides the `SmartIngest` utility for efficiently uploading
//! large datasets to Salesforce Bulk API 2.0 using async streams and automatic batching.

use super::BulkHandler;
use crate::api::bulk::csv;
use crate::api::bulk::types::{
    CreateJobRequest, JobInfo, JobOperation, JobState, UpdateJobRequest,
};
use crate::error::Result;
use futures::{Stream, StreamExt};
use serde::Serialize;

/// A high-level, streaming ingestion utility for Salesforce Bulk API 2.0.
///
/// `SmartIngest` orchestrates the entire lifecycle of a bulk ingest job:
/// 1. Creates a job.
/// 2. Streams records from an async iterator.
/// 3. Buffers records into optimal batches (default 10,000 records).
/// 4. Serializes chunks to CSV (handling headers automatically).
/// 5. Uploads batches sequentially.
/// 6. Closes the job.
/// 7. Polls for completion.
#[derive(Debug)]
pub struct SmartIngest<'a, A: crate::auth::Authenticator> {
    handler: &'a BulkHandler<A>,
    object: String,
    operation: JobOperation,
    external_id_field: Option<String>,
    batch_size: usize,
}

impl<'a, A: crate::auth::Authenticator> SmartIngest<'a, A> {
    /// Creates a new SmartIngest builder.
    ///
    /// # Arguments
    ///
    /// * `handler` - The BulkHandler instance.
    /// * `object` - The SObject type (e.g., "Account").
    /// * `operation` - The operation to perform (Insert, Update, etc.).
    #[must_use]
    pub fn new(
        handler: &'a BulkHandler<A>,
        object: impl Into<String>,
        operation: JobOperation,
    ) -> Self {
        Self {
            handler,
            object: object.into(),
            operation,
            external_id_field: None,
            batch_size: 10_000,
        }
    }

    /// Sets the external ID field for upsert operations.
    #[must_use]
    pub fn external_id_field(mut self, field: impl Into<String>) -> Self {
        self.external_id_field = Some(field.into());
        self
    }

    /// Sets the batch size (number of records per chunk).
    ///
    /// Defaults to 10,000.
    #[must_use]
    pub fn batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    /// Executes the ingest job using the provided stream of records.
    ///
    /// This method consumes the stream, buffers records, and uploads them in batches.
    ///
    /// # Arguments
    ///
    /// * `stream` - An async stream of serializable records.
    ///
    /// # Returns
    ///
    /// The final `JobInfo` after the job completes.
    pub async fn execute_stream<S, T>(self, stream: S) -> Result<JobInfo>
    where
        S: Stream<Item = T> + Unpin + Send,
        T: Serialize + Send + Sync,
    {
        if self.batch_size == 0 {
            return Err(crate::error::ForceError::InvalidInput(
                "Batch size must be greater than 0".to_string(),
            ));
        }

        // 1. Create Job
        let job_id = self.create_job_internal().await?;

        // 2. Process Stream
        if let Err(e) = self.process_stream(&job_id, stream).await {
            // Attempt to abort the job on failure to avoid leaving it Open
            let abort_req = UpdateJobRequest {
                state: JobState::Aborted,
            };
            // We ignore errors from the abort attempt to ensure the original error is returned
            let _ = self.handler.update_job(&job_id, abort_req).await;
            return Err(e);
        }

        // 3. Close Job
        self.close_job_internal(&job_id).await?;

        // 4. Poll for Completion
        self.poll_for_completion(&job_id).await
    }

    async fn create_job_internal(&self) -> Result<String> {
        let create_request = CreateJobRequest {
            object: self.object.clone(),
            operation: self.operation,
            content_type: None, // Defaults to CSV
            external_id_field_name: self.external_id_field.clone(),
            line_ending: None,
            column_delimiter: None,
        };

        let job_info = self.handler.create_job(create_request).await?;
        Ok(job_info.id)
    }

    async fn process_stream<S, T>(&self, job_id: &str, mut stream: S) -> Result<()>
    where
        S: Stream<Item = T> + Unpin + Send,
        T: Serialize + Send + Sync,
    {
        // Cap initial allocation to avoid panic/OOM on huge batch_size
        let capacity = std::cmp::min(self.batch_size, 10_000);
        let mut buffer = Vec::with_capacity(capacity);
        let mut is_first_batch = true;
        // Optimization: track the size of the previous batch's CSV data
        // to pre-allocate the next buffer and reduce reallocations.
        let mut capacity_hint = 0;

        while let Some(record) = stream.next().await {
            buffer.push(record);

            if buffer.len() >= self.batch_size {
                let size = self
                    .upload_batch(job_id, &buffer, is_first_batch, capacity_hint)
                    .await?;
                capacity_hint = size;
                buffer.clear();
                is_first_batch = false;
            }
        }

        if !buffer.is_empty() {
            self.upload_batch(job_id, &buffer, is_first_batch, capacity_hint)
                .await?;
        }
        Ok(())
    }

    async fn close_job_internal(&self, job_id: &str) -> Result<()> {
        let update_request = UpdateJobRequest {
            state: JobState::UploadComplete,
        };
        self.handler.update_job(job_id, update_request).await?;
        Ok(())
    }

    async fn poll_for_completion(&self, job_id: &str) -> Result<JobInfo> {
        let poll_policy = crate::api::bulk::BulkPollPolicy::default();
        let mut attempt = 0;

        loop {
            let job_info = self.handler.get_job(job_id).await?;

            match job_info.state {
                JobState::JobComplete => return Ok(job_info),
                JobState::Failed => {
                    return Err(crate::error::HttpError::StatusError {
                        status_code: 500,
                        message: format!(
                            "Job failed: {}",
                            job_info.error_message.unwrap_or_default()
                        ),
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

    async fn upload_batch<T>(
        &self,
        job_id: &str,
        records: &[T],
        is_first_batch: bool,
        capacity_hint: usize,
    ) -> Result<usize>
    where
        T: Serialize + Sync,
    {
        // Serialize to CSV
        // Use capacity hint to reduce reallocations
        let mut csv_data = Vec::with_capacity(capacity_hint);
        // Use the new helper with options
        csv::serialize_to_csv_with_options(records, &mut csv_data, is_first_batch)?;
        let size = csv_data.len();

        // Upload
        let base_url = self.handler.base_url().await?;
        let url = format!("{}/{}/batches", base_url, job_id);

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Content-Type",
            reqwest::header::HeaderValue::from_static("text/csv"),
        );

        let request = self
            .handler
            .inner
            .put(&url)
            .headers(headers)
            .body(csv_data)
            .build()
            .map_err(crate::error::HttpError::from)?;

        let response = self.handler.inner.execute_request(request).await?;

        if !response.status().is_success() {
            return Err(
                crate::http::response_to_force_error(response, "Batch upload failed").await,
            );
        }

        Ok(size)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::SmartIngest;
    use crate::api::bulk::types::{JobOperation, JobState};
    use crate::auth::{AccessToken, Authenticator, TokenResponse};
    use crate::client::{ForceClient, builder};
    use crate::error::Result;
    use crate::test_support::{Must, MustMsg};
    use async_trait::async_trait;
    use serde::Serialize;
    use wiremock::matchers::{body_string, header, method, path};
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

    async fn create_test_client(mock_server_url: String) -> ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", &mock_server_url);
        builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("failed to create test client")
    }

    #[derive(Serialize, Clone)]
    struct TestRecord {
        id: String,
        name: String,
    }

    #[tokio::test]
    async fn test_smart_ingest_single_batch() {
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

        // Mock: Upload Batch (Expecting Headers)
        Mock::given(method("PUT"))
            .and(path("/services/data/v60.0/jobs/ingest/JOB_ID/batches"))
            .and(header("content-type", "text/csv"))
            .and(body_string("id,name\n001,Test\n"))
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
                "createdById": "005xx0000000001AAA",
                "numberRecordsProcessed": 1,
                "numberRecordsFailed": 0
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let records = vec![TestRecord {
            id: "001".to_string(),
            name: "Test".to_string(),
        }];
        let stream = futures::stream::iter(records);

        let result = SmartIngest::new(&handler, "Account", JobOperation::Insert)
            .execute_stream(stream)
            .await;

        assert!(result.is_ok());
        let info = result.must();
        assert_eq!(info.id, "JOB_ID");
        assert_eq!(info.state, JobState::JobComplete);
        assert_eq!(info.number_records_processed, Some(1));
    }

    #[tokio::test]
    async fn test_smart_ingest_multi_batch() {
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

        // Mock: Upload Batch 1 (With Headers)
        Mock::given(method("PUT"))
            .and(path("/services/data/v60.0/jobs/ingest/JOB_ID/batches"))
            .and(body_string("id,name\n001,Test1\n"))
            .respond_with(ResponseTemplate::new(201))
            .expect(1)
            .mount(&mock_server)
            .await;

        // Mock: Upload Batch 2 (Without Headers)
        Mock::given(method("PUT"))
            .and(path("/services/data/v60.0/jobs/ingest/JOB_ID/batches"))
            .and(body_string("002,Test2\n"))
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
                name: "Test1".to_string(),
            },
            TestRecord {
                id: "002".to_string(),
                name: "Test2".to_string(),
            },
        ];
        let stream = futures::stream::iter(records);

        // Force batch size 1 to trigger multi-batch
        let result = SmartIngest::new(&handler, "Account", JobOperation::Insert)
            .batch_size(1)
            .execute_stream(stream)
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_smart_ingest_create_job_failure() {
        let mock_server = MockServer::start().await;

        // Mock: Create Job (Failure)
        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/ingest"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "message": "Bad Request",
                "errorCode": "INVALID_JOB"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let records = vec![TestRecord {
            id: "001".to_string(),
            name: "Test".to_string(),
        }];
        let stream = futures::stream::iter(records);

        let result = SmartIngest::new(&handler, "Account", JobOperation::Insert)
            .execute_stream(stream)
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_smart_ingest_upload_batch_failure_triggers_abort() {
        let mock_server = MockServer::start().await;

        // Mock: Create Job (Success)
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

        // Mock: Upload Batch (Failure)
        Mock::given(method("PUT"))
            .and(path("/services/data/v60.0/jobs/ingest/JOB_ID/batches"))
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&mock_server)
            .await;

        // Mock: Abort Job (Critical Check)
        Mock::given(method("PATCH"))
            .and(path("/services/data/v60.0/jobs/ingest/JOB_ID"))
            .and(body_string(r#"{"state":"Aborted"}"#))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "JOB_ID",
                "state": "Aborted"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let records = vec![TestRecord {
            id: "001".to_string(),
            name: "Test".to_string(),
        }];
        let stream = futures::stream::iter(records);

        let result = SmartIngest::new(&handler, "Account", JobOperation::Insert)
            .execute_stream(stream)
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_smart_ingest_close_job_failure() {
        let mock_server = MockServer::start().await;

        // Mock: Create Job (Success)
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

        // Mock: Upload Batch (Success)
        Mock::given(method("PUT"))
            .and(path("/services/data/v60.0/jobs/ingest/JOB_ID/batches"))
            .respond_with(ResponseTemplate::new(201))
            .expect(1)
            .mount(&mock_server)
            .await;

        // Mock: Close Job (Failure)
        Mock::given(method("PATCH"))
            .and(path("/services/data/v60.0/jobs/ingest/JOB_ID"))
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let records = vec![TestRecord {
            id: "001".to_string(),
            name: "Test".to_string(),
        }];
        let stream = futures::stream::iter(records);

        let result = SmartIngest::new(&handler, "Account", JobOperation::Insert)
            .execute_stream(stream)
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_smart_ingest_poll_failed_job() {
        let mock_server = MockServer::start().await;

        // Mock: Create Job (Success)
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

        // Mock: Upload Batch (Success)
        Mock::given(method("PUT"))
            .and(path("/services/data/v60.0/jobs/ingest/JOB_ID/batches"))
            .respond_with(ResponseTemplate::new(201))
            .expect(1)
            .mount(&mock_server)
            .await;

        // Mock: Close Job (Success)
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

        // Mock: Poll (Failed)
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/JOB_ID"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "JOB_ID",
                "state": "Failed",
                "errorMessage": "Something went wrong",
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

        let records = vec![TestRecord {
            id: "001".to_string(),
            name: "Test".to_string(),
        }];
        let stream = futures::stream::iter(records);

        let result = SmartIngest::new(&handler, "Account", JobOperation::Insert)
            .execute_stream(stream)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Job failed: Something went wrong"));
    }

    #[tokio::test]
    async fn test_smart_ingest_poll_aborted_job() {
        let mock_server = MockServer::start().await;

        // Mock: Create Job (Success)
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

        // Mock: Upload Batch (Success)
        Mock::given(method("PUT"))
            .and(path("/services/data/v60.0/jobs/ingest/JOB_ID/batches"))
            .respond_with(ResponseTemplate::new(201))
            .expect(1)
            .mount(&mock_server)
            .await;

        // Mock: Close Job (Success)
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

        // Mock: Poll (Aborted)
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/JOB_ID"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "JOB_ID",
                "state": "Aborted",
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

        let records = vec![TestRecord {
            id: "001".to_string(),
            name: "Test".to_string(),
        }];
        let stream = futures::stream::iter(records);

        let result = SmartIngest::new(&handler, "Account", JobOperation::Insert)
            .execute_stream(stream)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Job was aborted"));
    }

    #[tokio::test]
    async fn test_smart_ingest_empty_stream() {
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

        // Mock: Upload Batch (Should NOT be called for empty stream)
        Mock::given(method("PUT"))
            .and(path("/services/data/v60.0/jobs/ingest/JOB_ID/batches"))
            .respond_with(ResponseTemplate::new(200)) // Needed for type system even if expected 0
            .expect(0) // Should NOT be called
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

        let records: Vec<TestRecord> = vec![];
        let stream = futures::stream::iter(records);

        let result = SmartIngest::new(&handler, "Account", JobOperation::Insert)
            .execute_stream(stream)
            .await;

        assert!(result.is_ok());
    }
}
