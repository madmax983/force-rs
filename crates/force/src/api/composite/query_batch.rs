//! Batch Query Processing.
//!
//! This module provides a utility to query records and perform batch operations on them.
//! It handles pagination and batch execution automatically.

use crate::api::rest_operation::RestOperation;
use crate::auth::Authenticator;
use crate::error::Result;
use crate::session::Session;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;

/// Operations supported by the batch processor.
#[derive(Debug, Clone)]
pub enum BatchOp {
    /// Update a record (SObject, Id, Fields).
    Update(String, String, Value),
    /// Delete a record (SObject, Id).
    Delete(String, String),
    /// Create a new record (SObject, Fields).
    Create(String, Value),
}

/// Statistics for the batch processing job.
#[derive(Debug, Default, Clone, Copy)]
pub struct BatchStats {
    /// Number of records processed from the query.
    pub records_processed: usize,
    /// Number of batch operations executed successfully.
    pub ops_succeeded: usize,
    /// Number of batch operations failed.
    pub ops_failed: usize,
}

/// A processor that queries records and executes batch operations.
#[cfg(feature = "composite")]
pub struct QueryBatch<A: Authenticator> {
    session: Arc<Session<A>>,
    query: String,
    halt_on_error: bool,
}

#[cfg(feature = "composite")]
impl<A: Authenticator> QueryBatch<A> {
    /// Creates a new query batch processor.
    ///
    /// # Arguments
    ///
    /// * `session` - The Force client session.
    /// * `query` - The SOQL query to execute.
    pub fn new(session: Arc<Session<A>>, query: impl Into<String>) -> Self {
        Self {
            session,
            query: query.into(),
            halt_on_error: false,
        }
    }

    /// Sets whether to stop processing if a batch operation fails.
    #[must_use]
    pub fn halt_on_error(mut self, halt: bool) -> Self {
        self.halt_on_error = halt;
        self
    }

    /// Executes the query and processes records.
    ///
    /// # Arguments
    ///
    /// * `transform` - A closure that takes a record and returns an optional operation.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails or if batch execution fails.
    pub async fn run<T, F>(self, transform: F) -> Result<BatchStats>
    where
        T: DeserializeOwned,
        F: Fn(T) -> Option<BatchOp>,
    {
        let mut stats = BatchStats::default();
        let mut buffer = Vec::with_capacity(25);

        let rest = crate::api::rest::RestHandler::new(Arc::clone(&self.session));
        let mut result = rest.query::<T>(&self.query).await?;

        loop {
            let records = std::mem::take(&mut result.records);
            for record in records {
                stats.records_processed += 1;
                if let Some(op) = transform(record) {
                    buffer.push(op);
                    if buffer.len() >= 25 {
                        self.flush_batch(&mut buffer, &mut stats).await?;
                    }
                }
            }

            if result.is_done() {
                break;
            }

            if let Some(next_url) = result.next_records_url {
                result = rest.query_more(&next_url).await?;
            } else {
                break;
            }
        }

        // Flush remaining
        if !buffer.is_empty() {
            self.flush_batch(&mut buffer, &mut stats).await?;
        }

        Ok(stats)
    }

