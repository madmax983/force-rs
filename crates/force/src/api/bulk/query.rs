//! Bulk Query API for Salesforce Bulk API 2.0.
//!
//! This module provides functionality for executing bulk queries against Salesforce,
//! allowing efficient retrieval of large datasets via CSV streaming.
//!
//! # Query Lifecycle
//!
//! 1. Create a query job with SOQL
//! 2. Poll job status until complete
//! 3. Stream CSV results
//! 4. Process results as typed records or DynamicSObject
//!
//! # Examples
//!
//! ```ignore
//! use force::api::bulk::query::{BulkQueryRequest, BulkQueryStream};
//!
//! // Create query job
//! let request = BulkQueryRequest::new("SELECT Id, Name FROM Account");
//! let job = client.bulk().create_query_job(request).await?;
//!
//! // Wait for completion
//! let completed = client.bulk().wait_for_query_job(&job.id).await?;
//!
//! // Stream results
//! let mut stream = client.bulk().query_results::<Account>(&job.id).await?;
//! while let Some(record) = stream.next().await? {
//!     println!("{:?}", record);
//! }
//! ```

use crate::error::Result;
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio_util::compat::TokioAsyncReadCompatExt;
use tokio_util::io::StreamReader;

/// Request to create a bulk query job.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkQueryRequest {
    /// The SOQL query to execute.
    pub query: String,
    /// Operation type (always "query" for bulk queries).
    pub operation: String,
}

impl BulkQueryRequest {
    /// Creates a new bulk query request.
    ///
    /// # Arguments
    ///
    /// * `query` - The SOQL query string
    ///
    /// # Examples
    ///
    /// ```
    /// use force::api::bulk::query::BulkQueryRequest;
    ///
    /// let request = BulkQueryRequest::new("SELECT Id, Name FROM Account WHERE CreatedDate > TODAY");
    /// ```
    #[must_use]
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            operation: "query".to_string(),
        }
    }
}

/// Response from creating a bulk query job.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkQueryJobInfo {
    /// Unique job ID.
    pub id: String,
    /// Operation type (query).
    pub operation: String,
    /// Job state.
    pub state: super::types::JobState,
    /// Job creation timestamp.
    pub created_date: String,
    /// User who created the job.
    pub created_by_id: String,
    /// System modstamp timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_modstamp: Option<String>,
    /// Number of records processed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_records_processed: Option<i64>,
    /// Total processing time in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_processing_time: Option<i64>,
    /// API version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_version: Option<String>,
}

/// Stream for iterating over bulk query results.
///
/// This stream lazily fetches and deserializes CSV results from a completed
/// bulk query job. It's memory-efficient and suitable for processing large
/// result sets (100MB+).
pub struct BulkQueryStream<T, A: crate::auth::Authenticator> {
    /// Reference to the client's inner state.
    inner: Arc<crate::client::Inner<A>>,
    /// Job ID for the query.
    job_id: String,
    /// CSV Async Reader for the current stream
    reader: Option<csv_async::AsyncReader<Box<dyn futures::AsyncRead + Send + Unpin + Sync>>>,
    /// Result locator for the next page (`None` means first page).
    next_locator: Option<String>,
    /// Whether the first page has been fetched.
    first_page_fetched: bool,
    /// Whether all records have been fetched.
    exhausted: bool,
    /// Phantom data for T
    _phantom: std::marker::PhantomData<T>,
}

