//! Composite Batch API.
//!
//! A batch request is a collection of up to 25 subrequests that you execute
//! in a single call. Each subrequest is independent and can be of a different
//! type (GET, POST, PATCH, DELETE).

use super::CompositeHandler;
use crate::auth::Authenticator;
use crate::error::{ForceError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    /// Panics if the SObject name or ID contains invalid characters.
    #[must_use]
    pub fn get(mut self, sobject: &str, id: &str) -> Self {
        Self::validate_sobject_name(sobject);
        Self::validate_id(id);

        self.requests.push(BatchSubRequest {
            method: "GET".to_string(),
            url: format!("sobjects/{}/{}", sobject, id),
            rich_input: None,
        });
        self
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
    /// Panics if the SObject name contains invalid characters.
    #[must_use]
    pub fn post(mut self, sobject: &str, body: Value) -> Self {
        Self::validate_sobject_name(sobject);

        self.requests.push(BatchSubRequest {
            method: "POST".to_string(),
            url: format!("sobjects/{}", sobject),
            rich_input: Some(body),
        });
        self
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
    /// Panics if the SObject name or ID contains invalid characters.
    #[must_use]
    pub fn patch(mut self, sobject: &str, id: &str, body: Value) -> Self {
        Self::validate_sobject_name(sobject);
        Self::validate_id(id);

        self.requests.push(BatchSubRequest {
            method: "PATCH".to_string(),
            url: format!("sobjects/{}/{}", sobject, id),
            rich_input: Some(body),
        });
        self
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
    /// Panics if the SObject name or ID contains invalid characters.
    #[must_use]
    pub fn delete(mut self, sobject: &str, id: &str) -> Self {
        Self::validate_sobject_name(sobject);
        Self::validate_id(id);

        self.requests.push(BatchSubRequest {
            method: "DELETE".to_string(),
            url: format!("sobjects/{}/{}", sobject, id),
            rich_input: None,
        });
        self
    }

    fn validate_sobject_name(name: &str) {
        assert!(!name.is_empty(), "Invalid SObject name: cannot be empty");
        assert!(
            name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "Invalid SObject name: '{}' contains invalid characters",
            name
        );
    }

    fn validate_id(id: &str) {
        assert!(
            crate::types::SalesforceId::new(id).is_ok(),
            "Invalid Salesforce ID: '{}'",
            id
        );
    }

    /// Adds a custom subrequest to the batch.
    ///
    /// Use this for requests that don't fit the standard CRUD patterns,
    /// such as queries or parameterized searches.
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

    /// Executes the batch request.
    ///
    /// Sends all accumulated subrequests to the Salesforce Composite API.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The batch size is 0 or exceeds 25
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

        let api_version = self.handler.api_version();
        // Construct the composite batch URL
        // It must be absolute for the HTTP client
        // We use the same pattern as RestHandler: resolve_url

        // We need access to inner client to get token and instance URL
        let token = self.handler.inner.token_manager.get_token_arc().await?;
        let url = format!(
            "{}/services/data/{}/composite/batch",
            token.instance_url(),
            api_version
        );

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

    // Validation tests

    use crate::client::builder;
    use crate::test_support::MockAuthenticator;

    async fn create_test_handler() -> super::CompositeHandler<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", "http://localhost");
        let client = builder().authenticate(auth).build().await.must();
        client.composite()
    }

    #[tokio::test]
    #[should_panic(expected = "Invalid SObject name")]
    async fn test_batch_builder_panics_on_invalid_sobject_name() {
        let handler = create_test_handler().await;
        // Should panic
        let _ = BatchBuilder::new(handler).get("Account; DROP TABLE", "001000000000001");
    }

    #[tokio::test]
    #[should_panic(expected = "Invalid Salesforce ID")]
    async fn test_batch_builder_panics_on_invalid_id() {
        let handler = create_test_handler().await;
        // Should panic
        let _ = BatchBuilder::new(handler).get("Account", "invalid_id_format");
    }

    #[tokio::test]
    #[should_panic(expected = "Invalid SObject name")]
    async fn test_batch_builder_panics_on_empty_sobject() {
        let handler = create_test_handler().await;
        // Should panic
        let _ = BatchBuilder::new(handler).get("", "001000000000001");
    }

    #[tokio::test]
    #[should_panic(expected = "Invalid Salesforce ID")]
    async fn test_batch_builder_panics_on_malformed_id_chars() {
        let handler = create_test_handler().await;
        // Should panic - special chars not allowed in ID
        let _ = BatchBuilder::new(handler).get("Account", "001000000@00001");
    }

    #[tokio::test]
    async fn test_batch_builder_execute_fails_on_empty_batch() {
        let handler = create_test_handler().await;
        let builder = BatchBuilder::new(handler);

        let result = builder.execute().await;
        assert!(result.is_err());
        match result {
            Err(ForceError::Serialization(crate::error::SerializationError::InvalidFormat(msg))) => {
                assert_eq!(msg, "Batch cannot be empty");
            }
            _ => panic!("Expected InvalidFormat error for empty batch"),
        }
    }
}
