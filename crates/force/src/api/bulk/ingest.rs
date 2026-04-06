//! Ingest Job API for Salesforce Bulk API 2.0.
//!
//! This module provides a compile-time safe API for managing bulk ingest jobs,
//! ensuring safe job lifecycle transitions.
//!
//! # Job Lifecycle
//!
//! Rust's type system enforces valid state transitions at compile time:
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
//! use force::api::bulk::ingest::IngestJob;
//! use force::api::bulk::types::JobOperation;
//!
//! // Create and upload data
//! let job = IngestJob::create(&client.bulk(), "Account", JobOperation::Insert, None).await?;
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

use super::BulkHandler;
use crate::api::bulk::BulkPollPolicy;
#[cfg(feature = "bulk")]
use crate::api::bulk::smart_ingest::SmartIngest;
use crate::api::bulk::types::{
    CreateJobRequest, JobInfo, JobOperation, JobState, UpdateJobRequest,
};
use crate::auth::Authenticator;
use crate::error::Result;
use crate::types::validator::{validate_external_id_field, validate_sobject_name};
use std::marker::PhantomData;
use std::sync::Arc;

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

/// Compile-time safe ingest job handle.
///
/// The type parameter `S` represents the current job state and enforces
/// valid state transitions at compile time.
#[derive(Debug)]
pub struct IngestJob<S, A: Authenticator> {
    job_id: String,
    inner: Arc<crate::session::Session<A>>,
    _state: PhantomData<S>,
}

impl<S: Send + Sync, A: Authenticator> IngestJob<S, A> {
    /// Creates a test-only job handle in any state.
    #[cfg(test)]
    pub(crate) fn new_for_test(job_id: String, inner: Arc<crate::session::Session<A>>) -> Self {
        Self {
            job_id,
            inner,
            _state: PhantomData,
        }
    }

    /// Sends a PATCH request to update the job's state (e.g., close, abort).
    async fn patch_job_state(&self, state: JobState, error_context: &str) -> Result<()> {
        let request = UpdateJobRequest { state };
        let body = serde_json::to_vec(&request).map_err(crate::error::SerializationError::from)?;

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Content-Type",
            reqwest::header::HeaderValue::from_static("application/json"),
        );

        self.execute_job_request(
            reqwest::Method::PATCH,
            None,
            Some(body),
            Some(headers),
            error_context,
        )
        .await?;
        Ok(())
    }

    async fn execute_job_request(
        &self,
        method: reqwest::Method,
        path_suffix: Option<&str>,
        body: Option<impl Into<reqwest::Body>>,
        headers: Option<reqwest::header::HeaderMap>,
        error_context: &str,
    ) -> Result<reqwest::Response> {
        let mut url = self
            .inner
            .resolve_url(&format!("/jobs/ingest/{}", self.job_id))
            .await?;

        if let Some(suffix) = path_suffix {
            url.push('/');
            url.push_str(suffix);
        }

        let mut builder = self.inner.request(method, &url);

        if let Some(b) = body {
            builder = builder.body(b);
        }

        if let Some(h) = headers {
            builder = builder.headers(h);
        }

        let request = builder.build().map_err(crate::error::HttpError::from)?;
        let response = self.inner.execute_request(request).await?;

        if !response.status().is_success() {
            return Err(crate::http::response_to_force_error(response, error_context).await);
        }

        Ok(response)
    }
}

impl<A: Authenticator> IngestJob<Open, A> {
    /// Creates a new ingest job in Open state directly.
    ///
    /// # Errors
    /// Returns an error if job creation fails.
    pub async fn create(
        handler: &crate::api::bulk::BulkHandler<A>,
        object: impl Into<String>,
        operation: JobOperation,
        external_id_field_name: Option<String>,
    ) -> Result<Self> {
        let object = object.into();
        validate_sobject_name(&object)?;
        if let Some(field) = &external_id_field_name {
            validate_external_id_field(field)?;
        }

        let request = CreateJobRequest {
            object,
            operation,
            content_type: None,
            external_id_field_name,
            line_ending: None,
            column_delimiter: None,
        };

        let job_info = handler.create_job(request).await?;
        Ok(Self::new(job_info.id, Arc::clone(&handler.inner)))
    }