impl<T, A: crate::auth::Authenticator> BulkQueryStream<T, A> {
    /// Creates a new bulk query stream.
    #[must_use]
    pub(crate) fn new(inner: Arc<crate::client::Inner<A>>, job_id: String) -> Self {
        Self {
            inner,
            job_id,
            reader: None,
            next_locator: None,
            first_page_fetched: false,
            exhausted: false,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Creates a new bulk query stream (async version for convenience methods).
    ///
    /// # Errors
    ///
    /// This version always succeeds; errors occur during streaming.
    #[allow(clippy::unused_async)]
    pub(crate) async fn new_async(
        inner: Arc<crate::client::Inner<A>>,
        job_id: &str,
    ) -> Result<Self> {
        Ok(Self::new(inner, job_id.to_string()))
    }

    /// Fetches the next record from the stream.
    ///
    /// Returns `None` when all records have been consumed.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - CSV deserialization fails
    pub async fn next(&mut self) -> Result<Option<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        loop {
            // If we've already marked as exhausted, return None
            if self.exhausted {
                return Ok(None);
            }

            // If we have a reader, try to read the next record
            if let Some(reader) = &mut self.reader {
                let mut byte_record = csv_async::ByteRecord::new();
                match reader.read_byte_record(&mut byte_record).await {
                    Ok(true) => {
                        let record: T = byte_record.deserialize(None).map_err(|e| {
                            crate::error::HttpError::StatusError {
                                status_code: 500,
                                message: format!("CSV deserialization failed: {}", e),
                            }
                        })?;
                        return Ok(Some(record));
                    }
                    Ok(false) => {
                        // End of current stream, drop reader and continue to fetch next page
                        self.reader = None;
                        // Continue loop to fetch next page
                    }
                    Err(e) => {
                        return Err(crate::error::HttpError::StatusError {
                            status_code: 500,
                            message: format!("CSV reading failed: {}", e),
                        }
                        .into());
                    }
                }
            }

            // Check if we need to fetch more pages
            if self.first_page_fetched && self.next_locator.is_none() {
                self.exhausted = true;
                return Ok(None);
            }

            // Fetch results from the API
            let token = self.inner.token_manager.token().await?;
            let base_url = format!(
                "{}/services/data/{}/jobs/query/{}/results",
                token.instance_url(),
                self.inner.config.api_version,
                self.job_id
            );
            let mut request_builder = self.inner.http_client.get(&base_url);
            if let Some(locator) = &self.next_locator {
                request_builder = request_builder.query(&[("locator", locator)]);
            }

            let response = request_builder
                .build()
                .map_err(crate::error::HttpError::from)?;
            let response = self.inner.execute_request(response).await?;

            if !response.status().is_success() {
                return Err(handle_error_response(
                    response,
                    &format!("Failed to fetch query results for job {}", self.job_id),
                )
                .await);
            }

            // Get locator for next page
            let locator_header = response
                .headers()
                .get("Sforce-Locator")
                .and_then(|value| value.to_str().ok())
                .map(std::string::ToString::to_string);
            self.first_page_fetched = true;
            self.next_locator = match locator_header.as_deref() {
                Some("null") | None => None,
                Some(value) => Some(value.to_string()),
            };

            // Stream CSV
            let stream = response
                .bytes_stream()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));

            let stream_reader = StreamReader::new(stream);
            let async_read = stream_reader.compat();

            // Box it to erase types
            let boxed_reader: Box<dyn futures::AsyncRead + Send + Unpin + Sync> = Box::new(async_read);

            let reader = csv_async::AsyncReader::from_reader(boxed_reader);
            self.reader = Some(reader);

            // Continue loop to read from the new reader
        }
    }
}

/// Helper function to handle error responses from bulk query API.
async fn handle_error_response(
    response: reqwest::Response,
    context: &str,
) -> crate::error::ForceError {
    crate::http::response_to_force_error(response, context).await
}

/// Extension methods for `BulkHandler` to support bulk queries.
impl<A: crate::auth::Authenticator> super::BulkHandler<A> {
    /// Constructs the base URL for Bulk Query API operations.
    ///
    /// The base URL is constructed as: `{instance_url}/services/data/{api_version}/jobs/query`
    ///
    /// # Errors
    ///
    /// Returns an error if token retrieval fails.
    pub async fn query_base_url(&self) -> Result<String> {
        let inner = self.inner();
        let token = inner.token_manager.token().await?;
        Ok(format!(
            "{}/services/data/{}/jobs/query",
            token.instance_url(),
            inner.config.api_version
        ))
    }

    /// Creates a new bulk query job.
    ///
    /// Submits a SOQL query for bulk execution. The job will be processed
    /// asynchronously and results can be retrieved once the job completes.
    ///
    /// # Arguments
    ///
    /// * `request` - The bulk query request with SOQL
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The SOQL query is invalid
    /// - The response cannot be deserialized
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use force::api::bulk::query::BulkQueryRequest;
    ///
    /// let request = BulkQueryRequest::new("SELECT Id, Name FROM Account");
    /// let job = client.bulk().create_query_job(request).await?;
    /// ```
    pub async fn create_query_job(&self, request: BulkQueryRequest) -> Result<BulkQueryJobInfo> {
        let url = self.query_base_url().await?;
        let inner = self.inner();
        let request = inner
            .http_client
            .post(&url)
            .json(&request)
            .build()
            .map_err(crate::error::HttpError::from)?;
        let response = inner.execute_request(request).await?;

        if !response.status().is_success() {
            return Err(handle_error_response(response, "Create query job request failed").await);
        }

        let job_info = response
            .json::<BulkQueryJobInfo>()
            .await
            .map_err(crate::error::HttpError::from)?;
        Ok(job_info)
    }

