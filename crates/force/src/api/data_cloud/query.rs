//! Data Cloud SQL Query API.
//!
//! Provides access to the Data Cloud SQL query endpoint at
//! `POST /services/data/vXX.0/ssot/query-sql`.

use super::types::{ColumnMetadata, DataCloudRecord, SqlQueryRequest};
use crate::error::Result;
use serde::Deserialize;
use std::collections::HashMap;

/// Response from the Data Cloud SQL query endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct SqlQueryResponse {
    /// The query result records.
    #[serde(default)]
    pub data: Vec<DataCloudRecord>,

    /// Column metadata describing the result schema.
    #[serde(default)]
    pub metadata: Vec<ColumnMetadata>,

    /// Cursor for pagination (present when more results are available).
    #[serde(default, alias = "nextBatchId")]
    pub next_batch_id: Option<String>,

    /// Number of rows returned.
    #[serde(default, alias = "rowCount")]
    pub row_count: Option<usize>,

    /// Additional response fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl<A: crate::auth::Authenticator> super::DataCloudHandler<A> {
    /// Executes a SQL query against Data Cloud.
    ///
    /// # Arguments
    ///
    /// * `sql` — The SQL query string.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let dc = client.data_cloud()?;
    /// let result = dc.query_sql("SELECT Id, Name__c FROM UnifiedProfile__dlm LIMIT 10").await?;
    /// for record in &result.data {
    ///     println!("{:?}", record);
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails or the response cannot be parsed.
    pub async fn query_sql(&self, sql: &str) -> Result<SqlQueryResponse> {
        let request = SqlQueryRequest::new(sql);
        self.post("query", &request, "Data Cloud SQL query failed")
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;

    #[test]
    fn test_sql_query_response_deserialization() {
        let json = r#"{
            "data": [
                {"Id": "abc123", "Name__c": "Test Profile"},
                {"Id": "def456", "Name__c": "Another Profile"}
            ],
            "metadata": [
                {"columnName": "Id", "type": "VARCHAR"},
                {"columnName": "Name__c", "type": "VARCHAR"}
            ],
            "rowCount": 2
        }"#;

        let response: SqlQueryResponse = serde_json::from_str(json).must();
        assert_eq!(response.data.len(), 2);
        assert_eq!(response.metadata.len(), 2);
        assert_eq!(response.row_count, Some(2));
        assert!(response.next_batch_id.is_none());
    }

    #[test]
    fn test_sql_query_response_with_pagination() {
        let json = r#"{
            "data": [{"Id": "abc"}],
            "metadata": [],
            "nextBatchId": "cursor_abc123",
            "rowCount": 1
        }"#;

        let response: SqlQueryResponse = serde_json::from_str(json).must();
        assert_eq!(response.next_batch_id.as_deref(), Some("cursor_abc123"));
    }

    #[test]
    fn test_sql_query_response_empty() {
        let json = r#"{
            "data": [],
            "metadata": [],
            "rowCount": 0
        }"#;

        let response: SqlQueryResponse = serde_json::from_str(json).must();
        assert!(response.data.is_empty());
        assert_eq!(response.row_count, Some(0));
    }

    #[test]
    fn test_sql_query_response_with_extra_fields() {
        let json = r#"{
            "data": [],
            "metadata": [],
            "rowCount": 0,
            "queryId": "q-12345",
            "done": true
        }"#;

        let response: SqlQueryResponse = serde_json::from_str(json).must();
        assert!(response.extra.contains_key("queryId"));
        assert!(response.extra.contains_key("done"));
    }

    #[cfg(feature = "mock")]
    mod integration {
        use crate::auth::DataCloudConfig;
        use crate::client::builder;
        use crate::test_utils::mock_auth::MockAuthenticator;
        use crate::test_utils::must::Must;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        /// Helper: build a client with DC pointed at the mock server.
        async fn dc_client_for(
            mock_server: &MockServer,
        ) -> crate::client::ForceClient<MockAuthenticator> {
            // Mock the token exchange endpoint
            let dc_token_response = serde_json::json!({
                "access_token": "dc_token_for_query",
                "instance_url": mock_server.uri(),
                "token_type": "Bearer",
                "expires_in": 7200
            });

            Mock::given(method("POST"))
                .and(path("/services/a360/token"))
                .respond_with(ResponseTemplate::new(200).set_body_json(dc_token_response))
                .mount(mock_server)
                .await;

            let auth = MockAuthenticator::new("platform_token", &mock_server.uri());
            builder()
                .authenticate(auth)
                .with_data_cloud(DataCloudConfig {
                    token_exchange_url: Some(format!("{}/services/a360/token", mock_server.uri())),
                    ..Default::default()
                })
                .build()
                .await
                .must()
        }

        #[tokio::test]
        async fn test_query_sql_success() {
            let mock_server = MockServer::start().await;

            let query_response = serde_json::json!({
                "data": [
                    {"Id": "rec1", "Name__c": "Alice"},
                    {"Id": "rec2", "Name__c": "Bob"}
                ],
                "metadata": [
                    {"columnName": "Id", "type": "VARCHAR"},
                    {"columnName": "Name__c", "type": "VARCHAR"}
                ],
                "rowCount": 2
            });

            Mock::given(method("POST"))
                .and(path("/services/data/v67.0/ssot/query"))
                .respond_with(ResponseTemplate::new(200).set_body_json(query_response))
                .expect(1)
                .mount(&mock_server)
                .await;

            let client = dc_client_for(&mock_server).await;
            let dc = client.data_cloud().must();
            let result = dc
                .query_sql("SELECT Id, Name__c FROM UnifiedProfile__dlm")
                .await
                .must();

            assert_eq!(result.data.len(), 2);
            assert_eq!(result.row_count, Some(2));

            let first = &result.data[0];
            assert_eq!(first.get("Id").and_then(|v| v.as_str()), Some("rec1"));
            assert_eq!(first.get("Name__c").and_then(|v| v.as_str()), Some("Alice"));
        }

        #[tokio::test]
        async fn test_query_sql_empty_result() {
            let mock_server = MockServer::start().await;

            let query_response = serde_json::json!({
                "data": [],
                "metadata": [],
                "rowCount": 0
            });

            Mock::given(method("POST"))
                .and(path("/services/data/v67.0/ssot/query"))
                .respond_with(ResponseTemplate::new(200).set_body_json(query_response))
                .mount(&mock_server)
                .await;

            let client = dc_client_for(&mock_server).await;
            let dc = client.data_cloud().must();
            let result = dc.query_sql("SELECT Id FROM Empty__dlm").await.must();

            assert!(result.data.is_empty());
            assert_eq!(result.row_count, Some(0));
        }

        #[tokio::test]
        async fn test_query_sql_error_response() {
            let mock_server = MockServer::start().await;

            Mock::given(method("POST"))
                .and(path("/services/data/v67.0/ssot/query"))
                .respond_with(
                    ResponseTemplate::new(400).set_body_string(
                        r#"[{"message":"Invalid SQL","errorCode":"INVALID_QUERY"}]"#,
                    ),
                )
                .mount(&mock_server)
                .await;

            let client = dc_client_for(&mock_server).await;
            let dc = client.data_cloud().must();
            let result = dc.query_sql("INVALID SQL").await;

            assert!(result.is_err());
        }

        #[tokio::test]
        async fn test_query_sql_with_pagination() {
            let mock_server = MockServer::start().await;

            let query_response = serde_json::json!({
                "data": [{"Id": "rec1"}],
                "metadata": [{"columnName": "Id", "type": "VARCHAR"}],
                "nextBatchId": "batch_cursor_xyz",
                "rowCount": 1
            });

            Mock::given(method("POST"))
                .and(path("/services/data/v67.0/ssot/query"))
                .respond_with(ResponseTemplate::new(200).set_body_json(query_response))
                .mount(&mock_server)
                .await;

            let client = dc_client_for(&mock_server).await;
            let dc = client.data_cloud().must();
            let result = dc.query_sql("SELECT Id FROM Big__dlm").await.must();

            assert_eq!(result.next_batch_id.as_deref(), Some("batch_cursor_xyz"));
        }
    }
}
