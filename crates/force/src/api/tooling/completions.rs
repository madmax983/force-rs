//! Tooling API completions endpoint.
//!
//! Provides code completion suggestions for Apex and Visualforce code via the
//! `/tooling/completions` endpoint.

use crate::api::rest_operation::RestOperation;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Type of code completion to request.
///
/// Determines whether the completions endpoint returns Apex or Visualforce
/// completion suggestions.
///
/// # Examples
///
/// ```
/// use force::api::tooling::CompletionsType;
///
/// let apex = CompletionsType::Apex;
/// assert_eq!(apex.to_string(), "apex");
///
/// let vf = CompletionsType::Visualforce;
/// assert_eq!(vf.to_string(), "visualforce");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompletionsType {
    /// Apex code completions.
    Apex,
    /// Visualforce markup completions.
    Visualforce,
}

impl CompletionsType {
    /// Returns the string representation used in API query parameters.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Apex => "apex",
            Self::Visualforce => "visualforce",
        }
    }
}

impl fmt::Display for CompletionsType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Result of a completions request.
///
/// The completions response from Salesforce has a flexible, loosely-defined
/// structure. The inner completions data uses `serde_json::Value` to
/// accommodate the varying response shapes returned by the API.
///
/// # Examples
///
/// ```
/// use force::api::tooling::CompletionsResult;
///
/// let json = serde_json::json!({
///     "completions": [
///         { "publicDeclarations": { "System": [] } }
///     ]
/// });
///
/// let result: CompletionsResult = serde_json::from_value(json).unwrap();
/// assert_eq!(result.completions.len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompletionsResult {
    /// List of completion entries.
    ///
    /// Each entry is a `serde_json::Value` because the Salesforce Tooling API
    /// returns a nested map structure whose shape varies depending on the
    /// query and completion type.
    pub completions: Vec<serde_json::Value>,
}