    /// Retrieves information about a bulk query job.
    ///
    /// Gets the current state and statistics for a bulk query job.
    ///
    /// # Arguments
    ///
    /// * `job_id` - The unique identifier for the query job
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
    /// let job = client.bulk().get_query_job("750xx0000000001AAA").await?;
    /// println!("Job state: {:?}", job.state);
    /// ```
    pub async fn get_query_job(&self, job_id: &str) -> Result<BulkQueryJobInfo> {
        let url = format!("{}/{}", self.query_base_url().await?, job_id);
        let inner = self.inner();
        let request = inner
            .http_client
            .get(&url)
            .build()
            .map_err(crate::error::HttpError::from)?;
        let response = inner.execute_request(request).await?;

        if !response.status().is_success() {
            return Err(handle_error_response(
                response,
                &format!("Get query job request failed for job {}", job_id),
            )
            .await);
        }

        let job_info = response
            .json::<BulkQueryJobInfo>()
            .await
            .map_err(crate::error::HttpError::from)?;
        Ok(job_info)
    }

    /// Aborts a running bulk query job.
    ///
    /// Cancels a query job that is in progress. Once aborted, the job cannot
    /// be resumed and results cannot be retrieved.
    ///
    /// # Arguments
    ///
    /// * `job_id` - The unique identifier for the query job
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The job does not exist
    /// - The job is already complete
    ///
    /// # Examples
    ///
    /// ```ignore
    /// client.bulk().abort_query_job("750xx0000000001AAA").await?;
    /// ```
    pub async fn abort_query_job(&self, job_id: &str) -> Result<BulkQueryJobInfo> {
        let url = format!("{}/{}", self.query_base_url().await?, job_id);
        let inner = self.inner();

        let update_request = super::types::UpdateJobRequest {
            state: super::types::JobState::Aborted,
        };

        let request = inner
            .http_client
            .patch(&url)
            .json(&update_request)
            .build()
            .map_err(crate::error::HttpError::from)?;
        let response = inner.execute_request(request).await?;

        if !response.status().is_success() {
            return Err(handle_error_response(
                response,
                &format!("Abort query job request failed for job {}", job_id),
            )
            .await);
        }

        let job_info = response
            .json::<BulkQueryJobInfo>()
            .await
            .map_err(crate::error::HttpError::from)?;
        Ok(job_info)
    }

    /// Deletes a bulk query job.
    ///
    /// Permanently deletes a query job. Results will no longer be available.
    ///
    /// # Arguments
    ///
    /// * `job_id` - The unique identifier for the query job
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The job does not exist
    ///
    /// # Examples
    ///
    /// ```ignore
    /// client.bulk().delete_query_job("750xx0000000001AAA").await?;
    /// ```
    pub async fn delete_query_job(&self, job_id: &str) -> Result<()> {
        let url = format!("{}/{}", self.query_base_url().await?, job_id);
        let inner = self.inner();
        let request = inner
            .http_client
            .delete(&url)
            .build()
            .map_err(crate::error::HttpError::from)?;
        let response = inner.execute_request(request).await?;

        if !response.status().is_success() {
            return Err(handle_error_response(
                response,
                &format!("Delete query job request failed for job {}", job_id),
            )
            .await);
        }

        Ok(())
    }

