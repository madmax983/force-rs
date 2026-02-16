#![cfg(feature = "bulk")]

use crate::api::bulk::ingest::IngestJobBuilder;
use crate::api::bulk::types::{JobInfo, JobOperation};
use crate::api::bulk::csv;
use crate::error::Result;
use crate::auth::Authenticator;
use crate::client::ForceClient;
use serde::{Deserialize, Serialize};

/// Result of a bulk ingest operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkResult<T> {
    /// Information about the completed job.
    pub job_info: JobInfo,
    /// Records that were successfully processed.
    pub successful_records: Vec<SuccessfulRecord<T>>,
    /// Records that failed processing.
    pub failed_records: Vec<FailedRecord<T>>,
}

/// A wrapper for successfully processed records, including system fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessfulRecord<T> {
    /// The original record data.
    #[serde(flatten)]
    pub record: T,
    /// The Salesforce ID of the record.
    #[serde(rename = "sf__Id")]
    pub id: String,
    /// Whether the record was created (true) or updated (false).
    #[serde(rename = "sf__Created")]
    pub created: bool,
}

/// A wrapper for failed records, including the error message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedRecord<T> {
    /// The original record data.
    #[serde(flatten)]
    pub record: T,
    /// The error message returned by Salesforce.
    #[serde(rename = "sf__Error")]
    pub error: String,
}

/// Smart bulk ingester that handles the full lifecycle of a bulk job.
///
/// This struct provides a high-level API for performing bulk operations (insert, update, upsert, delete)
/// and automatically handling the job lifecycle, including polling and result retrieval.
///
/// # Examples
///
/// ```ignore
/// use force::experimental::smart_ingest::SmartIngester;
/// use force::api::bulk::types::JobOperation;
///
/// let ingester = SmartIngester::new(&client, "Account", JobOperation::Insert);
/// let results = ingester.ingest(&accounts).await?;
///
/// for success in results.successful_records {
///     println!("Created account: {}", success.id);
/// }
/// ```
pub struct SmartIngester<'a, A: Authenticator> {
    client: &'a ForceClient<A>,
    object: String,
    operation: JobOperation,
    external_id_field: Option<String>,
}

impl<'a, A: Authenticator> SmartIngester<'a, A> {
    /// Creates a new SmartIngester.
    ///
    /// # Arguments
    ///
    /// * `client` - The ForceClient instance.
    /// * `object` - The Salesforce object type (e.g., "Account").
    /// * `operation` - The operation to perform (Insert, Update, Upsert, Delete).
    pub fn new(client: &'a ForceClient<A>, object: impl Into<String>, operation: JobOperation) -> Self {
        Self {
            client,
            object: object.into(),
            operation,
            external_id_field: None,
        }
    }

    /// Sets the external ID field for upsert operations.
    pub fn external_id_field(mut self, field: impl Into<String>) -> Self {
        self.external_id_field = Some(field.into());
        self
    }

