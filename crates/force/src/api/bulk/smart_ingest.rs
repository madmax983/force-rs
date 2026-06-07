//! Smart Ingest - High-level streaming bulk ingest.
//!
//! This module provides the `SmartIngest` utility for uploading datasets to
//! Salesforce Bulk API 2.0 from async streams.

use super::BulkHandler;
use crate::api::bulk::csv;
use crate::api::bulk::types::{
    CreateJobRequest, JobInfo, JobOperation, JobState, UpdateJobRequest,
};
use crate::error::Result;
use futures::{Stream, StreamExt};
use serde::Serialize;

/// Default maximum CSV upload payload size per Bulk API 2.0 ingest job.
///
/// Salesforce accepts up to 150 MB after base64 conversion; keeping the raw CSV
/// payload at 100 MB leaves room for that expansion.
pub const DEFAULT_MAX_UPLOAD_BYTES: usize = 100_000_000;

/// Result of a `SmartIngest` operation.
#[derive(Debug, Clone)]
pub struct SmartIngestResult {
    /// Completed Bulk API 2.0 ingest jobs created for the stream.
    pub jobs: Vec<JobInfo>,
}

impl SmartIngestResult {
    /// Returns the number of Bulk API 2.0 jobs used.
    #[must_use]
    pub fn job_count(&self) -> usize {
        self.jobs.len()
    }

    /// Returns the total records processed across all jobs.
    #[must_use]
    pub fn total_records_processed(&self) -> i64 {
        self.jobs
            .iter()
            .map(|job| job.number_records_processed.unwrap_or(0))
            .sum()
    }

    /// Returns the total records failed across all jobs.
    #[must_use]
    pub fn total_records_failed(&self) -> i64 {
        self.jobs
            .iter()
            .map(|job| job.number_records_failed.unwrap_or(0))
            .sum()
    }

    /// Returns the total Salesforce-reported processing time across all jobs.
    #[must_use]
    pub fn total_processing_time(&self) -> i64 {
        self.jobs
            .iter()
            .map(|job| job.total_processing_time.unwrap_or(0))
            .sum()
    }
}

