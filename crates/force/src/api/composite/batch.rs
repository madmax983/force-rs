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

struct UrlEncodedWriter<'a>(&'a mut String);

impl std::fmt::Write for UrlEncodedWriter<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0
            .extend(url::form_urlencoded::byte_serialize(s.as_bytes()));
        Ok(())
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
    /// Returns an error if the `sobject` name or `id` contains invalid characters,
    /// or if the batch size limit (25) is exceeded.
    pub fn get(self, sobject: &str, id: &str) -> Result<Self> {
        validator::validate_sobject_name(sobject)?;
        validate_id(id)?;
        self.add_request("GET", format!("sobjects/{}/{}", sobject, id), None)
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
    /// Returns an error if the `sobject` name contains invalid characters,
    /// or if the batch size limit (25) is exceeded.
    pub fn post(self, sobject: &str, body: Value) -> Result<Self> {
        validator::validate_sobject_name(sobject)?;
        self.add_request("POST", format!("sobjects/{}", sobject), Some(body))
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
    /// Returns an error if the `sobject` name or `id` contains invalid characters,
    /// or if the batch size limit (25) is exceeded.
    pub fn patch(self, sobject: &str, id: &str, body: Value) -> Result<Self> {
        validator::validate_sobject_name(sobject)?;
        validate_id(id)?;
        self.add_request("PATCH", format!("sobjects/{}/{}", sobject, id), Some(body))
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
    /// Returns an error if the `sobject` name or `id` contains invalid characters,
    /// or if the batch size limit (25) is exceeded.
    pub fn delete(self, sobject: &str, id: &str) -> Result<Self> {
        validator::validate_sobject_name(sobject)?;
        validate_id(id)?;
        self.add_request("DELETE", format!("sobjects/{}/{}", sobject, id), None)
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
    /// # Errors
    ///
    /// Returns an error if the batch size limit (25) is exceeded.
    ///
    /// # Performance
    ///
    /// Accepts `impl Into<String>` to avoid unnecessary allocations when the caller
    /// already has an owned `String` (e.g. from `format!`).
    pub fn add_request(
        mut self,
        method: impl Into<String>,
        url: impl Into<String>,
        body: Option<Value>,
    ) -> Result<Self> {
        if self.requests.len() >= 25 {
            return Err(ForceError::InvalidInput(
                "Batch size limit of 25 requests exceeded".to_string(),
            ));
        }

        self.requests.push(BatchSubRequest {
            method: method.into(),
            url: url.into(),
            rich_input: body,
        });
        Ok(self)
    }

    /// Adds a SOQL query request to the batch.
    ///
    /// This method automatically URL-encodes the query string to prevent injection vulnerabilities
    /// and ensures valid URL formatting.
    ///
    /// Performance: Uses streaming URL-encoding to write the query directly into the URL buffer,
    /// avoiding intermediate string allocations.
    ///
    /// # Arguments
    ///
    /// * `query_builder` - The SOQL query builder
    ///
    /// # Errors
    ///
    /// Returns an error if the batch size limit (25) is exceeded.
    ///
    /// # Errors
    ///
    /// Returns an error if the batch size limit (25) is exceeded, if the query
    /// builder is invalid (e.g. missing fields or SObject), or if formatting fails.
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
    ///     .query(query)?
    ///     .execute()
    ///     .await?;
    /// ```
    #[allow(clippy::needless_pass_by_value)] // Ownership consumed to enforce builder pattern
    pub fn query(mut self, query_builder: SoqlQueryBuilder) -> Result<Self> {
        if self.requests.len() >= 25 {
            return Err(ForceError::InvalidInput(
                "Batch size limit of 25 requests exceeded".to_string(),
            ));
        }

        if let Err(e) = query_builder.validate() {
            return Err(ForceError::InvalidInput(format!(
                "Invalid query builder: {}",
                e
            )));
        }

        // 256 + 8 is a reasonable guess for typical queries
        let mut url = String::with_capacity(256 + 8);
        url.push_str("query?q=");

        {
            let mut writer = UrlEncodedWriter(&mut url);
            if let Err(e) = query_builder.write_query(&mut writer) {
                return Err(ForceError::InvalidInput(format!(
                    "Formatting failed: {}",
                    e
                )));
            }
        }

        self.requests.push(BatchSubRequest {
            method: "GET".to_string(),
            url,
            rich_input: None,
        });
        Ok(self)
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
            // This is theoretically unreachable now due to add_request check
            // but kept as defense in depth.
            return Err(ForceError::Serialization(
                crate::error::SerializationError::InvalidFormat(
                    "Batch size exceeds limit of 25 requests".to_string(),
                ),
            ));
        }

        // Construct the composite batch URL
        let url = self.handler.inner.resolve_url("composite/batch").await?;

        let request_body = BatchRequest {
            batch_requests: self.requests,
            halt_on_error: self.halt_on_error,
        };

        let request = self
            .handler
            .inner
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
        let value: serde_json::Value = serde_json::from_str(&json).must();

        assert_eq!(value["haltOnError"], true);

        let requests = value["batchRequests"]
            .as_array()
            .expect("batchRequests should be an array");
        assert_eq!(requests.len(), 2);

        let req1 = &requests[0];
        assert_eq!(req1["method"], "GET");
        assert_eq!(req1["url"], "sobjects/Account/001");
        assert!(req1.get("richInput").is_none());

        let req2 = &requests[1];
        assert_eq!(req2["method"], "POST");
        assert_eq!(req2["url"], "sobjects/Contact");
        assert_eq!(req2["richInput"]["LastName"], "Doe");
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
    async fn test_batch_query_encoding_special_chars() {
        let builder = create_builder().await;

        let query = SoqlQueryBuilder::new()
            .select(&["Id"])
            .from("Account")
            .where_eq("Name", "100% + 50%");

        let mut builder = builder.query(query).unwrap();

        let req = builder.requests.pop().expect("No request added");

        // Expected SOQL: SELECT Id FROM Account WHERE Name = '100% + 50%'
        // Encoded: query?q=SELECT+Id+FROM+Account+WHERE+Name+%3D+%27100%25+%2B+50%25%27
        // Space -> +
        // ' -> %27
        // % -> %25
        // + -> %2B

        let expected_url = "query?q=SELECT+Id+FROM+Account+WHERE+Name+%3D+%27100%25+%2B+50%25%27";

        assert_eq!(req.url, expected_url);
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
            // With the new Result return type, we have to unwrap each time
            // We expect the last one (i=25) to fail
            let res = builder.get("Account", &format!("001000000000{:03}AAA", i));

            if i < 25 {
                builder = res.unwrap();
            } else {
                assert!(res.is_err());
                if let Err(ForceError::InvalidInput(msg)) = res {
                    assert!(msg.contains("Batch size limit of 25 requests exceeded"));
                } else {
                    panic!("Expected InvalidInput error for batch size limit");
                }
                return; // Test passed
            }
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
        // It will fail because of mock authenticator probably not handling 25 requests or just returning generic error,
        // but it shouldn't be the size limit error we added to execute (which is now redundant but kept)
        if let Err(ForceError::Serialization(e)) = &result {
            // If we get here, check it's NOT the size limit message
            assert!(
                !e.to_string().contains("Batch size exceeds limit"),
                "Batch size limit triggered for 25 requests (should allow up to 25)"
            );
        }
    }

    #[tokio::test]
    async fn test_batch_query_invalid_builder() {
        let builder = create_builder().await;

        // Create an invalid builder (missing FROM clause)
        let query = SoqlQueryBuilder::new().select(&["Id"]);

        let res = builder.query(query);

        assert!(res.is_err());
        let err_msg = res.unwrap_err().to_string();
        assert!(
            err_msg.contains("Invalid query builder")
                || err_msg.contains("query requires a FROM clause"),
            "Error message did not match expected: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_batch_query_encoding() {
        let builder = create_builder().await;

        let query = SoqlQueryBuilder::new()
            .select(&["Id", "Name"])
            .from("Account")
            .where_eq("Name", "Acme & Co.");

        let mut builder = builder.query(query).unwrap();

        // Check the request
        let req = builder.requests.pop().expect("No request added");
        assert_eq!(req.method, "GET");

        // Verify encoding against a hardcoded expected string.
        // This ensures that we are not just mirroring the implementation's encoding logic.
        // Expected SOQL: SELECT Id, Name FROM Account WHERE Name = 'Acme & Co.'
        // Encoded: query?q=SELECT+Id%2C+Name+FROM+Account+WHERE+Name+%3D+%27Acme+%26+Co.%27
        // Note: We expect application/x-www-form-urlencoded encoding (spaces are +)

        let expected_url =
            "query?q=SELECT+Id%2C+Name+FROM+Account+WHERE+Name+%3D+%27Acme+%26+Co.%27";

        assert_eq!(req.url, expected_url);
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
        let mut builder = builder.add_request("GET", unsafe_url, None).unwrap();

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
        let mut builder = builder.add_request(method, url, None).unwrap();

        let req = builder.requests.pop().expect("No request added");

        assert_eq!(req.method, "POST");
        assert_eq!(req.url, "sobjects/Account");
    }
}