    /// Streams results from a completed bulk query job.
    ///
    /// Returns a stream that lazily fetches and deserializes CSV results.
    /// This is memory-efficient for large result sets.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The record type to deserialize. Must implement `Deserialize`.
    ///
    /// # Arguments
    ///
    /// * `job_id` - The unique identifier for the completed query job
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The job is not complete
    /// - The HTTP request fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut stream = client.bulk().query_results::<Account>("750xx0000000001AAA").await?;
    /// while let Some(record) = stream.next().await? {
    ///     println!("{:?}", record);
    /// }
    /// ```
    #[allow(clippy::unused_async)]
    pub async fn query_results<T>(&self, job_id: &str) -> Result<BulkQueryStream<T, A>>
    where
        T: for<'de> Deserialize<'de>,
    {
        // Placeholder for GREEN phase
        Ok(BulkQueryStream::new(
            Arc::clone(self.inner()),
            job_id.to_string(),
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::bulk::types::JobState;
    use crate::auth::{AccessToken, Authenticator, TokenResponse};
    use crate::client::{ForceClient, builder};
    use crate::test_support::{Must, MustMsg};
    use async_trait::async_trait;
    use wiremock::matchers::{
        bearer_token, body_string_contains, header, method, path, query_param,
        query_param_is_missing,
    };
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
            .authenticate(auth)
            .build()
            .await
            .must_msg("failed to create test client")
    }

    // RED PHASE - Write failing tests first

    #[test]
    fn test_bulk_query_request_new() {
        let request = BulkQueryRequest::new("SELECT Id FROM Account");
        assert_eq!(request.query, "SELECT Id FROM Account");
        assert_eq!(request.operation, "query");
    }

    #[test]
    fn test_bulk_query_request_serialization() {
        let request = BulkQueryRequest::new("SELECT Id, Name FROM Contact");
        let json = serde_json::to_string(&request).must();
        assert!(json.contains(r#""query":"SELECT Id, Name FROM Contact""#));
        assert!(json.contains(r#""operation":"query""#));
    }

    #[tokio::test]
    async fn test_query_base_url_construction() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let base_url = handler.query_base_url().await.must();
        assert!(base_url.contains(&mock_server.uri()));
        assert!(base_url.contains("/services/data/"));
        assert!(base_url.ends_with("v60.0/jobs/query"));
    }

    #[tokio::test]
    async fn test_create_query_job_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/query"))
            .and(bearer_token("test_token"))
            .and(header("content-type", "application/json"))
            .and(body_string_contains("SELECT Id FROM Account"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000001AAA",
                "operation": "query",
                "state": "InProgress",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let request = BulkQueryRequest::new("SELECT Id FROM Account");
        let job = handler.create_query_job(request).await.must();

        assert_eq!(job.id, "750xx0000000001AAA");
        assert_eq!(job.operation, "query");
        assert_eq!(job.state, JobState::InProgress);
    }

    #[tokio::test]
    async fn test_create_query_job_with_complex_soql() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/query"))
            .and(body_string_contains(
                "SELECT Id, Name, (SELECT FirstName FROM Contacts)",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000002AAA",
                "operation": "query",
                "state": "InProgress",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let request =
            BulkQueryRequest::new("SELECT Id, Name, (SELECT FirstName FROM Contacts) FROM Account");
        let job = handler.create_query_job(request).await.must();

        assert_eq!(job.id, "750xx0000000002AAA");
    }

    #[tokio::test]
    async fn test_create_query_job_invalid_soql() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/query"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "message": "Invalid SOQL query",
                "errorCode": "INVALID_QUERY"
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let request = BulkQueryRequest::new("SELECT InvalidField FROM Account");
        let result = handler.create_query_job(request).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_query_job_in_progress() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/query/750xx0000000001AAA"))
            .and(bearer_token("test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000001AAA",
                "operation": "query",
                "state": "InProgress",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let job = handler.get_query_job("750xx0000000001AAA").await.must();

        assert_eq!(job.id, "750xx0000000001AAA");
        assert_eq!(job.state, JobState::InProgress);
    }

    #[tokio::test]
    async fn test_get_query_job_complete() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/query/750xx0000000001AAA"))
            .and(bearer_token("test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000001AAA",
                "operation": "query",
                "state": "JobComplete",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA",
                "numberRecordsProcessed": 1500,
                "totalProcessingTime": 8500
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let job = handler.get_query_job("750xx0000000001AAA").await.must();

        assert_eq!(job.state, JobState::JobComplete);
        assert_eq!(job.number_records_processed, Some(1500));
        assert_eq!(job.total_processing_time, Some(8500));
    }

    #[tokio::test]
    async fn test_get_query_job_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/query/750xx0000000999AAA"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let result = handler.get_query_job("750xx0000000999AAA").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_abort_query_job_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("PATCH"))
            .and(path("/services/data/v60.0/jobs/query/750xx0000000001AAA"))
            .and(bearer_token("test_token"))
            .and(header("content-type", "application/json"))
            .and(body_string_contains("Aborted"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000001AAA",
                "operation": "query",
                "state": "Aborted",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let job = handler.abort_query_job("750xx0000000001AAA").await.must();

        assert_eq!(job.state, JobState::Aborted);
    }

    #[tokio::test]
    async fn test_abort_query_job_already_complete() {
        let mock_server = MockServer::start().await;

        Mock::given(method("PATCH"))
            .and(path("/services/data/v60.0/jobs/query/750xx0000000001AAA"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "message": "Cannot abort completed job",
                "errorCode": "INVALID_OPERATION"
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let result = handler.abort_query_job("750xx0000000001AAA").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_query_job_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("DELETE"))
            .and(path("/services/data/v60.0/jobs/query/750xx0000000001AAA"))
            .and(bearer_token("test_token"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let result = handler.delete_query_job("750xx0000000001AAA").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_delete_query_job_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("DELETE"))
            .and(path("/services/data/v60.0/jobs/query/750xx0000000999AAA"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let result = handler.delete_query_job("750xx0000000999AAA").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_query_results_stream_creation() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        // Just test that we can create a stream (no HTTP call yet)
        let stream = handler
            .query_results::<serde_json::Value>("750xx0000000001AAA")
            .await
            .must();

        assert_eq!(stream.job_id, "750xx0000000001AAA");
        assert!(!stream.exhausted);
    }

    #[tokio::test]
    async fn test_query_results_fetch_csv_data() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/jobs/query/750xx0000000001AAA/results",
            ))
            .and(bearer_token("test_token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("Id,Name\n001xx000000001AAA,Acme\n001xx000000002AAA,Globex"),
            )
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let mut stream = handler
            .query_results::<serde_json::Value>("750xx0000000001AAA")
            .await
            .must();

        // This will fail in RED phase because next() is not implemented
        let record = stream.next().await.must();
        assert!(record.is_some());
    }

    #[tokio::test]
    async fn test_query_results_empty_result_set() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/jobs/query/750xx0000000001AAA/results",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string("Id,Name\n"))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let mut stream = handler
            .query_results::<serde_json::Value>("750xx0000000001AAA")
            .await
            .must();

        let record = stream.next().await.must();
        assert!(record.is_none());
    }

    #[tokio::test]
    async fn test_query_results_stream_terminates_after_records_consumed() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/jobs/query/750xx0000000001AAA/results",
            ))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Sforce-Locator", "null")
                    .set_body_string("Id,Name\n001xx000000001AAA,Acme\n001xx000000002AAA,Globex"),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let mut stream = handler
            .query_results::<serde_json::Value>("750xx0000000001AAA")
            .await
            .must();

        assert!(stream.next().await.must().is_some());
        assert!(stream.next().await.must().is_some());
        assert!(stream.next().await.must().is_none());
    }

    #[tokio::test]
    async fn test_query_results_encodes_locator_query_parameter() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/jobs/query/750xx0000000001AAA/results",
            ))
            .and(query_param_is_missing("locator"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Sforce-Locator", "next&page=2")
                    .set_body_string("Id,Name\n001xx000000001AAA,Acme"),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/jobs/query/750xx0000000001AAA/results",
            ))
            .and(query_param("locator", "next&page=2"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Sforce-Locator", "null")
                    .set_body_string("Id,Name\n001xx000000002AAA,Globex"),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let mut stream = handler
            .query_results::<serde_json::Value>("750xx0000000001AAA")
            .await
            .must();

        assert!(stream.next().await.must().is_some());
        assert!(stream.next().await.must().is_some());
        assert!(stream.next().await.must().is_none());
    }

    #[tokio::test]
    async fn test_bulk_query_job_info_deserialization() {
        let json = r#"{
            "id": "750xx0000000001AAA",
            "operation": "query",
            "state": "JobComplete",
            "createdDate": "2024-01-01T00:00:00.000Z",
            "createdById": "005xx0000000001AAA",
            "numberRecordsProcessed": 2500,
            "totalProcessingTime": 12000,
            "apiVersion": "60.0"
        }"#;

        let info: BulkQueryJobInfo = serde_json::from_str(json).must();
        assert_eq!(info.id, "750xx0000000001AAA");
        assert_eq!(info.operation, "query");
        assert_eq!(info.state, JobState::JobComplete);
        assert_eq!(info.number_records_processed, Some(2500));
        assert_eq!(info.total_processing_time, Some(12000));
        assert_eq!(info.api_version, Some("60.0".to_string()));
    }
}