    /// Performs the bulk ingest operation.
    ///
    /// This method:
    /// 1. Serializes the records to CSV.
    /// 2. Creates a bulk job.
    /// 3. Uploads the data.
    /// 4. Closes the job.
    /// 5. Polls until completion.
    /// 6. Retrieves and deserializes the results.
    ///
    /// # Errors
    ///
    /// Returns an error if any step of the process fails.
    pub async fn ingest<T>(&self, records: &[T]) -> Result<BulkResult<T>>
    where
        T: Serialize + for<'de> Deserialize<'de> + Sync + Send + Clone,
    {
        // 1. Serialize records to CSV
        let mut csv_data = Vec::new();
        csv::serialize_to_csv(records, &mut csv_data)?;

        // 2. Create job
        let mut builder = IngestJobBuilder::new(&self.object, self.operation);
        if let Some(field) = &self.external_id_field {
            builder = builder.external_id_field(field);
        }

        let job = builder.build(&self.client.bulk()).await?;

        // 3. Upload data
        let job = job.upload(&csv_data).await?;

        // 4. Close job
        let job = job.close().await?;

        // 5. Poll until completion
        let job = job.poll_until_complete().await?;

        // 6. Retrieve results
        let successful_bytes = job.successful_results().await?;
        let failed_bytes = job.failed_results().await?;

        // Get final job info
        let job_info = self.client.bulk().get_job(job.job_id()).await?;

        // Parse results
        // Note: Salesforce returns empty body for empty results sometimes, or just header.
        // csv::deserialize_from_csv handles empty input gracefully (returns empty vec).

        let successful_records: Vec<SuccessfulRecord<T>> = if successful_bytes.is_empty() {
            Vec::new()
        } else {
            csv::deserialize_from_csv(successful_bytes.as_slice())?
        };

        let failed_records: Vec<FailedRecord<T>> = if failed_bytes.is_empty() {
            Vec::new()
        } else {
            csv::deserialize_from_csv(failed_bytes.as_slice())?
        };

        Ok(BulkResult {
            job_info,
            successful_records,
            failed_records,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{Must, MustMsg};
    use crate::client::builder;
    use crate::auth::{AccessToken, Authenticator, TokenResponse};
    use async_trait::async_trait;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // Mock authenticator
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

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
    struct TestAccount {
        #[serde(rename = "Name")]
        name: String,
        #[serde(rename = "Industry")]
        industry: String,
    }

    #[tokio::test]
    async fn test_smart_ingest_success() {
        let mock_server = MockServer::start().await;

        // Mock: Create job
        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/jobs/ingest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000001AAA",
                "operation": "insert",
                "object": "Account",
                "state": "Open",
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        // Mock: Upload CSV
        Mock::given(method("PUT"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000001AAA/batches"))
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

        // Mock: Poll job (complete)
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000001AAA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "750xx0000000001AAA",
                "operation": "insert",
                "object": "Account",
                "state": "JobComplete",
                "numberRecordsProcessed": 1,
                "numberRecordsFailed": 0,
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        // Mock: Successful results
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000001AAA/successfulResults"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "Name,Industry,sf__Id,sf__Created\nAcme,Tech,001xx0000000001AAA,true\n"
            ))
            .mount(&mock_server)
            .await;

        // Mock: Failed results (empty)
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000001AAA/failedResults"))
            .respond_with(ResponseTemplate::new(200).set_body_string(""))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let ingester = SmartIngester::new(&client, "Account", JobOperation::Insert);

        let records = vec![TestAccount {
            name: "Acme".to_string(),
            industry: "Tech".to_string(),
        }];

        let result = ingester.ingest(&records).await.must();

        assert_eq!(result.successful_records.len(), 1);
        assert_eq!(result.failed_records.len(), 0);
        assert_eq!(result.successful_records[0].id, "001xx0000000001AAA");
        assert!(result.successful_records[0].created);
        assert_eq!(result.successful_records[0].record.name, "Acme");
    }

    #[tokio::test]
    async fn test_smart_ingest_with_failures() {
        let mock_server = MockServer::start().await;

        // Setup mocks for job lifecycle...
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
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000002AAA/batches"))
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
                "numberRecordsProcessed": 1,
                "numberRecordsFailed": 1,
                "createdDate": "2024-01-01T00:00:00.000Z",
                "createdById": "005xx0000000001AAA"
            })))
            .mount(&mock_server)
            .await;

        // Mock: Successful results
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000002AAA/successfulResults"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "Name,Industry,sf__Id,sf__Created\nAcme,Tech,001xx0000000001AAA,true\n"
            ))
            .mount(&mock_server)
            .await;

        // Mock: Failed results
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/jobs/ingest/750xx0000000002AAA/failedResults"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "Name,Industry,sf__Error\nBad,,Required field missing\n"
            ))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let ingester = SmartIngester::new(&client, "Account", JobOperation::Insert);

        let records = vec![
            TestAccount { name: "Acme".to_string(), industry: "Tech".to_string() },
            TestAccount { name: "Bad".to_string(), industry: "".to_string() },
        ];

        let result = ingester.ingest(&records).await.must();

        assert_eq!(result.successful_records.len(), 1);
        assert_eq!(result.failed_records.len(), 1);
        assert_eq!(result.failed_records[0].error, "Required field missing");
        assert_eq!(result.failed_records[0].record.name, "Bad");
    }
}
