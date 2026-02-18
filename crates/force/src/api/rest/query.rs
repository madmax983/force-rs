//! SOQL query execution.
//!
//! This module provides typed and dynamic SOQL query execution.
//! For large result sets, use the `nextRecordsUrl` field from `QueryResult`
//! to manually fetch additional pages.

use crate::error::ForceError;
use crate::types::QueryResult;
use serde::de::DeserializeOwned;

impl<A: crate::auth::Authenticator> super::RestHandler<A> {
    /// Executes a SOQL query and returns the first page of results.
    ///
    /// This method performs a single query and returns only the first page.
    /// Use `query_more()` to fetch additional pages using the `nextRecordsUrl`.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The query is malformed
    /// - Network/authentication failures occur
    /// - Deserialization fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use force::types::DynamicSObject;
    ///
    /// let result = client.rest().query::<DynamicSObject>("SELECT Id, Name FROM Account LIMIT 10").await?;
    /// println!("Total: {}", result.total_size);
    /// for record in result.records {
    ///     println!("{:?}", record);
    /// }
    /// ```
    pub async fn query<T>(&self, soql: &str) -> Result<QueryResult<T>, ForceError>
    where
        T: DeserializeOwned,
    {
        // Construct query URL
        let url = format!("{}/query", self.base_url().await?);

        // Execute query
        let request = self
            .inner
            .http_client
            .get(&url)
            .query(&[("q", soql)])
            .build()
            .map_err(crate::error::HttpError::from)?;

        self.inner
            .send_request_and_decode(request, "SOQL query failed")
            .await
    }