impl<A: crate::auth::Authenticator> super::ToolingHandler<A> {
    /// Retrieves code completion suggestions for Apex or Visualforce.
    ///
    /// Calls the Salesforce Tooling API completions endpoint to obtain
    /// code completion candidates based on the provided query fragment.
    ///
    /// # Arguments
    ///
    /// * `completions_type` - Whether to get Apex or Visualforce completions.
    /// * `query` - The code fragment to get completions for (e.g., `"System.d"`).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The response cannot be deserialized
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use force::api::tooling::CompletionsType;
    ///
    /// let result = client.tooling()
    ///     .completions(CompletionsType::Apex, "System.d")
    ///     .await?;
    ///
    /// for entry in &result.completions {
    ///     println!("{}", entry);
    /// }
    /// ```
    pub async fn completions(
        &self,
        completions_type: CompletionsType,
        query: &str,
    ) -> crate::error::Result<CompletionsResult> {
        let url = self.session().resolve_url("tooling/completions").await?;
        let request = self
            .session()
            .get(&url)
            .query(&[("type", completions_type.as_str()), ("q", query)])
            .build()
            .map_err(crate::error::HttpError::from)?;

        self.session()
            .send_request_and_decode(request, "Completions request failed")
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

    // ── CompletionsType Display tests ─────────────────────────────────

    #[test]
    fn test_completions_type_display_apex() {
        assert_eq!(CompletionsType::Apex.to_string(), "apex");
    }

    #[test]
    fn test_completions_type_display_visualforce() {
        assert_eq!(CompletionsType::Visualforce.to_string(), "visualforce");
    }

    #[test]
    fn test_completions_type_debug() {
        let debug = format!("{:?}", CompletionsType::Apex);
        assert_eq!(debug, "Apex");
    }

    #[test]
    fn test_completions_type_clone() {
        let original = CompletionsType::Visualforce;
        let cloned = original;
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_completions_type_eq() {
        assert_eq!(CompletionsType::Apex, CompletionsType::Apex);
        assert_ne!(CompletionsType::Apex, CompletionsType::Visualforce);
    }

    // ── CompletionsResult deserialization tests ───────────────────────

    #[test]
    fn test_completions_result_deserialize() {
        let json = json!({
            "completions": [
                {
                    "publicDeclarations": {
                        "System": [
                            {
                                "name": "debug",
                                "parameters": [
                                    { "name": "msg", "type": "Object" }
                                ]
                            }
                        ]
                    }
                }
            ]
        });

        let result: CompletionsResult = serde_json::from_value(json).must();
        assert_eq!(result.completions.len(), 1);

        let entry = &result.completions[0];
        assert!(entry.get("publicDeclarations").is_some());
    }

    #[test]
    fn test_completions_result_deserialize_empty() {
        let json = json!({ "completions": [] });

        let result: CompletionsResult = serde_json::from_value(json).must();
        assert!(result.completions.is_empty());
    }

    #[test]
    fn test_completions_result_serialize_roundtrip() {
        let original = CompletionsResult {
            completions: vec![json!({"publicDeclarations": {}})],
        };

        let serialized = serde_json::to_value(&original).must();
        let deserialized: CompletionsResult = serde_json::from_value(serialized).must();

        assert_eq!(original, deserialized);
    }

    // ── Wiremock integration tests ───────────────────────────────────

    #[tokio::test]
    async fn test_completions_apex() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        let response_body = json!({
            "completions": [
                {
                    "publicDeclarations": {
                        "System": [
                            {
                                "name": "debug",
                                "parameters": [
                                    { "name": "msg", "type": "Object" }
                                ]
                            }
                        ]
                    }
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/tooling/completions"))
            .and(query_param("type", "apex"))
            .and(query_param("q", "System.d"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .expect(1)
            .mount(&mock_server)
            .await;

        let result = client
            .tooling()
            .completions(CompletionsType::Apex, "System.d")
            .await
            .must();

        assert_eq!(result.completions.len(), 1);

        let declarations = &result.completions[0]["publicDeclarations"];
        assert!(declarations["System"].is_array());
    }

    #[tokio::test]
    async fn test_completions_visualforce() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        let response_body = json!({
            "completions": [
                {
                    "publicDeclarations": {
                        "apex": [
                            { "name": "outputText", "parameters": [] }
                        ]
                    }
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/tooling/completions"))
            .and(query_param("type", "visualforce"))
            .and(query_param("q", "apex:o"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .expect(1)
            .mount(&mock_server)
            .await;

        let result = client
            .tooling()
            .completions(CompletionsType::Visualforce, "apex:o")
            .await
            .must();

        assert_eq!(result.completions.len(), 1);
    }

    #[tokio::test]
    async fn test_completions_empty_result() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/tooling/completions"))
            .and(query_param("type", "apex"))
            .and(query_param("q", "xyznonexistent"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "completions": [] })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let result = client
            .tooling()
            .completions(CompletionsType::Apex, "xyznonexistent")
            .await
            .must();

        assert!(result.completions.is_empty());
    }

    #[tokio::test]
    async fn test_completions_http_error_400() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/tooling/completions"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!([
                { "errorCode": "INVALID_TYPE", "message": "Invalid type parameter" }
            ])))
            .expect(1)
            .mount(&mock_server)
            .await;

        let result = client
            .tooling()
            .completions(CompletionsType::Apex, "bad")
            .await;

        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(
            matches!(
                err,
                crate::error::ForceError::Api(_) | crate::error::ForceError::Http(_)
            ),
            "Expected Api or Http error, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_completions_http_error_500() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/tooling/completions"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .expect(1)
            .mount(&mock_server)
            .await;

        let result = client
            .tooling()
            .completions(CompletionsType::Visualforce, "test")
            .await;

        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(
            matches!(
                err,
                crate::error::ForceError::Api(_) | crate::error::ForceError::Http(_)
            ),
            "Expected Api or Http error, got: {err}"
        );
    }
}