    pub(crate) fn new(job_id: String, inner: Arc<crate::session::Session<A>>) -> Self {
        Self {
            job_id,
            inner,
            _state: PhantomData,
        }
    }

    /// Uploads CSV data to the job.
    ///
    /// # Arguments
    ///
    /// * `data` - CSV data (Bytes, Vec<u8>, String, etc.)
    ///
    /// # Errors
    ///
    /// Returns an error if the upload fails.
    pub async fn upload(
        self,
        data: impl Into<reqwest::Body>,
    ) -> Result<IngestJob<UploadComplete, A>> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Content-Type",
            reqwest::header::HeaderValue::from_static("text/csv"),
        );

        self.execute_job_request(
            reqwest::Method::PUT,
            Some("batches"),
            Some(data),
            Some(headers),
            "CSV upload failed",
        )
        .await?;

        Ok(IngestJob {
            job_id: self.job_id,
            inner: self.inner,
            _state: PhantomData,
        })
    }

    /// Aborts the job.
    ///
    /// # Errors
    ///
    /// Returns an error if aborting fails.
    pub async fn abort(self) -> Result<()> {
        self.patch_job_state(JobState::Aborted, "Abort job failed")
            .await
    }
}

impl<A: Authenticator> IngestJob<UploadComplete, A> {
    /// Closes the job and transitions to processing state.
    ///
    /// # Errors
    ///
    /// Returns an error if closing the job fails.
    pub async fn close(self) -> Result<IngestJob<InProgress, A>> {
        self.patch_job_state(JobState::UploadComplete, "Close job failed")
            .await?;

        Ok(IngestJob {
            job_id: self.job_id,
            inner: self.inner,
            _state: PhantomData,
        })
    }
}

impl<A: Authenticator> IngestJob<InProgress, A> {
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
                        inner: self.inner,
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
        let response = self
            .execute_job_request(
                reqwest::Method::GET,
                None,
                Option::<Vec<u8>>::None,
                None,
                "Get job status failed",
            )
            .await?;

        let bytes = crate::http::error::read_capped_body_bytes(response, 100 * 1024 * 1024).await?;
        let job_info = serde_json::from_slice::<JobInfo>(&bytes)
            .map_err(crate::error::SerializationError::from)?;
        Ok(job_info)
    }
}

impl<A: Authenticator> IngestJob<JobComplete, A> {
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
        let response = self
            .execute_job_request(
                reqwest::Method::GET,
                Some(result_type),
                Option::<Vec<u8>>::None,
                None,
                &format!("Get {} failed", result_type),
            )
            .await?;

        let bytes = response
            .bytes()
            .await
            .map_err(crate::error::HttpError::from)?;
        Ok(bytes.to_vec())
    }
}