    /// Fetches the next page of query results using a `nextRecordsUrl`.
    ///
    /// When a query returns `done: false`, use the `nextRecordsUrl` from the
    /// previous result to fetch the next page.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The locator URL is invalid
    /// - Network/authentication failures occur
    /// - Deserialization fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use force::types::DynamicSObject;
    ///
    /// let mut result = client.rest().query::<DynamicSObject>("SELECT Id, Name FROM Account").await?;
    /// while !result.is_done() {
    ///     if let Some(next_url) = result.next_records_url.as_ref() {
    ///         result = client.rest().query_more(next_url).await?;
    ///         // Process result.records...
    ///     }
    /// }
    /// ```
    pub async fn query_more<T>(&self, next_records_url: &str) -> Result<QueryResult<T>, ForceError>
    where
        T: DeserializeOwned,
    {
        // Construct full URL (next_records_url might be absolute or relative)
        let url = if next_records_url.starts_with("http") {
            next_records_url.to_string()
        } else {
            let token = self.inner.token_manager.token().await?;
            format!("{}{}", token.instance_url(), next_records_url)
        };

        // Execute query
        let request = self
            .inner
            .http_client
            .get(&url)
            .build()
            .map_err(crate::error::HttpError::from)?;

        self.inner
            .send_request_and_decode(request, "Query pagination failed")
            .await
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::builder;
    use crate::test_support::{MockAuthenticator, Must};
    use crate::types::DynamicSObject;
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // Test SObject for typed queries
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestAccount {
        #[serde(rename = "Id")]
        id: String,
        #[serde(rename = "Name")]
        name: String,
    }

    // RED PHASE - Write failing tests first
    // Note: These are compilation tests to ensure types are correct.
    // Integration tests will be added once the client builder is stabilized.

    #[test]
    fn test_query_result_deserialization_typed() {
        // Test that QueryResult can deserialize typed records
        let json = serde_json::json!({
            "totalSize": 2,
            "done": true,
            "records": [
                {"Id": "001", "Name": "Acme"},
                {"Id": "002", "Name": "Globex"}
            ]
        });

        let result: QueryResult<TestAccount> = serde_json::from_value(json).must();

        assert_eq!(result.total_size, 2);
        assert!(result.is_done());
        assert_eq!(result.len(), 2);
        assert_eq!(result.records[0].name, "Acme");
        assert_eq!(result.records[1].name, "Globex");
    }

    #[test]
    fn test_query_result_deserialization_dynamic() {
        // Test that QueryResult can deserialize DynamicSObject
        let json = serde_json::json!({
            "totalSize": 1,
            "done": true,
            "records": [{
                "attributes": {
                    "type": "Account",
                    "url": "/services/data/v60.0/sobjects/Account/001"
                },
                "Id": "001",
                "Name": "Acme"
            }]
        });

        let result: QueryResult<DynamicSObject> = serde_json::from_value(json).must();

        assert_eq!(result.total_size, 1);
        assert_eq!(result.records[0].object_type(), "Account");
        assert_eq!(
            result.records[0]
                .get_field("Name")
                .and_then(|v: &serde_json::Value| v.as_str()),
            Some("Acme")
        );
    }

    #[test]
    fn test_query_result_with_pagination() {
        // Test pagination metadata
        let json = serde_json::json!({
            "totalSize": 4,
            "done": false,
            "nextRecordsUrl": "/services/data/v60.0/query/01g-2000",
            "records": [
                {"Id": "001", "Name": "Acme"},
                {"Id": "002", "Name": "Globex"}
            ]
        });

        let result: QueryResult<TestAccount> = serde_json::from_value(json).must();

        assert_eq!(result.total_size, 4);
        assert!(!result.is_done());
        assert!(result.has_more());
        assert_eq!(
            result.next_records_url,
            Some("/services/data/v60.0/query/01g-2000".to_string())
        );
    }

    #[test]
    fn test_query_result_empty() {
        // Test empty result set
        let json = serde_json::json!({
            "totalSize": 0,
            "done": true,
            "records": []
        });

        let result: QueryResult<TestAccount> = serde_json::from_value(json).must();

        assert_eq!(result.total_size, 0);
        assert!(result.is_done());
        assert!(result.is_empty());
    }

    #[test]
    fn test_query_result_with_nested_query() {
        // Test complex SOQL with subqueries
        let json = serde_json::json!({
            "totalSize": 1,
            "done": true,
            "records": [{
                "Id": "001",
                "Name": "Tech Corp",
                "Contacts": {
                    "totalSize": 0,
                    "done": true,
                    "records": []
                }
            }]
        });

        let result: QueryResult<serde_json::Value> = serde_json::from_value(json).must();
        assert_eq!(result.total_size, 1);
    }

    #[test]
    fn test_query_result_pagination_workflow() {
        // Test that pagination metadata is correctly available
        let json = serde_json::json!({
            "totalSize": 100,
            "done": false,
            "nextRecordsUrl": "/services/data/v60.0/query/01g-batch2",
            "records": [
                {"Id": "001", "Name": "First"},
                {"Id": "002", "Name": "Second"}
            ]
        });

        let result: QueryResult<TestAccount> = serde_json::from_value(json).must();

        // Verify pagination state
        assert!(!result.is_done());
        assert!(result.has_more());
        assert!(result.next_records_url.is_some());

        // The next_records_url can be used with query_more()
        let next_url = result.next_records_url.must();
        assert!(next_url.starts_with("/services/data"));
    }

    // Integration tests with wiremock

    #[tokio::test]
    async fn test_query_success() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        // Mock the query endpoint
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .and(query_param("q", "SELECT Id, Name FROM Account LIMIT 2"))
            .and(header("authorization", "Bearer test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 2,
                "done": true,
                "records": [
                    {"Id": "001xx0000000001", "Name": "Acme Corp"},
                    {"Id": "001xx0000000002", "Name": "Globex Inc"}
                ]
            })))
            .mount(&mock_server)
            .await;

        let client = builder().authenticate(auth).build().await.must();

        let result: QueryResult<TestAccount> = client
            .rest()
            .query("SELECT Id, Name FROM Account LIMIT 2")
            .await
            .must();

        assert_eq!(result.total_size, 2);
        assert!(result.is_done());
        assert_eq!(result.len(), 2);
        assert_eq!(result.records[0].name, "Acme Corp");
        assert_eq!(result.records[1].name, "Globex Inc");
    }

    #[tokio::test]
    async fn test_query_pagination_missing_url() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        // Mock response with done: false but no nextRecordsUrl
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 10,
                "done": false,
                // nextRecordsUrl is missing!
                "records": [
                    {"Id": "001", "Name": "Record1"}
                ]
            })))
            .mount(&mock_server)
            .await;

        let client = builder().authenticate(auth).build().await.must();

        let result: QueryResult<TestAccount> = client
            .rest()
            .query("SELECT Id, Name FROM Account")
            .await
            .must();

        // Verify that the client deserializes it as is
        assert!(!result.is_done());
        assert!(result.has_more());
        assert!(result.next_records_url.is_none());

        // This confirms that the client passes the invalid state to the user,
        // who will then likely panic if they try to unwrap next_records_url.
    }

    #[tokio::test]
    async fn test_query_with_pagination() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        // Mock first page
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .and(query_param("q", "SELECT Id, Name FROM Account"))
            .and(header("authorization", "Bearer test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 4,
                "done": false,
                "nextRecordsUrl": "/services/data/v60.0/query/01gxx-batch2",
                "records": [
                    {"Id": "001xx0000000001", "Name": "Page1 Record1"},
                    {"Id": "001xx0000000002", "Name": "Page1 Record2"}
                ]
            })))
            .mount(&mock_server)
            .await;

        let client = builder().authenticate(auth).build().await.must();

        let result: QueryResult<TestAccount> = client
            .rest()
            .query("SELECT Id, Name FROM Account")
            .await
            .must();

        assert_eq!(result.total_size, 4);
        assert!(!result.is_done());
        assert!(result.has_more());
        assert_eq!(result.len(), 2);
        assert_eq!(
            result.next_records_url,
            Some("/services/data/v60.0/query/01gxx-batch2".to_string())
        );
    }

    #[tokio::test]
    async fn test_query_more_success() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        // Mock first page
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .and(query_param("q", "SELECT Id, Name FROM Account"))
            .and(header("authorization", "Bearer test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 4,
                "done": false,
                "nextRecordsUrl": "/services/data/v60.0/query/01gxx-batch2",
                "records": [
                    {"Id": "001xx0000000001", "Name": "Batch1 Rec1"},
                    {"Id": "001xx0000000002", "Name": "Batch1 Rec2"}
                ]
            })))
            .mount(&mock_server)
            .await;

        // Mock second page (query_more)
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query/01gxx-batch2"))
            .and(header("authorization", "Bearer test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 4,
                "done": true,
                "records": [
                    {"Id": "001xx0000000003", "Name": "Batch2 Rec1"},
                    {"Id": "001xx0000000004", "Name": "Batch2 Rec2"}
                ]
            })))
            .mount(&mock_server)
            .await;

        let client = builder().authenticate(auth).build().await.must();

        // First page
        let page1: QueryResult<TestAccount> = client
            .rest()
            .query("SELECT Id, Name FROM Account")
            .await
            .must();

        assert!(!page1.is_done());
        assert_eq!(page1.len(), 2);
        assert_eq!(page1.records[0].name, "Batch1 Rec1");

        // Second page using query_more
        let page2: QueryResult<TestAccount> = client
            .rest()
            .query_more(page1.next_records_url.as_ref().must())
            .await
            .must();

        assert!(page2.is_done());
        assert_eq!(page2.len(), 2);
        assert_eq!(page2.records[0].name, "Batch2 Rec1");
        assert_eq!(page2.records[1].name, "Batch2 Rec2");
    }

    #[tokio::test]
    async fn test_query_more_full_pagination_loop() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        // Mock first page
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 6,
                "done": false,
                "nextRecordsUrl": "/services/data/v60.0/query/batch2",
                "records": [
                    {"Id": "001xx0000000001", "Name": "Record1"},
                    {"Id": "001xx0000000002", "Name": "Record2"}
                ]
            })))
            .mount(&mock_server)
            .await;

        // Mock second page
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query/batch2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 6,
                "done": false,
                "nextRecordsUrl": "/services/data/v60.0/query/batch3",
                "records": [
                    {"Id": "001xx0000000003", "Name": "Record3"},
                    {"Id": "001xx0000000004", "Name": "Record4"}
                ]
            })))
            .mount(&mock_server)
            .await;

        // Mock third (final) page
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query/batch3"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 6,
                "done": true,
                "records": [
                    {"Id": "001xx0000000005", "Name": "Record5"},
                    {"Id": "001xx0000000006", "Name": "Record6"}
                ]
            })))
            .mount(&mock_server)
            .await;

        let client = builder().authenticate(auth).build().await.must();

        // Collect all records by manually paginating
        let mut all_records = Vec::new();
        let mut result: QueryResult<TestAccount> = client
            .rest()
            .query("SELECT Id, Name FROM Account")
            .await
            .must();

        all_records.extend(result.records.clone());

        while !result.is_done() {
            if let Some(next_url) = result.next_records_url.as_ref() {
                result = client.rest().query_more(next_url).await.must();
                all_records.extend(result.records.clone());
            } else {
                break;
            }
        }

        // Verify we got all 6 records
        assert_eq!(all_records.len(), 6);
        assert_eq!(all_records[0].name, "Record1");
        assert_eq!(all_records[5].name, "Record6");
    }

    #[tokio::test]
    async fn test_query_more_http_error() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        // Mock query_more with 404 error (invalid locator)
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query/invalid-locator"))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!([{
                "errorCode": "INVALID_QUERY_LOCATOR",
                "message": "Unable to find query cursor"
            }])))
            .mount(&mock_server)
            .await;

        let client = builder().authenticate(auth).build().await.must();

        let result: Result<QueryResult<TestAccount>, _> = client
            .rest()
            .query_more("/services/data/v60.0/query/invalid-locator")
            .await;

        let Err(err) = result else {
            panic!("expected query_more to fail for invalid locator");
        };
        assert!(
            err.to_string().contains("404") || err.to_string().contains("Query pagination failed")
        );
    }

    #[tokio::test]
    async fn test_query_empty_result() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 0,
                "done": true,
                "records": []
            })))
            .mount(&mock_server)
            .await;

        let client = builder().authenticate(auth).build().await.must();

        let result: QueryResult<TestAccount> = client
            .rest()
            .query("SELECT Id, Name FROM Account WHERE Name = 'NonExistent'")
            .await
            .must();

        assert_eq!(result.total_size, 0);
        assert!(result.is_done());
        assert!(result.is_empty());
        assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    async fn test_query_more_absolute_url() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        // Mock first page
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 4,
                "done": false,
                // Absolute URL pointing back to the mock server
                "nextRecordsUrl": format!("{}/services/data/v60.0/query/batch2", mock_server.uri()),
                "records": [
                    {"Id": "001xx0000000001", "Name": "Record1"}
                ]
            })))
            .mount(&mock_server)
            .await;

        // Mock second page
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query/batch2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 4,
                "done": true,
                "records": [
                    {"Id": "001xx0000000002", "Name": "Record2"}
                ]
            })))
            .mount(&mock_server)
            .await;

        let client = builder().authenticate(auth).build().await.must();

        let page1: QueryResult<TestAccount> = client
            .rest()
            .query("SELECT Id, Name FROM Account")
            .await
            .must();

        let next_url = page1.next_records_url.as_ref().must();
        assert!(next_url.starts_with("http")); // Verify it is absolute

        let page2: QueryResult<TestAccount> = client.rest().query_more(next_url).await.must();

        assert_eq!(page2.len(), 1);
        assert_eq!(page2.records[0].name, "Record2");
    }
}