/// A high-level, streaming ingestion utility for Salesforce Bulk API 2.0.
///
/// `SmartIngest` orchestrates the lifecycle of one or more bulk ingest jobs:
/// 1. Creates a job as records arrive.
/// 2. Streams records from an async iterator.
/// 3. Buffers records while streaming them into byte-limited CSV payloads.
/// 4. Uploads the CSV once, as required by Bulk API 2.0 ingest jobs.
/// 5. Closes and polls each job.
/// 6. Starts another job when the next record would exceed the upload limit.
#[derive(Debug)]
pub struct SmartIngest<'a, A: crate::auth::Authenticator> {
    handler: &'a BulkHandler<A>,
    object: String,
    operation: JobOperation,
    external_id_field: Option<String>,
    batch_size: usize,
    max_upload_bytes: usize,
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
            max_upload_bytes: DEFAULT_MAX_UPLOAD_BYTES,
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

    /// Sets the maximum raw CSV payload size uploaded to one Bulk API 2.0 job.
    ///
    /// Defaults to 100 MB, which leaves room for Salesforce's base64 conversion
    /// before the documented 150 MB post-conversion limit.
    #[must_use]
    pub fn max_upload_bytes(mut self, size: usize) -> Self {
        self.max_upload_bytes = size;
        self
    }

    /// Executes ingest using the provided stream of records.
    ///
    /// This method consumes the stream, buffers records into CSV, and creates
    /// additional Bulk API 2.0 jobs whenever adding the next record would exceed
    /// `max_upload_bytes`.
    ///
    /// # Arguments
    ///
    /// * `stream` - An async stream of serializable records.
    ///
    /// # Returns
    ///
    /// A `SmartIngestResult` containing every completed job.
    pub async fn execute_stream<S, T>(self, stream: S) -> Result<SmartIngestResult>
    where
        S: Stream<Item = T> + Unpin + Send,
        T: Serialize + Send + Sync,
    {
        if self.batch_size == 0 {
            return Err(crate::error::ForceError::InvalidInput(
                "Batch size must be greater than 0".to_string(),
            ));
        }

        if self.max_upload_bytes == 0 {
            return Err(crate::error::ForceError::InvalidInput(
                "Max upload bytes must be greater than 0".to_string(),
            ));
        }

        self.process_stream(stream).await
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

    async fn process_stream<S, T>(&self, mut stream: S) -> Result<SmartIngestResult>
    where
        S: Stream<Item = T> + Unpin + Send,
        T: Serialize + Send + Sync,
    {
        // Cap initial allocation to avoid panic/OOM on huge batch_size
        let capacity = std::cmp::min(self.batch_size, 10_000);
        let mut buffer = Vec::with_capacity(capacity);
        let mut csv_data = Vec::new();
        let mut current_job_id = None;
        let mut jobs = Vec::new();

        while let Some(record) = stream.next().await {
            buffer.push(record);

            if buffer.len() >= self.batch_size {
                if let Err(error) = self
                    .append_records(&buffer, &mut current_job_id, &mut csv_data, &mut jobs)
                    .await
                {
                    if let Some(job_id) = current_job_id.as_deref() {
                        self.abort_job_best_effort(job_id).await;
                    }
                    return Err(error);
                }
                buffer.clear();
            }
        }

        if !buffer.is_empty() {
            if let Err(error) = self
                .append_records(&buffer, &mut current_job_id, &mut csv_data, &mut jobs)
                .await
            {
                if let Some(job_id) = current_job_id.as_deref() {
                    self.abort_job_best_effort(job_id).await;
                }
                return Err(error);
            }
        }

        if let Some(job_id) = current_job_id {
            jobs.push(self.finish_job(job_id, csv_data).await?);
        } else {
            let job_id = self.create_job_internal().await?;
            jobs.push(self.finish_job(job_id, Vec::new()).await?);
        }

        Ok(SmartIngestResult { jobs })
    }

    async fn append_records<T>(
        &self,
        records: &[T],
        current_job_id: &mut Option<String>,
        csv_data: &mut Vec<u8>,
        jobs: &mut Vec<JobInfo>,
    ) -> Result<()>
    where
        T: Serialize + Sync,
    {
        for record in records {
            let (header, row) = Self::serialize_record_parts(record)?;
            let minimum_payload_size = header.len() + row.len();

            if minimum_payload_size > self.max_upload_bytes {
                return Err(crate::error::ForceError::InvalidInput(format!(
                    "Serialized record is {minimum_payload_size} bytes with CSV headers, exceeding max_upload_bytes ({})",
                    self.max_upload_bytes
                )));
            }

            if current_job_id.is_none() {
                *current_job_id = Some(self.create_job_internal().await?);
                csv_data.extend_from_slice(&header);
            }

            if csv_data.len() + row.len() > self.max_upload_bytes {
                let Some(job_id) = current_job_id.take() else {
                    return Err(crate::error::ForceError::InvalidInput(
                        "Cannot finish SmartIngest job because no active job exists".to_string(),
                    ));
                };
                let payload = std::mem::take(csv_data);
                jobs.push(self.finish_job(job_id, payload).await?);

                *current_job_id = Some(self.create_job_internal().await?);
                csv_data.extend_from_slice(&header);
            }

            csv_data.extend_from_slice(&row);
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

            job_info.check_terminal("Job")?;

            if job_info.state == JobState::JobComplete {
                return Ok(job_info);
            }

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

    fn serialize_record_parts<T>(record: &T) -> Result<(Vec<u8>, Vec<u8>)>
    where
        T: Serialize + Sync,
    {
        let mut bytes = Vec::new();
        csv::serialize_to_csv_with_options(std::slice::from_ref(record), &mut bytes, true)?;

        let header_end = bytes
            .iter()
            .position(|byte| *byte == b'\n')
            .ok_or_else(|| {
                crate::error::ForceError::InvalidInput(
                    "Serialized CSV record did not include a header row".to_string(),
                )
            })?
            + 1;

        let row = bytes.split_off(header_end);
        Ok((bytes, row))
    }

    async fn finish_job(&self, job_id: String, csv_data: Vec<u8>) -> Result<JobInfo> {
        if !csv_data.is_empty()
            && let Err(error) = self.upload_csv(&job_id, csv_data).await
        {
            self.abort_job_best_effort(&job_id).await;
            return Err(error);
        }

        self.close_job_internal(&job_id).await?;
        self.poll_for_completion(&job_id).await
    }

    async fn abort_job_best_effort(&self, job_id: &str) {
        let abort_req = UpdateJobRequest {
            state: JobState::Aborted,
        };
        let _ = self.handler.update_job(job_id, abort_req).await;
    }

    async fn upload_csv(&self, job_id: &str, csv_data: Vec<u8>) -> Result<()> {
        let url = self
            .handler
            .inner
            .resolve_url(&format!("jobs/ingest/{}/batches", job_id))
            .await?;

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

        self.handler
            .inner
            .execute_and_check_success(request, "Batch upload failed")
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SmartIngest;
    use crate::api::bulk::types::{JobOperation, JobState};
    use crate::client::{ForceClient, builder};
    use crate::test_support::{MockAuthenticator, Must, MustMsg};
    use serde::Serialize;
    use wiremock::matchers::{body_string, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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

    async fn mount_completed_ingest_job(
        mock_server: &MockServer,
        job_id: &str,
        expected_csv: &str,
        records_processed: i64,
    ) {
        Mock::given(method("PUT"))
            .and(path(format!(
                "/services/data/v60.0/jobs/ingest/{job_id}/batches"
            )))
            .and(body_string(expected_csv.to_string()))
            .respond_with(ResponseTemplate::new(201))
            .expect(1)
            .mount(mock_server)
            .await;

        Mock::given(method("PATCH"))
            .and(path(format!("/services/data/v60.0/jobs/ingest/{job_id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": job_id,
                "state": "UploadComplete",
                "operation": "insert",
                "object": "Account",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .expect(1)
            .mount(mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path(format!("/services/data/v60.0/jobs/ingest/{job_id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": job_id,
                "state": "JobComplete",
                "operation": "insert",
                "object": "Account",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA",
                "numberRecordsProcessed": records_processed,
                "numberRecordsFailed": 0
            })))
            .expect(1)
            .mount(mock_server)
            .await;
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
        let result = result.must();
        let info = &result.jobs[0];
        assert_eq!(info.id, "JOB_ID");
        assert_eq!(info.state, JobState::JobComplete);
        assert_eq!(info.number_records_processed, Some(1));
        assert_eq!(result.job_count(), 1);
        assert_eq!(result.total_records_processed(), 1);
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

        // Bulk API 2.0 accepts exactly one CSV upload per ingest job. A tiny
        // batch size only controls local buffering; it must not create
        // multiple PUTs against the same job.
        Mock::given(method("PUT"))
            .and(path("/services/data/v60.0/jobs/ingest/JOB_ID/batches"))
            .and(body_string("id,name\n001,Test1\n002,Test2\n"))
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
    async fn test_smart_ingest_splits_across_jobs_when_upload_limit_is_reached() {
        let mock_server = MockServer::start().await;

        let create_job_call_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let create_job_call_count_for_mock = std::sync::Arc::clone(&create_job_call_count);

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/ingest"))
            .respond_with(move |_: &wiremock::Request| {
                let call_index = create_job_call_count_for_mock
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let id = match call_index {
                    0 => "JOB_ID_1",
                    1 => "JOB_ID_2",
                    _ => "UNEXPECTED_JOB_ID",
                };

                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "id": id,
                    "state": "Open",
                    "operation": "insert",
                    "object": "Account",
                    "createdDate": "2024-01-01T00:00:00.000Z",
                    "createdById": "005xx0000000001AAA"
                }))
            })
            .expect(2)
            .mount(&mock_server)
            .await;

        mount_completed_ingest_job(&mock_server, "JOB_ID_1", "id,name\n001,First\n", 1).await;
        mount_completed_ingest_job(&mock_server, "JOB_ID_2", "id,name\n002,Second\n", 1).await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let records = vec![
            TestRecord {
                id: "001".to_string(),
                name: "First".to_string(),
            },
            TestRecord {
                id: "002".to_string(),
                name: "Second".to_string(),
            },
        ];
        let stream = futures::stream::iter(records);

        let result = SmartIngest::new(&handler, "Account", JobOperation::Insert)
            .max_upload_bytes(20)
            .execute_stream(stream)
            .await
            .must();

        assert_eq!(result.jobs.len(), 2);
        assert_eq!(result.jobs[0].id, "JOB_ID_1");
        assert_eq!(result.jobs[1].id, "JOB_ID_2");
        assert_eq!(result.total_records_processed(), 2);
        assert_eq!(result.total_records_failed(), 0);
    }

    #[tokio::test]
    async fn test_smart_ingest_aborts_open_job_when_next_record_exceeds_upload_limit() {
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
            .expect(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("PATCH"))
            .and(path("/services/data/v60.0/jobs/ingest/JOB_ID"))
            .and(body_string(r#"{"state":"Aborted"}"#))
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

        let records = vec![
            TestRecord {
                id: "001".to_string(),
                name: "Ok".to_string(),
            },
            TestRecord {
                id: "002".to_string(),
                name: "This row is too large for the configured limit".to_string(),
            },
        ];
        let stream = futures::stream::iter(records);

        let result = SmartIngest::new(&handler, "Account", JobOperation::Insert)
            .max_upload_bytes(21)
            .execute_stream(stream)
            .await;

        let Err(crate::error::ForceError::InvalidInput(message)) = result else {
            panic!("Expected InvalidInput, got {:?}", result);
        };
        assert!(message.contains("exceeding max_upload_bytes"));
    }

    #[tokio::test]
    async fn test_smart_ingest_create_job_failure() {
        let mock_server = MockServer::start().await;

        // Mock: Create Job (Failure)
        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/ingest"))
            .respond_with(
                ResponseTemplate::new(400).set_body_json(serde_json::json!([{
                    "message": "Bad Request",
                    "errorCode": "INVALID_JOB"
                }])),
            )
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

        let Err(crate::error::ForceError::Http(crate::error::HttpError::StatusError {
            status_code,
            message,
        })) = result
        else {
            panic!("Expected StatusError, got {:?}", result);
        };
        assert_eq!(status_code, 400);
        assert!(
            message.contains("Bad Request"),
            "Actual message: {}",
            message
        );
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
            .respond_with(
                ResponseTemplate::new(500).set_body_json(serde_json::json!([{
                    "errorCode": "SERVER_ERROR",
                    "message": "Upload failed"
                }])),
            )
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

        let Err(crate::error::ForceError::Http(crate::error::HttpError::StatusError {
            status_code,
            message,
        })) = result
        else {
            panic!("Expected StatusError, got {:?}", result);
        };
        assert_eq!(status_code, 500);
        assert!(
            message.contains("Upload failed"),
            "Actual message: {}",
            message
        );
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
            .respond_with(
                ResponseTemplate::new(500).set_body_json(serde_json::json!([{
                    "errorCode": "SERVER_ERROR",
                    "message": "Close failed"
                }])),
            )
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

        let Err(crate::error::ForceError::Http(crate::error::HttpError::StatusError {
            status_code,
            message,
        })) = result
        else {
            panic!("Expected StatusError, got {:?}", result);
        };
        assert_eq!(status_code, 500);
        assert!(
            message.contains("Close failed"),
            "Actual message: {}",
            message
        );
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

        let Err(err) = result else {
            panic!("Expected Err")
        };
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

        let Err(err) = result else {
            panic!("Expected Err")
        };
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