    async fn flush_batch(&self, ops: &mut Vec<BatchOp>, stats: &mut BatchStats) -> Result<()> {
        if ops.is_empty() {
            return Ok(());
        }

        let mut batch = crate::api::composite::CompositeHandler::new(Arc::clone(&self.session))
            .batch()
            .halt_on_error(self.halt_on_error);

        for op in ops.drain(..) {
            batch = match op {
                BatchOp::Update(sobject, id, fields) => batch.patch(&sobject, &id, fields)?,
                BatchOp::Delete(sobject, id) => batch.delete(&sobject, &id)?,
                BatchOp::Create(sobject, fields) => batch.post(&sobject, fields)?,
            };
        }

        let response = batch.execute().await?;

        for res in response.results {
            if (200..=299).contains(&res.status_code) {
                stats.ops_succeeded += 1;
            } else {
                stats.ops_failed += 1;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::ForceClient;
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
    async fn test_query_batch_pagination() {
        let mock_server = create_mock_server().await;
        let client: ForceClient<MockAuthenticator> = create_test_client(&mock_server).await;

        let mut page1_records = Vec::new();
        for i in 0..20 {
            page1_records.push(json!({
                "attributes": { "type": "Account", "url": format!("/services/data/v60.0/sobjects/Account/0010000000000{:02}AAA", i) },
                "Id": format!("0010000000000{:02}AAA", i)
            }));
        }

        let mut page2_records = Vec::new();
        for i in 20..30 {
            page2_records.push(json!({
                "attributes": { "type": "Account", "url": format!("/services/data/v60.0/sobjects/Account/0010000000000{:02}AAA", i) },
                "Id": format!("0010000000000{:02}AAA", i)
            }));
        }

        // Mock the first query response
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .and(query_param("q", "SELECT Id FROM Account"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 30,
                "done": false,
                "nextRecordsUrl": "/services/data/v60.0/query/01gD0000002HU6K",
                "records": page1_records
            })))
            .mount(&mock_server)
            .await;

        // Mock the next page query response
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query/01gD0000002HU6K"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 30,
                "done": true,
                "records": page2_records
            })))
            .mount(&mock_server)
            .await;

        let mut responses_page1 = Vec::new();
        for _ in 0..25 {
            responses_page1.push(json!({ "statusCode": 204, "result": null }));
        }

        let mut responses_page2 = Vec::new();
        for _ in 0..5 {
            responses_page2.push(json!({ "statusCode": 204, "result": null }));
        }

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/composite/batch"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "hasErrors": false,
                "results": responses_page2
            })))
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/composite/batch"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "hasErrors": false,
                "results": responses_page1
            })))
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;

        let query = "SELECT Id FROM Account";
        let query_batch = client.composite().query_batch(query);

        let stats = query_batch
            .run::<crate::types::DynamicSObject, _>(|record| {
                let id = record.get_field_as::<String>("Id").ok().flatten().must();
                Some(BatchOp::Delete(record.object_type().to_string(), id))
            })
            .await
            .must();

        println!("test stats: {:?}", stats);

        assert_eq!(stats.records_processed, 30);
        assert_eq!(stats.ops_succeeded, 30);
        assert_eq!(stats.ops_failed, 0);
    }

    #[tokio::test]
    async fn test_query_batch_partial_failures() {
        let mock_server = create_mock_server().await;
        let client: ForceClient<MockAuthenticator> = create_test_client(&mock_server).await;

        let mut records = Vec::new();
        for i in 0..10 {
            records.push(json!({
                "attributes": { "type": "Contact", "url": format!("/services/data/v60.0/sobjects/Contact/0030000000000{:02}AAA", i) },
                "Id": format!("0030000000000{:02}AAA", i)
            }));
        }

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .and(query_param("q", "SELECT Id FROM Contact"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 10,
                "done": true,
                "records": records
            })))
            .mount(&mock_server)
            .await;

        let mut results = Vec::new();
        for i in 0..10 {
            if i % 2 == 0 {
                // Succeeded
                results.push(json!({ "statusCode": 204, "result": null }));
            } else {
                // Failed
                results.push(json!({ "statusCode": 400, "result": [{"message": "Bad Request", "errorCode": "INVALID_FIELD"}] }));
            }
        }

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/composite/batch"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "hasErrors": true,
                "results": results
            })))
            .mount(&mock_server)
            .await;

        let query = "SELECT Id FROM Contact";
        let query_batch = client.composite().query_batch(query);

        let stats = query_batch
            .run::<crate::types::DynamicSObject, _>(|record| {
                let id = record.get_field_as::<String>("Id").ok().flatten().must();
                Some(BatchOp::Delete(record.object_type().to_string(), id))
            })
            .await
            .must();

        assert_eq!(stats.records_processed, 10);
        assert_eq!(stats.ops_succeeded, 5);
        assert_eq!(stats.ops_failed, 5);
    }
}
