//! Mass Operations using SOQL and Composite Batch API.
//!
//! This module provides the `SoqlMassOp` utility, which combines the power of
//! SOQL querying with the efficiency of the Composite Batch API. It allows you
//! to automatically delete or update a large set of records directly from a query,
//! handling pagination and batching (up to 25 requests per batch) behind the scenes.
//!
//! # The Spark
//! We have `SoqlQueryBuilder` for safe queries and `BatchBuilder` for executing
//! up to 25 subrequests. What if we connect them? You could query for records
//! and mass update or delete them without writing boilerplate pagination and batching loops.
//!
//! # Example
//!
//! ```no_run
//! # use force::client::ForceClientBuilder;
//! # use force::api::SoqlQueryBuilder;
//! # use force::api::composite::SoqlMassOp;
//! # use force::auth::ClientCredentials;
//! # use serde_json::json;
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! # let auth = ClientCredentials::new("id", "secret", "url");
//! # let client = ForceClientBuilder::new().authenticate(auth).build().await?;
//! // 1. Build a safe SOQL query to find records
//! let query = SoqlQueryBuilder::new()
//!     .select(&["Id"])
//!     .from("Account")
//!     .where_like("Name", "Test %");
//!
//! // 2. Mass delete all matching records!
//! let mass_op = SoqlMassOp::new(&client, query);
//! let stats = mass_op.delete_all().await?;
//!
//! println!("Processed: {}, Succeeded: {}, Failed: {}",
//!     stats.records_processed, stats.ops_succeeded, stats.ops_failed);
//! # Ok(())
//! # }
//! ```

use crate::api::SoqlQueryBuilder;
use crate::auth::Authenticator;
use crate::client::ForceClient;
use crate::error::{ForceError, Result};
use crate::types::DynamicSObject;
use serde_json::Value;

use super::query_batch::{BatchOp, BatchStats, QueryBatch};

/// Mass operations processor using SOQL and Composite Batch API.
#[derive(Debug)]
pub struct SoqlMassOp<'a, A: Authenticator> {
    client: &'a ForceClient<A>,
    query: SoqlQueryBuilder,
    halt_on_error: bool,
}

impl<'a, A: Authenticator> SoqlMassOp<'a, A> {
    /// Creates a new mass operations processor.
    ///
    /// # Arguments
    ///
    /// * `client` - The Force client.
    /// * `query` - The SOQL query builder used to find the target records.
    ///
    /// Note: The query MUST select the `Id` field, as it is required for update/delete operations.
    #[must_use]
    pub fn new(client: &'a ForceClient<A>, query: SoqlQueryBuilder) -> Self {
        Self {
            client,
            query,
            halt_on_error: false,
        }
    }

    /// Sets whether to stop processing if a batch operation fails.
    ///
    /// Default is false.
    #[must_use]
    pub fn halt_on_error(mut self, halt: bool) -> Self {
        self.halt_on_error = halt;
        self
    }

    /// Deletes all records found by the SOQL query using Composite Batch requests.
    ///
    /// This method fetches all records matching the query, automatically handling
    /// pagination, and sends them to Salesforce in batches of 25 (the Composite limit)
    /// to execute atomic DELETE operations.
    ///
    /// # Errors
    ///
    /// Returns an error if the SOQL query fails or if building/executing
    /// the batch encounters a structural error. Individual record failures do not
    /// throw an error unless `halt_on_error` is true; instead, they are tracked
    /// in the returned `BatchStats`.
    pub async fn delete_all(self) -> Result<BatchStats> {
        let qstr = self.query.try_build()?;

        let batch_op = QueryBatch::new(self.client, qstr).halt_on_error(self.halt_on_error);

        batch_op
            .run::<DynamicSObject, _>(|record| {
                // Ensure we have an Id field. If not, we skip the record.
                let id = record
                    .get_field_as::<String>("Id")
                    .ok()
                    .flatten()
                    .unwrap_or_default();

                if id.is_empty() {
                    None
                } else {
                    Some(BatchOp::Delete(record.object_type().to_string(), id))
                }
            })
            .await
    }

