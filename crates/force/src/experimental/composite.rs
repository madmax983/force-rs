//! Composite API handler for Salesforce.
//!
//! This module provides the `CompositeHandler` which enables executing multiple
//! REST API requests in a single call.

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Composite API handler for performing Salesforce Composite operations.
#[derive(Debug, Clone)]
pub struct CompositeHandler<A: crate::auth::Authenticator> {
    /// Reference to the client's inner state.
    inner: Arc<crate::client::inner::Inner<A>>,
}

impl<A: crate::auth::Authenticator> CompositeHandler<A> {
    /// Creates a new Composite handler for the given client inner state.
    #[must_use]
    pub(crate) fn new(inner: Arc<crate::client::inner::Inner<A>>) -> Self {
        Self { inner }
    }

    /// Constructs the base URL for Composite API operations.
    pub async fn base_url(&self) -> Result<String> {
        let token = self.inner.token_manager.token().await?;
        Ok(format!(
            "{}/services/data/{}",
            token.instance_url(),
            self.inner.config.api_version
        ))
    }

    /// Executes a batch of requests in a single call.
    ///
    /// # Arguments
    ///
    /// * `batch` - The batch request containing sub-requests
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The response cannot be deserialized
    pub async fn batch(&self, batch: &BatchRequest<'_>) -> Result<BatchResponse> {
        let url = format!("{}/composite/batch", self.base_url().await?);
        let request = self
            .inner
            .http_client
            .post(&url)
            .json(batch)
            .build()
            .map_err(crate::error::HttpError::from)?;

        self.inner
            .send_request_and_decode(request, "Composite Batch request failed")
            .await
    }
}

/// A request to the Composite Batch API.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BatchRequest<'a> {
    /// The list of subrequests to execute.
    pub batch_requests: Vec<SubRequest<'a>>,
    /// Whether to stop processing on the first error.
    pub halt_on_error: bool,
}

/// A subrequest within a batch.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SubRequest<'a> {
    /// The HTTP method to use (e.g., "GET", "POST").
    pub method: &'a str,
    /// The resource URL, relative to the version root (e.g., "sobjects/Account/001...").
    pub url: &'a str,
    /// The request body (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rich_input: Option<serde_json::Value>,
    /// A reference ID to correlate requests and responses (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_id: Option<&'a str>,
}

/// The response from the Composite Batch API.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BatchResponse {
    /// Whether any of the subrequests failed.
    pub has_errors: bool,
    /// The results of the subrequests.
    pub results: Vec<SubResponse>,
}

/// The result of a subrequest.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SubResponse {
    /// The HTTP status code of the subrequest.
    pub status_code: u16,
    /// The response body (optional).
    pub result: Option<serde_json::Value>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_batch_request_serialization() {
        let req = BatchRequest {
            batch_requests: vec![
                SubRequest {
                    method: "GET",
                    url: "sobjects/Account/123",
                    rich_input: None,
                    reference_id: Some("ref1"),
                },
                SubRequest {
                    method: "POST",
                    url: "sobjects/Account",
                    rich_input: Some(json!({"Name": "New Account"})),
                    reference_id: None,
                },
            ],
            halt_on_error: true,
        };

        let json = serde_json::to_string(&req).unwrap();
        // Check key fields
        assert!(json.contains("\"method\":\"GET\""));
        assert!(json.contains("\"url\":\"sobjects/Account/123\""));
        assert!(json.contains("\"referenceId\":\"ref1\""));
        assert!(json.contains("\"richInput\":{\"Name\":\"New Account\"}"));
        assert!(json.contains("\"haltOnError\":true"));
    }

    #[test]
    fn test_batch_response_deserialization() {
        let json = json!({
            "hasErrors": false,
            "results": [
                {
                    "statusCode": 200,
                    "result": { "Id": "123", "Name": "Test" }
                },
                {
                    "statusCode": 204,
                    "result": null
                }
            ]
        });

        let resp: BatchResponse = serde_json::from_value(json).unwrap();
        assert!(!resp.has_errors);
        assert_eq!(resp.results.len(), 2);
        assert_eq!(resp.results[0].status_code, 200);
        assert!(resp.results[0].result.is_some());
        assert_eq!(resp.results[1].status_code, 204);
        assert!(resp.results[1].result.is_none());
    }
}
