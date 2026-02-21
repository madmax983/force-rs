//! Composite Batch API.
//!
//! A batch request is a collection of up to 25 subrequests that you execute
//! in a single call. Each subrequest is independent and can be of a different
//! type (GET, POST, PATCH, DELETE).

use super::CompositeHandler;
use crate::api::rest::SoqlQueryBuilder;
use crate::auth::Authenticator;
use crate::error::{ForceError, Result};
use crate::types::{SalesforceId, validator};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Helper to validate SObject names.
fn validate_sobject_name_or_panic(sobject: &str) {
    if let Err(e) = validator::validate_sobject_name(sobject) {
        panic!("{}", e);
    }
}

/// Helper to validate Salesforce IDs.
fn validate_id_or_panic(id: &str) {
    if let Err(e) = SalesforceId::new(id) {
        panic!("Salesforce ID contains invalid characters: {}", e);
    }
}

/// Builder for constructing a Composite Batch request.
///
/// Use this builder to add up to 25 subrequests and execute them atomically.
#[derive(Debug)]
pub struct BatchBuilder<A: Authenticator> {
    handler: CompositeHandler<A>,
    requests: Vec<BatchSubRequest>,
    halt_on_error: bool,
}

impl<A: Authenticator> BatchBuilder<A> {
    pub(crate) fn new(handler: CompositeHandler<A>) -> Self {
        Self {
            handler,
            requests: Vec::new(),
            halt_on_error: false,
        }
    }

    /// sets whether the entire batch should stop processing if a subrequest fails.
    ///
    /// If true, subsequent requests in the batch will not be executed.
    /// Default is false.
    #[must_use]
    pub fn halt_on_error(mut self, halt: bool) -> Self {
        self.halt_on_error = halt;
        self
    }

    /// Adds a GET request to the batch.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The SObject type (e.g., "Account")
    /// * `id` - The record ID
    ///
    /// # Panics
    ///
    /// Panics if the `sobject` name or `id` contains invalid characters.
    #[must_use]
    pub fn get(self, sobject: &str, id: &str) -> Self {
        validate_sobject_name_or_panic(sobject);
        validate_id_or_panic(id);
        self.add_request("GET", &format!("sobjects/{}/{}", sobject, id), None)
    }

    /// Adds a POST (Create) request to the batch.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The SObject type (e.g., "Account")
    /// * `body` - The JSON body of the record
    ///
    /// # Panics
    ///
    /// Panics if the `sobject` name contains invalid characters.
    #[must_use]
    pub fn post(self, sobject: &str, body: Value) -> Self {
        validate_sobject_name_or_panic(sobject);
        self.add_request("POST", &format!("sobjects/{}", sobject), Some(body))
    }

    /// Adds a PATCH (Update) request to the batch.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The SObject type (e.g., "Account")
    /// * `id` - The record ID
    /// * `body` - The JSON body with fields to update
    ///
    /// # Panics
    ///
    /// Panics if the `sobject` name or `id` contains invalid characters.
    #[must_use]
    pub fn patch(self, sobject: &str, id: &str, body: Value) -> Self {
        validate_sobject_name_or_panic(sobject);
        validate_id_or_panic(id);
        self.add_request("PATCH", &format!("sobjects/{}/{}", sobject, id), Some(body))
    }

    /// Adds a DELETE request to the batch.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The SObject type (e.g., "Account")
    /// * `id` - The record ID
    ///
    /// # Panics
    ///
    /// Panics if the `sobject` name or `id` contains invalid characters.
    #[must_use]
    pub fn delete(self, sobject: &str, id: &str) -> Self {
        validate_sobject_name_or_panic(sobject);
        validate_id_or_panic(id);
        self.add_request("DELETE", &format!("sobjects/{}/{}", sobject, id), None)
    }

    /// Adds a custom subrequest to the batch.
    ///
    /// Use this for requests that don't fit the standard CRUD patterns,
    /// such as queries or parameterized searches.
    ///
    /// # Warning
    ///
    /// The `url` parameter must be properly URL-encoded, especially for query parameters.
    /// For SOQL queries, use [`query`](Self::query) instead, which handles encoding safely.
    ///
    /// # Arguments
    ///
    /// * `method` - HTTP method (GET, POST, etc.)
    /// * `url` - Relative URL (e.g., "query?q=Select+Id+From+Account")
    /// * `body` - Optional JSON body
    #[must_use]
    pub fn add_request(mut self, method: &str, url: &str, body: Option<Value>) -> Self {
        self.requests.push(BatchSubRequest {
            method: method.to_string(),
            url: url.to_string(),
            rich_input: body,
        });
        self
    }