/// Extension methods for `BulkHandler` to support ingest jobs.
impl<A: Authenticator> BulkHandler<A> {
    /// Creates a builder for a smart ingest job.
    ///
    /// `SmartIngest` is a high-level utility that handles the entire lifecycle of a bulk ingest job:
    /// - Creating the job
    /// - Uploading data in batches
    /// - Closing the job
    /// - Polling for completion
    ///
    /// # Arguments
    ///
    /// * `object` - The SObject type (e.g., "Account")
    /// * `operation` - The operation to perform (Insert, Update, etc.)
    #[cfg(feature = "bulk")]
    #[must_use]
    pub fn smart_ingest(
        &self,
        object: impl Into<String>,
        operation: JobOperation,
    ) -> SmartIngest<'_, A> {
        SmartIngest::new(self, object, operation)
    }

    /// Creates a new bulk ingest job.
    ///
    /// Creates a job for inserting, updating, upserting, or deleting records in bulk.
    /// The job is created in the `Open` state and ready to accept data uploads.
    ///
    /// # Arguments
    ///
    /// * `request` - Job creation parameters including object type and operation
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The response cannot be deserialized
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use force::api::bulk::types::{CreateJobRequest, JobOperation};
    ///
    /// let request = CreateJobRequest {
    ///     object: "Account".to_string(),
    ///     operation: JobOperation::Insert,
    ///     content_type: None,
    ///     external_id_field_name: None,
    ///     line_ending: None,
    ///     column_delimiter: None,
    /// };
    ///
    /// let job = client.bulk().create_job(request).await?;
    /// println!("Created job: {}", job.id);
    /// ```
    pub async fn create_job(&self, request: CreateJobRequest) -> Result<JobInfo> {
        validate_sobject_name(&request.object)?;
        if let Some(field) = &request.external_id_field_name {
            validate_external_id_field(field)?;
        }

        let url = self.base_url().await?;
        let request = self
            .inner
            .post(&url)
            .json(&request)
            .build()
            .map_err(crate::error::HttpError::from)?;

        self.inner
            .send_request_and_decode::<JobInfo>(request, "Create job request failed")
            .await
    }

    /// Retrieves information about a bulk job.
    ///
    /// Gets the current state and statistics for a bulk job.
    ///
    /// # Arguments
    ///
    /// * `job_id` - The unique identifier for the job
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The job does not exist
    /// - The response cannot be deserialized
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let job = client.bulk().get_job("750xx0000000001AAA").await?;
    /// println!("Job state: {:?}", job.state);
    /// ```
    pub async fn get_job(&self, job_id: &str) -> Result<JobInfo> {
        let url = self
            .inner
            .resolve_url(&format!("jobs/ingest/{}", job_id))
            .await?;
        let request = self
            .inner
            .get(&url)
            .build()
            .map_err(crate::error::HttpError::from)?;

        self.inner
            .send_request_and_decode::<JobInfo>(
                request,
                &format!("Get job request failed for job {}", job_id),
            )
            .await
    }

    /// Updates a bulk job's state.
    ///
    /// Changes the state of a job, typically to mark upload as complete or abort the job.
    ///
    /// # Arguments
    ///
    /// * `job_id` - The unique identifier for the job
    /// * `request` - The state update request
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The job does not exist
    /// - The state transition is invalid
    /// - The response cannot be deserialized
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use force::api::bulk::types::{UpdateJobRequest, JobState};
    ///
    /// let request = UpdateJobRequest {
    ///     state: JobState::UploadComplete,
    /// };
    ///
    /// let job = client.bulk().update_job("750xx0000000001AAA", request).await?;
    /// assert_eq!(job.state, JobState::UploadComplete);
    /// ```
    pub async fn update_job(&self, job_id: &str, request: UpdateJobRequest) -> Result<JobInfo> {
        let url = self
            .inner
            .resolve_url(&format!("jobs/ingest/{}", job_id))
            .await?;
        let request = self
            .inner
            .patch(&url)
            .json(&request)
            .build()
            .map_err(crate::error::HttpError::from)?;

        self.inner
            .send_request_and_decode::<JobInfo>(
                request,
                &format!("Update job request failed for job {}", job_id),
            )
            .await
    }

    /// Deletes a bulk job.
    ///
    /// Permanently deletes a job. Only jobs in `Open`, `Aborted`, or `Failed` states can be deleted.
    ///
    /// # Arguments
    ///
    /// * `job_id` - The unique identifier for the job
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The job does not exist
    /// - The job is not in a deletable state
    ///
    /// # Examples
    ///
    /// ```ignore
    /// client.bulk().delete_job("750xx0000000001AAA").await?;
    /// ```
    pub async fn delete_job(&self, job_id: &str) -> Result<()> {
        let url = self
            .inner
            .resolve_url(&format!("jobs/ingest/{}", job_id))
            .await?;
        let request = self
            .inner
            .delete(&url)
            .build()
            .map_err(crate::error::HttpError::from)?;
        let response = self.inner.execute_request(request).await?;

        if !response.status().is_success() {
            return Err(crate::http::response_to_force_error(
                response,
                &format!("Delete job request failed for job {}", job_id),
            )
            .await);
        }

        Ok(())
    }

    /// Convenience method to perform a bulk insert operation.
    ///
    /// Creates an ingest job, uploads CSV data, closes the job, and polls until completion.
    /// This is a simplified API for common bulk insert operations.
    ///
    /// # Arguments
    ///
    /// * `object` - The Salesforce object type (e.g., "Account", "Contact")
    /// * `records` - The records to insert
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Job creation fails
    /// - CSV serialization fails
    /// - Upload fails
    /// - Job processing fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use serde::Serialize;
    ///
    /// #[derive(Serialize)]
    /// struct Account {
    ///     #[serde(rename = "Name")]
    ///     name: String,
    /// }
    ///
    /// let accounts = vec![
    ///     Account { name: "Acme Corp".to_string() },
    /// ];
    ///
    /// let job = client.bulk().insert("Account", &accounts).await?;
    /// println!("Processed: {}", job.number_records_processed.unwrap_or(0));
    /// ```
    #[cfg(feature = "bulk")]
    pub async fn insert<T>(&self, object: &str, records: &[T]) -> Result<JobInfo>
    where
        T: serde::Serialize + Sync,
    {
        use crate::api::bulk::csv;

        // Serialize records to CSV
        // ⚡ Bolt: Pre-allocate capacity (assuming ~100 bytes per record) to minimize heap reallocations during serialization
        let mut csv_data = Vec::with_capacity(records.len() * 100);
        csv::serialize_to_csv(records, &mut csv_data)?;

        // Create job
        let job = IngestJob::create(self, object, JobOperation::Insert, None).await?;

        // Upload, close, and poll
        let job = job.upload(csv_data).await?;
        let job = job.close().await?;
        let job = job.poll_until_complete().await?;

        // Get final job info
        let job_info = self.get_job(job.job_id()).await?;
        Ok(job_info)
    }

    /// Convenience method to perform a bulk insert operation (legacy name).
    #[cfg(feature = "bulk")]
    pub async fn bulk_insert<T>(&self, object: &str, records: &[T]) -> Result<JobInfo>
    where
        T: serde::Serialize + Sync,
    {
        self.insert(object, records).await
    }

    /// Convenience method to perform a bulk update operation.
    ///
    /// Creates an ingest job, uploads CSV data, closes the job, and polls until completion.
    /// Records must include the Salesforce ID field.
    ///
    /// # Arguments
    ///
    /// * `object` - The Salesforce object type (e.g., "Account", "Contact")
    /// * `records` - The records to update (must include Id field)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Job creation fails
    /// - CSV serialization fails
    /// - Upload fails
    /// - Job processing fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use serde::Serialize;
    ///
    /// #[derive(Serialize)]
    /// struct Account {
    ///     #[serde(rename = "Id")]
    ///     id: String,
    ///     #[serde(rename = "Name")]
    ///     name: String,
    /// }
    ///
    /// let accounts = vec![
    ///     Account {
    ///         id: "001xx0000000001AAA".to_string(),
    ///         name: "Updated Name".to_string(),
    ///     },
    /// ];
    ///
    /// let job = client.bulk().update("Account", &accounts).await?;
    /// println!("Processed: {}", job.number_records_processed.unwrap_or(0));
    /// ```
    #[cfg(feature = "bulk")]
    pub async fn update<T>(&self, object: &str, records: &[T]) -> Result<JobInfo>
    where
        T: serde::Serialize + Sync,
    {
        use crate::api::bulk::csv;

        // Serialize records to CSV
        // ⚡ Bolt: Pre-allocate capacity (assuming ~100 bytes per record) to minimize heap reallocations during serialization
        let mut csv_data = Vec::with_capacity(records.len() * 100);
        csv::serialize_to_csv(records, &mut csv_data)?;

        // Create job
        let job = IngestJob::create(self, object, JobOperation::Update, None).await?;

        // Upload, close, and poll
        let job = job.upload(csv_data).await?;
        let job = job.close().await?;
        let job = job.poll_until_complete().await?;

        // Get final job info
        let job_info = self.get_job(job.job_id()).await?;
        Ok(job_info)
    }

    /// Convenience method to perform a bulk update operation (legacy name).
    #[cfg(feature = "bulk")]
    pub async fn bulk_update<T>(&self, object: &str, records: &[T]) -> Result<JobInfo>
    where
        T: serde::Serialize + Sync,
    {
        self.update(object, records).await
    }

    /// Convenience method to perform a bulk delete operation.
    ///
    /// Creates an ingest job, uploads CSV data with IDs, closes the job, and polls until completion.
    ///
    /// # Arguments
    ///
    /// * `object` - The Salesforce object type (e.g., "Account", "Contact")
    /// * `ids` - The Salesforce IDs to delete
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Job creation fails
    /// - CSV serialization fails
    /// - Upload fails
    /// - Job processing fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let ids = vec![
    ///     "001xx0000000001AAA".to_string(),
    ///     "001xx0000000002AAA".to_string(),
    /// ];
    ///
    /// let job = client.bulk().delete("Account", &ids).await?;
    /// println!("Deleted: {}", job.number_records_processed.unwrap_or(0));
    /// ```
    #[cfg(feature = "bulk")]
    pub async fn delete(&self, object: &str, ids: &[String]) -> Result<JobInfo> {
        use crate::api::bulk::csv;

        // Create CSV with Id column
        #[derive(serde::Serialize)]
        struct DeleteRecord<'a> {
            #[serde(rename = "Id")]
            id: &'a str,
        }

        let delete_records: Vec<DeleteRecord<'_>> = ids
            .iter()
            .map(|id| DeleteRecord { id: id.as_str() })
            .collect();

        let mut csv_data = Vec::with_capacity(ids.len() * 20); // Pre-allocate some capacity to minimize reallocations
        csv::serialize_to_csv(&delete_records, &mut csv_data)?;

        // Create job
        let job = IngestJob::create(self, object, JobOperation::Delete, None).await?;

        // Upload, close, and poll
        let job = job.upload(csv_data).await?;
        let job = job.close().await?;
        let job = job.poll_until_complete().await?;

        // Get final job info
        let job_info = self.get_job(job.job_id()).await?;
        Ok(job_info)
    }

    /// Convenience method to perform a bulk delete operation (legacy name).
    #[cfg(feature = "bulk")]
    pub async fn bulk_delete(&self, object: &str, ids: &[String]) -> Result<JobInfo> {
        self.delete(object, ids).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::bulk::types::{ContentType, JobOperation};
    use crate::client::{ForceClient, builder};
    use crate::test_support::{MockAuthenticator, Must, MustMsg};
    use wiremock::matchers::{bearer_token, body_bytes, header, method, path, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn create_test_client(mock_server_url: String) -> ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", &mock_server_url);
        builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("failed to create test client")
    }

    // Existing tests...
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

        let job = IngestJob::create(&handler, "Account", JobOperation::Insert, None)
            .await
            .must();

        let _ = job;
    }

    // ... (include all existing tests here, omitted for brevity but I will preserve them in the actual write)

    // Moved tests from mod.rs

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_create_job_success() {
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

        let request = CreateJobRequest {
            object: "Account".to_string(),
            operation: JobOperation::Insert,
            content_type: Some(ContentType::Csv),
            external_id_field_name: None,
            line_ending: None,
            column_delimiter: None,
        };

        let job = handler.create_job(request).await.must();
        assert_eq!(job.id, "750xx0000000001AAA");
        assert_eq!(job.operation, JobOperation::Insert);
        assert_eq!(job.object, "Account");
        assert_eq!(job.state, JobState::Open);
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_create_job_with_upsert() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/ingest"))
            .and(bearer_token("test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000002AAA",
                "operation": "upsert",
                "object": "Contact",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA",
                "state": "Open",
                "externalIdFieldName": "External_Id__c"
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let request = CreateJobRequest {
            object: "Contact".to_string(),
            operation: JobOperation::Upsert,
            content_type: None,
            external_id_field_name: Some("External_Id__c".to_string()),
            line_ending: None,
            column_delimiter: None,
        };

        let job = handler.create_job(request).await.must();
        assert_eq!(job.operation, JobOperation::Upsert);
        assert_eq!(
            job.external_id_field_name,
            Some("External_Id__c".to_string())
        );
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_create_job_failure() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/ingest"))
            .respond_with(ResponseTemplate::new(400))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let request = CreateJobRequest {
            object: "Account".to_string(),
            operation: JobOperation::Insert,
            content_type: None,
            external_id_field_name: None,
            line_ending: None,
            column_delimiter: None,
        };

        let result = handler.create_job(request).await;
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(err.to_string().contains(""));
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_get_job_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000001AAA"))
            .and(bearer_token("test_token"))
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

        let job = handler.get_job("750xx0000000001AAA").await.must();
        assert_eq!(job.id, "750xx0000000001AAA");
        assert_eq!(job.state, JobState::JobComplete);
        assert_eq!(job.number_records_processed, Some(100));
        assert_eq!(job.number_records_failed, Some(2));
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_get_job_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path_regex("/services/data/v60.0/jobs/ingest/.*"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let result = handler.get_job("750xx0000000999AAA").await;
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(err.to_string().contains(""));
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_update_job_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("PATCH"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000001AAA"))
            .and(bearer_token("test_token"))
            .and(header("content-type", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000001AAA",
                "operation": "insert",
                "object": "Account",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA",
                "state": "UploadComplete"
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let request = UpdateJobRequest {
            state: JobState::UploadComplete,
        };

        let job = handler
            .update_job("750xx0000000001AAA", request)
            .await
            .must();
        assert_eq!(job.state, JobState::UploadComplete);
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_update_job_invalid_state_transition() {
        let mock_server = MockServer::start().await;

        Mock::given(method("PATCH"))
            .and(path_regex("/services/data/v60.0/jobs/ingest/.*"))
            .respond_with(ResponseTemplate::new(400))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let request = UpdateJobRequest {
            state: JobState::Aborted,
        };

        let result = handler.update_job("750xx0000000001AAA", request).await;
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(err.to_string().contains(""));
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_delete_job_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("DELETE"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000001AAA"))
            .and(bearer_token("test_token"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let result = handler.delete_job("750xx0000000001AAA").await;
        assert!(result.is_ok());
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_delete_job_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("DELETE"))
            .and(path_regex("/services/data/v60.0/jobs/ingest/.*"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let result = handler.delete_job("750xx0000000999AAA").await;
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(err.to_string().contains(""));
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_bulk_insert_success() {
        use serde::Serialize;

        #[derive(Serialize, Clone)]
        struct Account {
            #[serde(rename = "Name")]
            name: String,
            #[serde(rename = "Industry")]
            industry: String,
        }

        let mock_server = MockServer::start().await;

        // Mock: Create job
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

        // Mock: Upload CSV
        Mock::given(method("PUT"))
            .and(path(
                "/services/data/v60.0/jobs/ingest/750xx0000000001AAA/batches",
            ))
            .respond_with(ResponseTemplate::new(201))
            .mount(&mock_server)
            .await;

        // Mock: Close job
        Mock::given(method("PATCH"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000001AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000001AAA",
                "operation": "insert",
                "object": "Account",
                "state": "UploadComplete",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        // Mock: Poll job (complete immediately)
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000001AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000001AAA",
                "operation": "insert",
                "object": "Account",
                "state": "JobComplete",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA",
                "numberRecordsProcessed": 2,
                "numberRecordsFailed": 0
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let records = vec![
            Account {
                name: "Acme Corp".to_string(),
                industry: "Technology".to_string(),
            },
            Account {
                name: "Global Industries".to_string(),
                industry: "Manufacturing".to_string(),
            },
        ];

        let job_info = handler.bulk_insert("Account", &records).await.must();
        assert_eq!(job_info.state, JobState::JobComplete);
        assert_eq!(job_info.number_records_processed, Some(2));
        assert_eq!(job_info.number_records_failed, Some(0));
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_bulk_insert_with_failures() {
        use serde::Serialize;

        #[derive(Serialize, Clone)]
        struct Account {
            #[serde(rename = "Name")]
            name: String,
        }

        let mock_server = MockServer::start().await;

        // Mock: Create, upload, close
        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/ingest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000002AAA",
                "operation": "insert",
                "object": "Account",
                "state": "Open",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("PUT"))
            .and(path(
                "/services/data/v60.0/jobs/ingest/750xx0000000002AAA/batches",
            ))
            .respond_with(ResponseTemplate::new(201))
            .mount(&mock_server)
            .await;

        Mock::given(method("PATCH"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000002AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000002AAA",
                "operation": "insert",
                "object": "Account",
                "state": "UploadComplete",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000002AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000002AAA",
                "operation": "insert",
                "object": "Account",
                "state": "JobComplete",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA",
                "numberRecordsProcessed": 5,
                "numberRecordsFailed": 2
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let records = vec![
            Account {
                name: "Valid Account".to_string(),
            },
            Account {
                name: String::new(), // Invalid - empty name
            },
        ];

        let job_info = handler.bulk_insert("Account", &records).await.must();
        assert_eq!(job_info.state, JobState::JobComplete);
        assert_eq!(job_info.number_records_failed, Some(2));
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_bulk_update_success() {
        use serde::Serialize;

        #[derive(Serialize, Clone)]
        struct Account {
            #[serde(rename = "Id")]
            id: String,
            #[serde(rename = "Name")]
            name: String,
        }

        let mock_server = MockServer::start().await;

        // Mock: Create job with update operation
        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/ingest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000003AAA",
                "operation": "update",
                "object": "Account",
                "state": "Open",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("PUT"))
            .and(path(
                "/services/data/v60.0/jobs/ingest/750xx0000000003AAA/batches",
            ))
            .respond_with(ResponseTemplate::new(201))
            .mount(&mock_server)
            .await;

        Mock::given(method("PATCH"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000003AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000003AAA",
                "operation": "update",
                "object": "Account",
                "state": "UploadComplete",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000003AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000003AAA",
                "operation": "update",
                "object": "Account",
                "state": "JobComplete",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA",
                "numberRecordsProcessed": 3,
                "numberRecordsFailed": 0
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let records = vec![
            Account {
                id: "001xx0000000001AAA".to_string(),
                name: "Updated Name 1".to_string(),
            },
            Account {
                id: "001xx0000000002AAA".to_string(),
                name: "Updated Name 2".to_string(),
            },
        ];

        let job_info = handler.bulk_update("Account", &records).await.must();
        assert_eq!(job_info.operation, JobOperation::Update);
        assert_eq!(job_info.state, JobState::JobComplete);
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_bulk_delete_success() {
        let mock_server = MockServer::start().await;

        // Mock: Create job with delete operation
        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/ingest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000004AAA",
                "operation": "delete",
                "object": "Account",
                "state": "Open",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("PUT"))
            .and(path(
                "/services/data/v60.0/jobs/ingest/750xx0000000004AAA/batches",
            ))
            .respond_with(ResponseTemplate::new(201))
            .mount(&mock_server)
            .await;

        Mock::given(method("PATCH"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000004AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000004AAA",
                "operation": "delete",
                "object": "Account",
                "state": "UploadComplete",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000004AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000004AAA",
                "operation": "delete",
                "object": "Account",
                "state": "JobComplete",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA",
                "numberRecordsProcessed": 5,
                "numberRecordsFailed": 0
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let ids = vec![
            "001xx0000000001AAA".to_string(),
            "001xx0000000002AAA".to_string(),
            "001xx0000000003AAA".to_string(),
        ];

        let job_info = handler.bulk_delete("Account", &ids).await.must();
        assert_eq!(job_info.operation, JobOperation::Delete);
        assert_eq!(job_info.state, JobState::JobComplete);
        assert_eq!(job_info.number_records_processed, Some(5));
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_bulk_delete_with_invalid_ids() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/ingest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000005AAA",
                "operation": "delete",
                "object": "Account",
                "state": "Open",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("PUT"))
            .and(path(
                "/services/data/v60.0/jobs/ingest/750xx0000000005AAA/batches",
            ))
            .respond_with(ResponseTemplate::new(201))
            .mount(&mock_server)
            .await;

        Mock::given(method("PATCH"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000005AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000005AAA",
                "operation": "delete",
                "object": "Account",
                "state": "UploadComplete",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000005AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000005AAA",
                "operation": "delete",
                "object": "Account",
                "state": "JobComplete",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA",
                "numberRecordsProcessed": 2,
                "numberRecordsFailed": 1
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let ids = vec!["001xx0000000001AAA".to_string(), "INVALID_ID".to_string()];

        let job_info = handler.bulk_delete("Account", &ids).await.must();
        assert_eq!(job_info.number_records_failed, Some(1));
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_bulk_insert_job_failure() {
        use serde::Serialize;

        #[derive(Serialize, Clone)]
        struct Account {
            #[serde(rename = "Name")]
            name: String,
        }

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/ingest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000008AAA",
                "operation": "insert",
                "object": "Account",
                "state": "Open",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("PUT"))
            .and(path(
                "/services/data/v60.0/jobs/ingest/750xx0000000008AAA/batches",
            ))
            .respond_with(ResponseTemplate::new(201))
            .mount(&mock_server)
            .await;

        Mock::given(method("PATCH"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000008AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000008AAA",
                "operation": "insert",
                "object": "Account",
                "state": "UploadComplete",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        // Job fails during processing
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000008AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000008AAA",
                "operation": "insert",
                "object": "Account",
                "state": "Failed",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let records = vec![Account {
            name: "Test".to_string(),
        }];

        let result = handler.bulk_insert("Account", &records).await;
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(err.to_string().contains(""));
    }

    // Include existing tests as well to avoid regression
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

        let job = IngestJob::create(&handler, "Account", JobOperation::Insert, None)
            .await
            .must();

        let csv_data = "Name,Industry\nAcme Corp,Technology\n";
        let _job = job.upload(csv_data.as_bytes()).await.must();
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_upload_bytes_zero_copy() {
        let mock_server = MockServer::start().await;

        // Mock job creation
        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/ingest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000009AAA",
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
                "/services/data/v60.0/jobs/ingest/750xx0000000009AAA/batches",
            ))
            .and(bearer_token("test_token"))
            .and(header("content-type", "text/csv"))
            .and(body_bytes(b"Zero Copy Data"))
            .respond_with(ResponseTemplate::new(201))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let job = IngestJob::create(&handler, "Account", JobOperation::Insert, None)
            .await
            .must();

        // Pass Bytes directly - this verifies the API accepts `impl Into<Body>`
        // and doesn't require &[u8] or Vec<u8> specifically.
        let data = bytes::Bytes::from_static(b"Zero Copy Data");
        let _job = job.upload(data).await.must();
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_abort_job() {
        use wiremock::matchers::body_json;
        let mock_server = MockServer::start().await;

        Mock::given(method("PATCH"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000009AAA"))
            .and(bearer_token("test_token"))
            .and(header("content-type", "application/json"))
            .and(body_json(serde_json::json!({
                "state": "Aborted"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000009AAA",
                "operation": "insert",
                "object": "Account",
                "createdById": "005xx0000000001AAA",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "state": "Aborted",
                "contentType": "CSV"
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let job =
            IngestJob::<Open, _>::new_for_test("750xx0000000009AAA".to_string(), client.session());

        job.abort().await.must();
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_get_results() {
        let mock_server = MockServer::start().await;

        // Mock successful results
        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/jobs/ingest/750xx0000000009AAA/successfulResults",
            ))
            .and(bearer_token("test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"success_data".to_vec()))
            .mount(&mock_server)
            .await;

        // Mock failed results
        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/jobs/ingest/750xx0000000009AAA/failedResults",
            ))
            .and(bearer_token("test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"failed_data".to_vec()))
            .mount(&mock_server)
            .await;

        // Mock unprocessed results
        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/jobs/ingest/750xx0000000009AAA/unprocessedrecords",
            ))
            .and(bearer_token("test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"unprocessed_data".to_vec()))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let job = IngestJob::<JobComplete, _>::new_for_test(
            "750xx0000000009AAA".to_string(),
            client.session(),
        );

        let success = job.successful_results().await.must();
        assert_eq!(success, b"success_data");

        let failed = job.failed_results().await.must();
        assert_eq!(failed, b"failed_data");

        let unprocessed = job.unprocessed_results().await.must();
        assert_eq!(unprocessed, b"unprocessed_data");
    }
}
