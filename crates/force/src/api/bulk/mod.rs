//! Bulk API 2.0 handler for Salesforce.
//!
//! This module provides the `BulkHandler` which serves as the foundation for all
//! Bulk API 2.0 operations including ingest jobs and bulk queries.

pub mod ingest;
pub mod query;
pub mod types;

#[cfg(feature = "bulk")]
pub mod csv;

use crate::error::Result;
use std::time::Duration;
use std::sync::Arc;
use types::{CreateJobRequest, JobInfo, UpdateJobRequest};

/// Polling behavior for asynchronous Bulk API jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BulkPollPolicy {
    /// Maximum number of polling retries while a job remains non-terminal.
    pub max_attempts: u32,
    /// Initial backoff delay before the next poll attempt.
    pub initial_backoff: Duration,
    /// Upper bound for exponential polling backoff.
    pub max_backoff: Duration,
}

impl BulkPollPolicy {
    /// Creates a new polling policy.
    #[must_use]
    pub const fn new(
        max_attempts: u32,
        initial_backoff: Duration,
        max_backoff: Duration,
    ) -> Self {
        Self {
            max_attempts,
            initial_backoff,
            max_backoff,
        }
    }

    #[must_use]
    pub(crate) fn backoff_for_attempt(self, attempt: u32) -> Duration {
        let shift = attempt.min(31);
        let multiplier = 1_u32 << shift;
        let Some(backoff) = self.initial_backoff.checked_mul(multiplier) else {
            return self.max_backoff;
        };
        backoff.min(self.max_backoff)
    }

    #[must_use]
    pub(crate) fn timeout_seconds(self) -> u64 {
        let mut total = Duration::ZERO;
        let mut attempt = 0;
        while attempt < self.max_attempts {
            total = total.saturating_add(self.backoff_for_attempt(attempt));
            attempt += 1;
        }
        total.as_secs()
    }
}

impl Default for BulkPollPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 10,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(30),
        }
    }
}

/// Bulk API 2.0 handler for performing Salesforce bulk operations.
///
/// The handler provides access to all Bulk API functionality including:
/// - Ingest jobs for high-volume data operations
/// - Bulk queries for retrieving large datasets
/// - Job state management
/// - CSV data upload/download
///
/// The handler is obtained from a `ForceClient` and shares its authentication
/// and configuration.
#[derive(Debug, Clone)]
pub struct BulkHandler<A: crate::auth::Authenticator> {
    /// Reference to the client's inner state.
    pub(crate) inner: Arc<crate::client::Inner<A>>,
}

impl<A: crate::auth::Authenticator> BulkHandler<A> {
    /// Creates a new Bulk handler for the given client inner state.
    ///
    /// # Arguments
    ///
    /// * `inner` - The client's inner state
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let handler = BulkHandler::new(inner);
    /// ```
    #[must_use]
    pub(crate) fn new(inner: Arc<crate::client::Inner<A>>) -> Self {
        Self { inner }
    }

    /// Returns a reference to the client's inner state.
    ///
    /// This is used internally by bulk API modules to access the HTTP client
    /// and token manager.
    #[must_use]
    pub(crate) fn inner(&self) -> &Arc<crate::client::Inner<A>> {
        &self.inner
    }

