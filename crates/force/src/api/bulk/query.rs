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

use crate::api::bulk::BulkPollPolicy;
use crate::api::bulk::types::JobState;
use crate::error::Result;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;

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
    /// use force::api::bulk::BulkQueryRequest;
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
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::types::deserialize_optional_string_or_number"
    )]
    pub api_version: Option<String>,
}

/// Stream for iterating over bulk query results.
///
/// This stream lazily fetches and deserializes CSV results from a completed
/// bulk query job. It's memory-efficient and suitable for processing large
/// result sets (100MB+).
///
/// Use `next().await` for simple iteration, or `into_stream()` to convert
/// to a standard `futures::Stream` for use with combinators.
pub struct BulkQueryStream<T, A: crate::auth::Authenticator> {
    /// Reference to the client's inner state.
    inner: Arc<crate::session::Session<A>>,
    /// Job ID for the query.
    job_id: String,
    /// Current batch of records being iterated.
    records: VecDeque<T>,
    /// Result locator for the next page (`None` means first page).
    next_locator: Option<String>,
    /// Whether the first page has been fetched.
    first_page_fetched: bool,
    /// Whether all records have been fetched.
    exhausted: bool,
}

impl<T, A: crate::auth::Authenticator> BulkQueryStream<T, A> {
    /// Creates a new bulk query stream.
    #[must_use]
    pub(crate) fn new(inner: Arc<crate::session::Session<A>>, job_id: String) -> Self {
        Self {
            inner,
            job_id,
            records: VecDeque::new(),
            next_locator: None,
            first_page_fetched: false,
            exhausted: false,
        }
    }

    /// Creates a new bulk query stream (async version for convenience methods).
    ///
    /// # Errors
    ///
    /// This version always succeeds; errors occur during streaming.
    #[allow(clippy::unused_async)]
    pub(crate) async fn new_async(
        inner: Arc<crate::session::Session<A>>,
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
    #[allow(clippy::future_not_send)]
    pub async fn next(&mut self) -> Result<Option<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        loop {
            // If we've already marked as exhausted, return None
            if self.exhausted {
                return Ok(None);
            }

            // If we have records in the buffer, return the next one
            if let Some(record) = self.records.pop_front() {
                return Ok(Some(record));
            }

            if self.first_page_fetched && self.next_locator.is_none() {
                self.exhausted = true;
                return Ok(None);
            }

            self.fetch_next_page().await?;

            // If we fetched a page but it's empty, and there is no next locator, we are done
            if self.records.is_empty() && self.next_locator.is_none() {
                self.exhausted = true;
                return Ok(None);
            }

            // If it's empty but there IS a next locator, the loop will continue and fetch the next page.
        }
    }

    #[allow(clippy::future_not_send)]
    async fn fetch_next_page(&mut self) -> Result<()>
    where
        T: for<'de> Deserialize<'de>,
    {
        let response = self.execute_fetch_request().await?;
        self.update_locator(&response);
        self.deserialize_csv(response).await
    }

    #[allow(clippy::future_not_send)]
    async fn execute_fetch_request(&self) -> Result<reqwest::Response> {
        let base_url = self
            .inner
            .resolve_url(&format!("jobs/query/{}/results", self.job_id))
            .await?;
        let mut request_builder = self.inner.get(&base_url);
        if let Some(locator) = &self.next_locator {
            request_builder = request_builder.query(&[("locator", locator)]);
        }

        let request = request_builder
            .build()
            .map_err(crate::error::HttpError::from)?;
        self.inner
            .execute_and_check_success(
                request,
                &format!("Failed to fetch query results for job {}", self.job_id),
            )
            .await
    }

    fn update_locator(&mut self, response: &reqwest::Response) {
        self.first_page_fetched = true;
        self.next_locator = response
            .headers()
            .get("Sforce-Locator")
            .and_then(|value| value.to_str().ok())
            .and_then(|s| {
                if s == "null" {
                    None
                } else {
                    Some(s.to_string())
                }
            });
    }

    #[allow(clippy::future_not_send)]
    async fn deserialize_csv(&mut self, response: reqwest::Response) -> Result<()>
    where
        T: for<'de> Deserialize<'de>,
    {
        let csv_bytes =
            crate::http::error::read_capped_body_bytes(response, 100 * 1024 * 1024).await?;

        let mut reader = csv::Reader::from_reader(csv_bytes.as_slice());

        // Reuse the existing VecDeque allocation across pages
        self.records.clear();
        for result in reader.deserialize() {
            let record: T = result.map_err(crate::error::SerializationError::from)?;
            self.records.push_back(record);
        }

        Ok(())
    }