    /// Updates all records found by the SOQL query using Composite Batch requests.
    ///
    /// This method applies the provided JSON `updates` object to every record matching
    /// the query, automatically handling pagination, and sends them in batches of 25.
    ///
    /// # Arguments
    ///
    /// * `updates` - A JSON object containing the field updates (e.g. `json!({"Status": "Closed"})`).
    ///
    /// # Errors
    ///
    /// Returns an error if `updates` is not a valid JSON object, if the query fails,
    /// or if executing the batch hits an error.
    pub async fn update_all(self, updates: Value) -> Result<BatchStats> {
        if !updates.is_object() {
            return Err(ForceError::InvalidInput(
                "Updates must be a JSON object".to_string(),
            ));
        }

        let qstr = self.query.try_build()?;

        let batch_op = QueryBatch::new(self.client, qstr).halt_on_error(self.halt_on_error);

        batch_op
            .run::<DynamicSObject, _>(|record| {
                let id = record
                    .get_field_as::<String>("Id")
                    .ok()
                    .flatten()
                    .unwrap_or_default();

                if id.is_empty() {
                    None
                } else {
                    Some(BatchOp::Update(
                        record.object_type().to_string(),
                        id,
                        updates.clone(),
                    ))
                }
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::builder;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;
    use serde_json::json;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn create_mock_server() -> MockServer {
        MockServer::start().await
    }

    async fn create_test_client(mock_server: &MockServer) -> ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        builder().authenticate(auth).build().await.must()
    }

    #[tokio::test]
    async fn test_mass_delete_success() {
        let mock_server = create_mock_server().await;
        let client = create_test_client(&mock_server).await;

        // Mock the query response
        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/query"))
            .and(query_param("q", "SELECT Id FROM Account WHERE Name LIKE 'Test %'"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 2,
                "done": true,
                "records": [
                    { "attributes": { "type": "Account", "url": "/services/data/v67.0/sobjects/Account/001000000000001AAA" }, "Id": "001000000000001AAA" },
                    { "attributes": { "type": "Account", "url": "/services/data/v67.0/sobjects/Account/001000000000002AAA" }, "Id": "001000000000002AAA" }
                ]
            })))
            .mount(&mock_server)
            .await;

        // Mock the composite batch response
        Mock::given(method("POST"))
            .and(path("/services/data/v67.0/composite/batch"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "hasErrors": false,
                "results": [
                    { "statusCode": 204, "result": null },
                    { "statusCode": 204, "result": null }
                ]
            })))
            .mount(&mock_server)
            .await;

        let query = SoqlQueryBuilder::new()
            .select(&["Id"])
            .from("Account")
            .where_like("Name", "Test %");

        let op = SoqlMassOp::new(&client, query);
        let stats = op.delete_all().await.must();

        assert_eq!(stats.records_processed, 2);
        assert_eq!(stats.ops_succeeded, 2);
        assert_eq!(stats.ops_failed, 0);
    }

    #[tokio::test]
    async fn test_mass_update_success() {
        let mock_server = create_mock_server().await;
        let client = create_test_client(&mock_server).await;

        // Mock the query response
        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/query"))
            .and(query_param("q", "SELECT Id FROM Contact WHERE Status = 'New'"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 1,
                "done": true,
                "records": [
                    { "attributes": { "type": "Contact", "url": "/services/data/v67.0/sobjects/Contact/003000000000001AAA" }, "Id": "003000000000001AAA" }
                ]
            })))
            .mount(&mock_server)
            .await;

        // Mock the composite batch response
        Mock::given(method("POST"))
            .and(path("/services/data/v67.0/composite/batch"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "hasErrors": false,
                "results": [
                    { "statusCode": 204, "result": null }
                ]
            })))
            .mount(&mock_server)
            .await;

        let query = SoqlQueryBuilder::new()
            .select(&["Id"])
            .from("Contact")
            .where_eq("Status", "New");

        let op = SoqlMassOp::new(&client, query);

        let updates = json!({
            "Status": "Processed"
        });

        let stats = op.update_all(updates).await.must();

        assert_eq!(stats.records_processed, 1);
        assert_eq!(stats.ops_succeeded, 1);
        assert_eq!(stats.ops_failed, 0);
    }

    #[tokio::test]
    async fn test_delete_all_missing_id_is_skipped() {
        let mock_server = create_mock_server().await;
        let client = create_test_client(&mock_server).await;

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 1,
                "done": true,
                "records": [
                    { "attributes": { "type": "Account", "url": "/services/data/v67.0/sobjects/Account/001000000000001AAA" }, "Name": "Test" }
                ]
            })))
            .mount(&mock_server)
            .await;

        let query = SoqlQueryBuilder::new().select(&["Name"]).from("Account");
        let op = SoqlMassOp::new(&client, query);
        let stats = op.delete_all().await.must();
        assert_eq!(stats.ops_succeeded, 0);
        assert_eq!(stats.ops_failed, 0);
    }

    #[tokio::test]
    async fn test_update_all_missing_id_is_skipped() {
        let mock_server = create_mock_server().await;
        let client = create_test_client(&mock_server).await;

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 1,
                "done": true,
                "records": [
                    { "attributes": { "type": "Account", "url": "/services/data/v67.0/sobjects/Account/001000000000001AAA" }, "Name": "Test" }
                ]
            })))
            .mount(&mock_server)
            .await;

        let query = SoqlQueryBuilder::new().select(&["Name"]).from("Account");
        let op = SoqlMassOp::new(&client, query);
        let stats = op.update_all(json!({"Name": "Updated"})).await.must();
        assert_eq!(stats.ops_succeeded, 0);
        assert_eq!(stats.ops_failed, 0);
    }

    #[tokio::test]
    async fn test_update_all_invalid_updates_is_err() {
        let mock_server = create_mock_server().await;
        let client = create_test_client(&mock_server).await;
        let query = SoqlQueryBuilder::new().select(&["Id"]).from("Account");
        let op = SoqlMassOp::new(&client, query);
        let result = op.update_all(json!("not an object")).await;
        assert!(matches!(result, Err(ForceError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn test_delete_all_invalid_query_is_err() {
        let mock_server = create_mock_server().await;
        let client = create_test_client(&mock_server).await;
        let query = SoqlQueryBuilder::new().select(&["Id"]); // missing FROM
        let op = SoqlMassOp::new(&client, query);
        let result = op.delete_all().await;
        assert!(matches!(result, Err(ForceError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn test_update_all_invalid_query_is_err() {
        let mock_server = create_mock_server().await;
        let client = create_test_client(&mock_server).await;
        let query = SoqlQueryBuilder::new().select(&["Id"]); // missing FROM
        let op = SoqlMassOp::new(&client, query);
        let result = op.update_all(json!({"Name": "Updated"})).await;
        assert!(matches!(result, Err(ForceError::InvalidInput(_))));
    }
}