    /// Constructs the base URL for Bulk API 2.0 operations.
    ///
    /// The base URL is constructed as: `{instance_url}/services/data/{api_version}/jobs/ingest`
    ///
    /// This method requires token access to get the instance URL from authentication.
    ///
    /// # Errors
    ///
    /// Returns an error if token retrieval fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let base = handler.base_url().await?;
    /// // Returns: "https://na1.salesforce.com/services/data/v60.0/jobs/ingest"
    /// ```
    pub async fn base_url(&self) -> Result<String> {
        let token = self.inner.token_manager.token().await?;
        Ok(format!(
            "{}/services/data/{}/jobs/ingest",
            token.instance_url(),
            self.inner.config.api_version
        ))
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
        let url = self.base_url().await?;
        let request = self
            .inner
            .http_client
            .post(&url)
            .json(&request)
            .build()
            .map_err(crate::error::HttpError::from)?;
        let response = self.inner.execute_request(request).await?;

        if !response.status().is_success() {
            return Err(
                crate::http::response_to_force_error(response, "Create job request failed").await,
            );
        }

        let job_info = response
            .json::<JobInfo>()
            .await
            .map_err(crate::error::HttpError::from)?;
        Ok(job_info)
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
        let url = format!("{}/{}", self.base_url().await?, job_id);
        let request = self
            .inner
            .http_client
            .get(&url)
            .build()
            .map_err(crate::error::HttpError::from)?;
        let response = self.inner.execute_request(request).await?;

        if !response.status().is_success() {
            return Err(
                crate::http::response_to_force_error(
                    response,
                    &format!("Get job request failed for job {}", job_id),
                )
                .await,
            );
        }

        let job_info = response
            .json::<JobInfo>()
            .await
            .map_err(crate::error::HttpError::from)?;
        Ok(job_info)
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
        let url = format!("{}/{}", self.base_url().await?, job_id);
        let request = self
            .inner
            .http_client
            .patch(&url)
            .json(&request)
            .build()
            .map_err(crate::error::HttpError::from)?;
        let response = self.inner.execute_request(request).await?;

        if !response.status().is_success() {
            return Err(
                crate::http::response_to_force_error(
                    response,
                    &format!("Update job request failed for job {}", job_id),
                )
                .await,
            );
        }

        let job_info = response
            .json::<JobInfo>()
            .await
            .map_err(crate::error::HttpError::from)?;
        Ok(job_info)
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
        let url = format!("{}/{}", self.base_url().await?, job_id);
        let request = self
            .inner
            .http_client
            .delete(&url)
            .build()
            .map_err(crate::error::HttpError::from)?;
        let response = self.inner.execute_request(request).await?;

        if !response.status().is_success() {
            return Err(
                crate::http::response_to_force_error(
                    response,
                    &format!("Delete job request failed for job {}", job_id),
                )
                .await,
            );
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
    /// let job = client.bulk().bulk_insert("Account", &accounts).await?;
    /// println!("Processed: {}", job.number_records_processed.unwrap_or(0));
    /// ```
    #[cfg(feature = "bulk")]
    pub async fn bulk_insert<T>(&self, object: &str, records: &[T]) -> Result<types::JobInfo>
    where
        T: serde::Serialize + Sync,
    {
        use ingest::IngestJobBuilder;
        use types::JobOperation;

        // Serialize records to CSV
        let mut csv_data = Vec::new();
        csv::serialize_to_csv(records, &mut csv_data)?;

        // Create job
        let job = IngestJobBuilder::new(object, JobOperation::Insert)
            .build_with_inner(Arc::clone(&self.inner))
            .await?;

        // Upload, close, and poll
        let job = job.upload(&csv_data).await?;
        let job = job.close().await?;
        let job = job.poll_until_complete().await?;

        // Get final job info
        let job_info = self.get_job(job.job_id()).await?;
        Ok(job_info)
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
    /// let job = client.bulk().bulk_update("Account", &accounts).await?;
    /// println!("Processed: {}", job.number_records_processed.unwrap_or(0));
    /// ```
    #[cfg(feature = "bulk")]
    pub async fn bulk_update<T>(&self, object: &str, records: &[T]) -> Result<types::JobInfo>
    where
        T: serde::Serialize + Sync,
    {
        use ingest::IngestJobBuilder;
        use types::JobOperation;

        // Serialize records to CSV
        let mut csv_data = Vec::new();
        csv::serialize_to_csv(records, &mut csv_data)?;

        // Create job
        let job = IngestJobBuilder::new(object, JobOperation::Update)
            .build_with_inner(Arc::clone(&self.inner))
            .await?;

        // Upload, close, and poll
        let job = job.upload(&csv_data).await?;
        let job = job.close().await?;
        let job = job.poll_until_complete().await?;

        // Get final job info
        let job_info = self.get_job(job.job_id()).await?;
        Ok(job_info)
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
    /// let job = client.bulk().bulk_delete("Account", &ids).await?;
    /// println!("Deleted: {}", job.number_records_processed.unwrap_or(0));
    /// ```
    #[cfg(feature = "bulk")]
    pub async fn bulk_delete(&self, object: &str, ids: &[String]) -> Result<types::JobInfo> {
        use ingest::IngestJobBuilder;
        use types::JobOperation;

        // Create CSV with Id column
        #[derive(serde::Serialize)]
        struct DeleteRecord {
            #[serde(rename = "Id")]
            id: String,
        }

        let delete_records: Vec<DeleteRecord> = ids
            .iter()
            .map(|id| DeleteRecord { id: id.clone() })
            .collect();

        let mut csv_data = Vec::new();
        csv::serialize_to_csv(&delete_records, &mut csv_data)?;

        // Create job
        let job = IngestJobBuilder::new(object, JobOperation::Delete)
            .build_with_inner(Arc::clone(&self.inner))
            .await?;

        // Upload, close, and poll
        let job = job.upload(&csv_data).await?;
        let job = job.close().await?;
        let job = job.poll_until_complete().await?;

        // Get final job info
        let job_info = self.get_job(job.job_id()).await?;
        Ok(job_info)
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
    /// let mut stream = client.bulk().bulk_query::<Account>(soql).await?;
    ///
    /// while let Some(account) = stream.next().await? {
    ///     println!("{}: {}", account.id, account.name);
    /// }
    /// ```
    #[cfg(feature = "bulk")]
    pub async fn bulk_query<T>(&self, soql: &str) -> Result<query::BulkQueryStream<T, A>>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        self.bulk_query_with_policy(soql, BulkPollPolicy::default())
            .await
    }

    /// Creates a bulk query job with a custom polling policy and returns a stream of results.
    ///
    /// This variant lets callers tune polling behavior for long-running jobs.
    #[cfg(feature = "bulk")]
    pub async fn bulk_query_with_policy<T>(
        &self,
        soql: &str,
        poll_policy: BulkPollPolicy,
    ) -> Result<query::BulkQueryStream<T, A>>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        use query::BulkQueryRequest;

        // Create query job
        let request = BulkQueryRequest::new(soql);
        let job = self.create_query_job(request).await?;
        self.poll_query_job_until_complete(&job.id, poll_policy).await?;

        // Return results stream
        self.query_results(&job.id).await
    }

    #[cfg(feature = "bulk")]
    async fn poll_query_job_until_complete(
        &self,
        job_id: &str,
        poll_policy: BulkPollPolicy,
    ) -> Result<()> {
        use types::JobState;

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
use crate::test_support::{Must, MustMsg};
    use super::*;
    use crate::auth::{AccessToken, Authenticator, TokenResponse};
    use crate::client::{ForceClient, builder};
    use crate::config::ClientConfigBuilder;
    use async_trait::async_trait;
    use types::{ContentType, JobOperation, JobState};
    use wiremock::matchers::{bearer_token, header, method, path, path_regex};
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

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_bulk_handler_construction() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(mock_server.uri()).await;
        let _handler = client.bulk();
        // If we get here, handler was created successfully
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_bulk_handler_is_cloneable() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(mock_server.uri()).await;
        let handler1 = client.bulk();
        let handler2 = handler1.clone();

        // Both should produce the same base URL
        let url1 = handler1.base_url().await.must();
        let url2 = handler2.base_url().await.must();
        assert_eq!(url1, url2);
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_base_url_construction() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let base_url = handler.base_url().await.must();
        assert!(base_url.contains(&mock_server.uri()));
        assert!(base_url.contains("/services/data/"));
        assert!(base_url.ends_with("v60.0/jobs/ingest")); // Default API version
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_base_url_with_custom_api_version() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let config = ClientConfigBuilder::new().api_version("v59.0").build();
        let client = builder()
            .authenticate(auth)
            .config(config)
            .build()
            .await
            .must();

        let handler = client.bulk();
        let base_url = handler.base_url().await.must();

        assert!(base_url.ends_with("v59.0/jobs/ingest"));
    }

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
        assert!(result.is_err());
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
        assert!(result.is_err());
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
        assert!(result.is_err());
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
        assert!(result.is_err());
    }

    // RED PHASE: Convenience method tests
    // These tests will fail until implementation is added

    #[cfg(feature = "bulk")]
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
            .and(path("/services/data/v60.0/jobs/query/750xx0000000006AAA/results"))
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
        assert!(result.is_err());
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
        assert!(result.is_err());
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
            .and(path("/services/data/v60.0/jobs/query/750xx0000000010AAA/results"))
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

        assert!(result.is_err());
    }
}