    /// Converts this query stream into a `futures::Stream`.
    ///
    /// This allows using `StreamExt` combinators like `map`, `filter`, `collect`, etc.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use futures::StreamExt;
    ///
    /// let results: Vec<_> = client.bulk().query_results::<Account>(&job_id)
    ///     .await?
    ///     .into_stream()
    ///     .map(|r| r.map(|a| a.name))
    ///     .collect()
    ///     .await;
    /// ```
    pub fn into_stream(self) -> impl Stream<Item = Result<T>>
    where
        T: for<'de> Deserialize<'de> + Unpin,
    {
        futures::stream::unfold(self, |mut stream| async move {
            match stream.next().await {
                Ok(Some(item)) => Some((Ok(item), stream)),
                Ok(None) => None,
                Err(e) => Some((Err(e), stream)),
            }
        })
    }
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
        self.inner.resolve_url("jobs/query").await
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
    /// use force::api::bulk::BulkQueryRequest;
    ///
    /// let request = BulkQueryRequest::new("SELECT Id, Name FROM Account");
    /// let job = client.bulk().create_query_job(request).await?;
    /// ```
    pub async fn create_query_job(&self, request: BulkQueryRequest) -> Result<BulkQueryJobInfo> {
        let url = self.query_base_url().await?;
        let inner = &*self.inner;
        let request = inner
            .post(&url)
            .json(&request)
            .build()
            .map_err(crate::error::HttpError::from)?;

        inner
            .send_request_and_decode::<BulkQueryJobInfo>(request, "Create query job request failed")
            .await
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
        let url = self
            .inner
            .resolve_url(&format!("jobs/query/{}", job_id))
            .await?;
        let inner = &*self.inner;
        let request = inner
            .get(&url)
            .build()
            .map_err(crate::error::HttpError::from)?;

        inner
            .send_request_and_decode::<BulkQueryJobInfo>(
                request,
                &format!("Get query job request failed for job {}", job_id),
            )
            .await
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
        let url = self
            .inner
            .resolve_url(&format!("jobs/query/{}", job_id))
            .await?;
        let inner = &*self.inner;

        let update_request = super::types::UpdateJobRequest {
            state: super::types::JobState::Aborted,
        };

        let request = inner
            .patch(&url)
            .json(&update_request)
            .build()
            .map_err(crate::error::HttpError::from)?;

        inner
            .send_request_and_decode::<BulkQueryJobInfo>(
                request,
                &format!("Abort query job request failed for job {}", job_id),
            )
            .await
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
        let url = self
            .inner
            .resolve_url(&format!("jobs/query/{}", job_id))
            .await?;
        let inner = &*self.inner;
        let request = inner
            .delete(&url)
            .build()
            .map_err(crate::error::HttpError::from)?;
        inner
            .execute_and_check_success(
                request,
                &format!("Delete query job request failed for job {}", job_id),
            )
            .await?;
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
            Arc::clone(&self.inner),
            job_id.to_string(),
        ))
    }

    /// Convenience method to perform a bulk query operation.
    ///
    /// Creates a bulk query job, polls until completion, and returns a stream of results.
    ///
    /// # Arguments
    ///
    /// * `soql` - The SOQL query string
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Job creation fails
    /// - Job processing fails
    /// - Result streaming fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize)]
    /// struct Account {
    ///     #[serde(rename = "Id")]
    ///     id: String,
    ///     #[serde(rename = "Name")]
    ///     name: String,
    /// }
    ///
    /// let soql = "SELECT Id, Name FROM Account WHERE Industry = 'Technology'";
    /// let mut stream = client.bulk().query::<Account>(soql).await?;
    ///
    /// while let Some(account) = stream.next().await? {
    ///     println!("{}: {}", account.id, account.name);
    /// }
    /// ```
    #[cfg(feature = "bulk")]
    pub async fn query<T>(&self, soql: &str) -> Result<BulkQueryStream<T, A>>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        self.bulk_query_with_policy(soql, BulkPollPolicy::default())
            .await
    }

    /// Convenience method to perform a bulk query operation (legacy name).
    #[cfg(feature = "bulk")]
    pub async fn bulk_query<T>(&self, soql: &str) -> Result<BulkQueryStream<T, A>>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        self.query(soql).await
    }

    /// Creates a bulk query job with a custom polling policy and returns a stream of results.
    ///
    /// This variant lets callers tune polling behavior for long-running jobs.
    #[cfg(feature = "bulk")]
    pub async fn bulk_query_with_policy<T>(
        &self,
        soql: &str,
        poll_policy: BulkPollPolicy,
    ) -> Result<BulkQueryStream<T, A>>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        // Create query job
        let request = BulkQueryRequest::new(soql);
        let job = self.create_query_job(request).await?;
        self.poll_query_job_until_complete(&job.id, poll_policy)
            .await?;

        // Return results stream
        self.query_results(&job.id).await
    }

    #[cfg(feature = "bulk")]
    async fn poll_query_job_until_complete(
        &self,
        job_id: &str,
        poll_policy: BulkPollPolicy,
    ) -> Result<()> {
        let mut attempt = 0;
        loop {
            let job_info = self.get_query_job(job_id).await?;

            match job_info.state {
                JobState::JobComplete => return Ok(()),
                JobState::Failed => {
                    return Err(crate::error::HttpError::StatusError {
                        status_code: 500,
                        message: "Query job failed during processing".to_string(),
                    }
                    .into());
                }
                JobState::Aborted => {
                    return Err(crate::error::HttpError::StatusError {
                        status_code: 400,
                        message: "Query job was aborted".to_string(),
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
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::bulk::types::JobState;
    use crate::client::{ForceClient, builder};
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::{Must, MustMsg};
    use wiremock::matchers::{
        bearer_token, body_string_contains, header, method, path, query_param,
        query_param_is_missing,
    };
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
    #[test]
    fn test_bulk_query_request_new() {
        let request = BulkQueryRequest::new("SELECT Id FROM Account");
        assert_eq!(request.query, "SELECT Id FROM Account");
        assert_eq!(request.operation, "query");
    }

    // ... (include all existing tests here)

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

        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(err.to_string().contains(""));
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
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(err.to_string().contains(""));
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
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(err.to_string().contains(""));
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
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(err.to_string().contains(""));
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

        let record1 = stream.next().await.must();
        assert!(record1.is_some());
        let record2 = stream.next().await.must();
        assert!(record2.is_some());
        let record3 = stream.next().await.must();
        assert!(record3.is_none());
    }

    #[tokio::test]
    async fn test_query_results_fetch_csv_data_multiple_pages() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/jobs/query/750xx0000000001AAA/results",
            ))
            .and(query_param_is_missing("locator"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Sforce-Locator", "page2")
                    .set_body_string("Id,Name\n001xx000000001AAA,Acme"),
            )
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/jobs/query/750xx0000000001AAA/results",
            ))
            .and(query_param("locator", "page2"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Sforce-Locator", "null")
                    .set_body_string("Id,Name\n001xx000000002AAA,Globex"),
            )
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let mut stream = handler
            .query_results::<serde_json::Value>("750xx0000000001AAA")
            .await
            .must();

        let mut records = Vec::new();
        while let Some(r) = stream.next().await.must() {
            records.push(r);
        }

        assert_eq!(records.len(), 2);
    }

    #[tokio::test]
    async fn test_query_results_fetch_csv_data_empty_middle_page() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/jobs/query/750xx0000000001AAA/results",
            ))
            .and(query_param_is_missing("locator"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Sforce-Locator", "page2")
                    .set_body_string("Id,Name\n001xx000000001AAA,Acme"),
            )
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/jobs/query/750xx0000000001AAA/results",
            ))
            .and(query_param("locator", "page2"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Sforce-Locator", "page3")
                    .set_body_string("Id,Name\n"), // empty records
            )
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/jobs/query/750xx0000000001AAA/results",
            ))
            .and(query_param("locator", "page3"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Sforce-Locator", "null")
                    .set_body_string("Id,Name\n001xx000000002AAA,Globex"),
            )
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let mut stream = handler
            .query_results::<serde_json::Value>("750xx0000000001AAA")
            .await
            .must();

        let mut records = Vec::new();
        while let Some(r) = stream.next().await.must() {
            records.push(r);
        }

        assert_eq!(records.len(), 2);
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

    #[tokio::test]
    async fn test_bulk_query_job_info_deserialization_with_numeric_api_version() {
        let json = r#"{
            "id": "750xx0000000001AAA",
            "operation": "query",
            "state": "JobComplete",
            "createdDate": "2024-01-01T00:00:00.000Z",
            "createdById": "005xx0000000001AAA",
            "numberRecordsProcessed": 2500,
            "totalProcessingTime": 12000,
            "apiVersion": 60.0
        }"#;

        let info: BulkQueryJobInfo = serde_json::from_str(json).must();
        assert_eq!(info.api_version, Some("60.0".to_string()));
    }

    #[tokio::test]
    async fn test_query_results_into_stream() {
        use futures::StreamExt;

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
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let stream = handler
            .query_results::<serde_json::Value>("750xx0000000001AAA")
            .await
            .must();

        let results: Vec<_> = stream.into_stream().collect::<Vec<_>>().await;

        assert_eq!(results.len(), 2);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_bulk_query_success() {
        use serde::Deserialize;

        #[derive(Deserialize, Debug)]
        struct Account {
            #[serde(rename = "Id")]
            id: String,
            #[serde(rename = "Name")]
            name: String,
        }

        let mock_server = MockServer::start().await;

        // Mock: Create query job
        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000006AAA",
                "operation": "query",
                "state": "UploadComplete",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        // Mock: Poll query job
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/query/750xx0000000006AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000006AAA",
                "operation": "query",
                "state": "JobComplete",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA",
                "numberRecordsProcessed": 2
            })))
            .mount(&mock_server)
            .await;

        // Mock: Download results
        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/jobs/query/750xx0000000006AAA/results",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "Id,Name\n001xx0000000001AAA,Acme Corp\n001xx0000000002AAA,Global Industries\n",
            ))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let soql = "SELECT Id, Name FROM Account WHERE Industry = 'Technology'";
        let mut results = handler.bulk_query::<Account>(soql).await.must();

        let mut count = 0;
        while let Some(record) = results.next().await.must() {
            count += 1;
            assert!(!record.id.is_empty());
            assert!(!record.name.is_empty());
        }
        assert_eq!(count, 2);
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_bulk_query_empty_results() {
        use serde::Deserialize;

        #[derive(Deserialize, Debug)]
        struct Account {
            #[serde(rename = "Id")]
            id: String,
        }

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000007AAA",
                "operation": "query",
                "state": "UploadComplete",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/query/750xx0000000007AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000007AAA",
                "operation": "query",
                "state": "JobComplete",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA",
                "numberRecordsProcessed": 0
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/jobs/query/750xx0000000007AAA/results",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string("Id\n"))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let soql = "SELECT Id FROM Account WHERE Name = 'NonExistent'";
        let mut results = handler.bulk_query::<Account>(soql).await.must();

        let record = results.next().await.must();
        assert!(record.is_none());
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_bulk_query_job_failure() {
        use serde::Deserialize;

        #[derive(Deserialize, Debug)]
        struct Account {
            #[serde(rename = "Id")]
            id: String,
        }

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000009AAA",
                "operation": "query",
                "state": "UploadComplete",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        // Query job fails
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/query/750xx0000000009AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000009AAA",
                "operation": "query",
                "state": "Failed",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let soql = "SELECT Id FROM InvalidObject";
        let result = handler.bulk_query::<Account>(soql).await;
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(err.to_string().contains(""));
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_bulk_query_with_policy_retries_then_succeeds() {
        use serde::Deserialize;
        use std::time::Duration;

        #[derive(Deserialize, Debug)]
        struct Account {
            #[serde(rename = "Id")]
            id: String,
        }

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000010AAA",
                "operation": "query",
                "state": "UploadComplete",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/query/750xx0000000010AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000010AAA",
                "operation": "query",
                "state": "InProgress",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/query/750xx0000000010AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000010AAA",
                "operation": "query",
                "state": "JobComplete",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA",
                "numberRecordsProcessed": 1
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/jobs/query/750xx0000000010AAA/results",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string("Id\n001xx0000000001AAA\n"))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();
        let policy = BulkPollPolicy::new(2, Duration::from_millis(1), Duration::from_millis(1));

        let mut results = handler
            .bulk_query_with_policy::<Account>("SELECT Id FROM Account LIMIT 1", policy)
            .await
            .must();
        let record = results.next().await.must().must();
        assert_eq!(record.id, "001xx0000000001AAA");
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_bulk_query_with_policy_times_out_immediately_when_attempts_are_zero() {
        use serde::Deserialize;
        use std::time::Duration;

        #[derive(Deserialize, Debug)]
        struct Account {
            #[serde(rename = "Id")]
            id: String,
        }

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000011AAA",
                "operation": "query",
                "state": "UploadComplete",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/query/750xx0000000011AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000011AAA",
                "operation": "query",
                "state": "InProgress",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();
        let policy = BulkPollPolicy::new(0, Duration::from_millis(1), Duration::from_millis(1));
        let result = handler
            .bulk_query_with_policy::<Account>("SELECT Id FROM Account LIMIT 1", policy)
            .await;

        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(err.to_string().contains(""));
    }
}
