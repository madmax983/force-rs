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

/// Helper to validate Salesforce IDs.
fn validate_id(id: &str) -> Result<()> {
    SalesforceId::new(id).map_err(|e| {
        ForceError::InvalidInput(format!("Salesforce ID contains invalid characters: {}", e))
    })?;
    Ok(())
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
    /// Creates a new BatchBuilder.
    ///
    /// Performance: Pre-allocates capacity for 25 requests (Salesforce limit)
    /// to avoid heap reallocations during request accumulation.
    pub(crate) fn new(handler: CompositeHandler<A>) -> Self {
        Self {
            handler,
            requests: Vec::with_capacity(25),
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
    /// # Errors
    ///
    /// Returns an error if the `sobject` name or `id` contains invalid characters.
    pub fn get(self, sobject: &str, id: &str) -> Result<Self> {
        validator::validate_sobject_name(sobject)?;
        validate_id(id)?;
        Ok(self.add_request("GET", format!("sobjects/{}/{}", sobject, id), None))
    }

    /// Adds a POST (Create) request to the batch.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The SObject type (e.g., "Account")
    /// * `body` - The JSON body of the record
    ///
    /// # Errors
    ///
    /// Returns an error if the `sobject` name contains invalid characters.
    pub fn post(self, sobject: &str, body: Value) -> Result<Self> {
        validator::validate_sobject_name(sobject)?;
        Ok(self.add_request("POST", format!("sobjects/{}", sobject), Some(body)))
    }

    /// Adds a PATCH (Update) request to the batch.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The SObject type (e.g., "Account")
    /// * `id` - The record ID
    /// * `body` - The JSON body with fields to update
    ///
    /// # Errors
    ///
    /// Returns an error if the `sobject` name or `id` contains invalid characters.
    pub fn patch(self, sobject: &str, id: &str, body: Value) -> Result<Self> {
        validator::validate_sobject_name(sobject)?;
        validate_id(id)?;
        Ok(self.add_request("PATCH", format!("sobjects/{}/{}", sobject, id), Some(body)))
    }

    /// Adds a DELETE request to the batch.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The SObject type (e.g., "Account")
    /// * `id` - The record ID
    ///
    /// # Errors
    ///
    /// Returns an error if the `sobject` name or `id` contains invalid characters.
    pub fn delete(self, sobject: &str, id: &str) -> Result<Self> {
        validator::validate_sobject_name(sobject)?;
        validate_id(id)?;
        Ok(self.add_request("DELETE", format!("sobjects/{}/{}", sobject, id), None))
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
    ///
    /// # Performance
    ///
    /// Accepts `impl Into<String>` to avoid unnecessary allocations when the caller
    /// already has an owned `String` (e.g. from `format!`).
    #[must_use]
    pub fn add_request(
        mut self,
        method: impl Into<String>,
        url: impl Into<String>,
        body: Option<Value>,
    ) -> Self {
        self.requests.push(BatchSubRequest {
            method: method.into(),
            url: url.into(),
            rich_input: body,
        });
        self
    }

    /// Adds a SOQL query request to the batch.
    ///
    /// This method automatically URL-encodes the query string to prevent injection vulnerabilities
    /// and ensures valid URL formatting.
    ///
    /// Performance: Uses `byte_serialize` to stream encoded output directly into the URL buffer,
    /// avoiding intermediate string allocations and `format!` overhead.
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

        // 8 is for "query?q=" and a bit of slack
        let mut url = String::with_capacity(query_string.len() + 8);
        url.push_str("query?q=");
        url.extend(url::form_urlencoded::byte_serialize(
            query_string.as_bytes(),
        ));

        self.requests.push(BatchSubRequest {
            method: "GET".to_string(),
            url,
            rich_input: None,
        });
        self
    }

    /// Returns the number of requests currently in the batch.
    #[must_use]
    pub fn len(&self) -> usize {
        self.requests.len()
    }

    /// Returns true if the batch is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    /// Returns true if the batch is full (25 requests).
    ///
    /// The Salesforce Composite API limits batch requests to 25 subrequests.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.requests.len() >= 25
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
    async fn test_batch_validation_sobject_invalid() {
        let builder = create_builder().await;
        let result = builder.get("Invalid;Name", "001000000000000");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("SObject name contains invalid characters")
        );
    }

    #[tokio::test]
    async fn test_batch_validation_sobject_empty() {
        let builder = create_builder().await;
        let result = builder.get("", "001000000000000");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("SObject name cannot be empty")
        );
    }

    #[tokio::test]
    async fn test_batch_validation_id_invalid() {
        let builder = create_builder().await;
        let result = builder.get("Account", "Invalid;ID");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Salesforce ID contains invalid characters")
        );
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
            builder = builder
                .get("Account", &format!("001000000000{:03}AAA", i))
                .unwrap();
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
            builder = builder
                .get("Account", &format!("001000000000{:03}AAA", i))
                .unwrap();
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

        // We expect form-urlencoded encoding (spaces are +)
        let mut expected_url = "query?q=".to_string();
        expected_url.extend(url::form_urlencoded::byte_serialize(
            expected_soql.as_bytes(),
        ));

        assert_eq!(req.url, expected_url);

        // Ensure space is encoded as + (application/x-www-form-urlencoded default)
        assert!(req.url.contains("SELECT+Id"));
        // Ensure & is encoded as %26 inside the value
        assert!(req.url.contains("%26"));
    }

    #[tokio::test]
    async fn test_batch_capacity_helpers() {
        let mut builder = create_builder().await;

        assert!(builder.is_empty());
        assert_eq!(builder.len(), 0);
        assert!(!builder.is_full());

        // Add 25 items
        for i in 0..25 {
            builder = builder
                .get("Account", &format!("001000000000{:03}AAA", i))
                .unwrap();
        }

        assert!(!builder.is_empty());
        assert_eq!(builder.len(), 25);
        assert!(builder.is_full());
    }

    #[tokio::test]
    async fn test_add_request_raw_url() {
        // Verify that add_request does NOT encode the URL
        // This is important behavior to document via test
        let builder = create_builder().await;

        let unsafe_url = "query?q=SELECT Id FROM Account";
        let mut builder = builder.add_request("GET", unsafe_url, None);

        let req = builder.requests.pop().expect("No request added");

        // It should match exactly what was passed
        assert_eq!(req.url, unsafe_url);

        // It should NOT be encoded (e.g. no + for spaces)
        assert!(!req.url.contains('+'));
    }

    #[tokio::test]
    async fn test_add_request_owned_optimization() {
        // Verify that add_request accepts owned String directly
        // This confirms the Zero-Cost Abstraction where we avoid cloning
        // if the caller already has an owned String (e.g. from format!)
        let builder = create_builder().await;

        let method = String::from("POST");
        let url = String::from("sobjects/Account");
        let mut builder = builder.add_request(method, url, None);

        let req = builder.requests.pop().expect("No request added");

        assert_eq!(req.method, "POST");
        assert_eq!(req.url, "sobjects/Account");
    }
}