    /// Adds a SOQL query request to the batch.
    ///
    /// This method automatically URL-encodes the query string to prevent injection vulnerabilities
    /// and ensures valid URL formatting.
    ///
    /// # Arguments
    ///
    /// * `query_builder` - The SOQL query builder
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let query = SoqlQueryBuilder::new()
    ///     .select(&["Id", "Name"])
    ///     .from("Account")
    ///     .where_eq("Name", "Acme Corp");
    ///
    /// let batch = client.composite().batch()
    ///     .query(query)
    ///     .execute()
    ///     .await?;
    /// ```
    #[must_use]
    pub fn query(mut self, query_builder: SoqlQueryBuilder) -> Self {
        let query_string = query_builder.build();
        // Use form_urlencoded for correct query param encoding
        let encoded: String = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("q", &query_string)
            .finish();

        self.requests.push(BatchSubRequest {
            method: "GET".to_string(),
            url: format!("query?{}", encoded),
            rich_input: None,
        });
        self
    }

    /// Executes the batch request.
    ///
    /// Sends all accumulated subrequests to the Salesforce Composite API.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The batch size exceeds 25
    /// - The response cannot be deserialized
    pub async fn execute(self) -> Result<BatchResponse> {
        if self.requests.is_empty() {
            return Err(ForceError::Serialization(
                crate::error::SerializationError::InvalidFormat(
                    "Batch cannot be empty".to_string(),
                ),
            ));
        }
        if self.requests.len() > 25 {
            return Err(ForceError::Serialization(
                crate::error::SerializationError::InvalidFormat(
                    "Batch size exceeds limit of 25 requests".to_string(),
                ),
            ));
        }

        // Construct the composite batch URL
        // It must be absolute for the HTTP client
        let base_url = self.handler.base_url().await?;
        let url = format!("{}/composite/batch", base_url);

        let request_body = BatchRequest {
            batch_requests: self.requests,
            halt_on_error: self.halt_on_error,
        };

        let request = self
            .handler
            .inner
            .http_client
            .post(&url)
            .json(&request_body)
            .build()
            .map_err(crate::error::HttpError::from)?;

        self.handler
            .inner
            .send_request_and_decode(request, "Composite Batch failed")
            .await
    }
}

/// A request to the Composite Batch API.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchRequest {
    batch_requests: Vec<BatchSubRequest>,
    halt_on_error: bool,
}

/// A single subrequest within a batch.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchSubRequest {
    method: String,
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    rich_input: Option<Value>,
}

/// The response from a Composite Batch API call.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchResponse {
    /// True if any subrequest failed.
    pub has_errors: bool,
    /// The results of each subrequest, in order.
    pub results: Vec<BatchSubResponse>,
}

/// The result of a single subrequest.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSubResponse {
    /// The HTTP status code of the subrequest.
    pub status_code: u16,
    /// The JSON result (if any).
    /// For success, this is the response body.
    /// For error, this contains the error details.
    pub result: Option<Value>,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::test_support::Must;

    // Unit tests for serialization logic

    #[test]
    fn test_batch_request_serialization() {
        let req = BatchRequest {
            halt_on_error: true,
            batch_requests: vec![
                BatchSubRequest {
                    method: "GET".to_string(),
                    url: "sobjects/Account/001".to_string(),
                    rich_input: None,
                },
                BatchSubRequest {
                    method: "POST".to_string(),
                    url: "sobjects/Contact".to_string(),
                    rich_input: Some(serde_json::json!({"LastName": "Doe"})),
                },
            ],
        };

        let json = serde_json::to_string(&req).must();
        assert!(json.contains("\"haltOnError\":true"));
        assert!(json.contains("\"method\":\"GET\""));
        assert!(json.contains("\"url\":\"sobjects/Account/001\""));
        assert!(json.contains("\"richInput\":{\"LastName\":\"Doe\"}"));
    }

    #[test]
    fn test_batch_response_deserialization() {
        let json = r#"{
            "hasErrors": false,
            "results": [
                {
                    "statusCode": 200,
                    "result": {"id": "001..."}
                },
                {
                    "statusCode": 201,
                    "result": {"id": "003..."}
                }
            ]
        }"#;

        let resp: BatchResponse = serde_json::from_str(json).must();
        assert!(!resp.has_errors);
        assert_eq!(resp.results.len(), 2);
        assert_eq!(resp.results[0].status_code, 200);
    }

    use crate::client::builder as client_builder;
    use crate::test_support::MockAuthenticator;

    async fn create_builder() -> BatchBuilder<MockAuthenticator> {
        let auth = MockAuthenticator::new("token", "https://test.salesforce.com");
        let client = client_builder()
            .authenticate(auth)
            .build()
            .await
            .expect("failed to build client");

        client.composite().batch()
    }

    #[tokio::test]
    #[should_panic(expected = "SObject name contains invalid characters")]
    async fn test_batch_validation_sobject_invalid() {
        let builder = create_builder().await;
        let _ = builder.get("Invalid;Name", "001000000000000");
    }

    #[tokio::test]
    #[should_panic(expected = "SObject name cannot be empty")]
    async fn test_batch_validation_sobject_empty() {
        let builder = create_builder().await;
        let _ = builder.get("", "001000000000000");
    }

    #[tokio::test]
    #[should_panic(expected = "Salesforce ID contains invalid characters")]
    async fn test_batch_validation_id_invalid() {
        let builder = create_builder().await;
        let _ = builder.get("Account", "Invalid;ID");
    }

    #[tokio::test]
    async fn test_batch_execute_empty() {
        let builder = create_builder().await;
        let result = builder.execute().await;
        match result {
            Err(ForceError::Serialization(e)) => {
                assert!(e.to_string().contains("Batch cannot be empty"));
            }
            _ => panic!(
                "Expected Serialization error for empty batch, got {:?}",
                result
            ),
        }
    }

    #[tokio::test]
    async fn test_batch_size_limit() {
        let mut builder = create_builder().await;
        // Add 26 requests
        for i in 0..26 {
            // Use different IDs to avoid any potential deduplication (though not expected here)
            builder = builder.get("Account", &format!("001000000000{:03}AAA", i));
        }

        let result = builder.execute().await;
        match result {
            Err(ForceError::Serialization(e)) => {
                assert!(e.to_string().contains("Batch size exceeds limit"));
            }
            _ => panic!(
                "Expected Serialization error for batch size limit, got {:?}",
                result
            ),
        }
    }

    #[tokio::test]
    async fn test_batch_size_limit_boundary_25() {
        let mut builder = create_builder().await;
        // Add 25 requests
        for i in 0..25 {
            builder = builder.get("Account", &format!("001000000000{:03}AAA", i));
        }

        let result = builder.execute().await;
        // Should NOT be serialization error about size
        if let Err(ForceError::Serialization(e)) = &result {
            assert!(
                !e.to_string().contains("Batch size exceeds limit"),
                "Batch size limit triggered for 25 requests (should allow up to 25)"
            );
        }
    }

    #[tokio::test]
    async fn test_batch_query_encoding() {
        let builder = create_builder().await;

        let query = SoqlQueryBuilder::new()
            .select(&["Id", "Name"])
            .from("Account")
            .where_eq("Name", "Acme & Co.");

        let mut builder = builder.query(query);

        // Check the request
        let req = builder.requests.pop().expect("No request added");
        assert_eq!(req.method, "GET");

        // Verify encoding
        // SoqlQueryBuilder produces: SELECT Id, Name FROM Account WHERE Name = 'Acme & Co.'
        // Note: SoqlQueryBuilder escapes ' but not & unless needed for SOSL, but for SOQL literals & is fine inside quotes.
        // Wait, does SoqlQueryBuilder escape &? No.

        let expected_soql = "SELECT Id, Name FROM Account WHERE Name = 'Acme & Co.'";
        let expected_encoded: String = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("q", expected_soql)
            .finish();

        assert_eq!(req.url, format!("query?{}", expected_encoded));

        // Also ensure + is used for space (application/x-www-form-urlencoded default)
        assert!(req.url.contains("SELECT+Id"));
        // Ensure & is encoded as %26 inside the value
        assert!(req.url.contains("%26"));
    }
}
