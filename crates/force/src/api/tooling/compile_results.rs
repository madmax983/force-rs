//! Tooling API `apexCompileResults` resource (v68.0+).
//!
//! Returns compile errors for Apex classes and triggers that are not valid.
//! The call does not change `ApexClass.IsValid`.

use crate::api::rest_operation::RestOperation;
use serde::{Deserialize, Serialize};

/// Operation-level outcome of `apexCompileResults`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CompileStatus {
    /// All classes and triggers compiled, or none needed a compile.
    Ok,
    /// One or more classes or triggers failed to compile.
    PartialFailure,
    /// A value that this crate does not know.
    #[serde(other)]
    Unknown,
}

/// One compile error or warning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompileProblem {
    /// Source line. `0` if not applicable.
    #[serde(default)]
    pub line: i64,
    /// Source column. `0` if not applicable.
    #[serde(default)]
    pub column: i64,
    /// Problem text.
    #[serde(default)]
    pub message: String,
}

/// Compile outcome for one class or trigger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApexCompileResult {
    /// Developer name of the class or trigger.
    pub name: String,
    /// Namespace. Empty for the default namespace.
    #[serde(default)]
    pub namespace: String,
    /// `false` when one or more problems exist.
    #[serde(default)]
    pub success: bool,
    /// Compile errors.
    #[serde(default)]
    pub problems: Vec<CompileProblem>,
    /// Compile warnings. Only present when errors also exist.
    #[serde(default)]
    pub warnings: Vec<CompileProblem>,
}

/// Response of `apexCompileResults`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApexCompileResults {
    /// Operation-level outcome.
    pub status: CompileStatus,
    /// Failed classes and triggers only. Empty on full success.
    #[serde(default)]
    pub results: Vec<ApexCompileResult>,
}

impl<A: crate::auth::Authenticator> super::ToolingHandler<A> {
    /// Gets compile results for Apex classes and triggers that are not valid.
    ///
    /// `POST /tooling/apexCompileResults/` with body `{}`. Needs API v68.0+
    /// and the Author Apex permission. The call is synchronous.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn apex_compile_results(&self) -> crate::error::Result<ApexCompileResults> {
        let url = self
            .session()
            .resolve_url("tooling/apexCompileResults/")
            .await?;
        let request = self
            .session()
            .post(&url)
            .json(&serde_json::json!({}))
            .build()
            .map_err(crate::error::HttpError::from)?;
        self.session()
            .send_request_and_decode(request, "apexCompileResults request failed")
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
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn test_decode_partial_failure() {
        let body = json!({
            "status": "PARTIAL_FAILURE",
            "results": [{
                "name": "MyInvalidClass",
                "namespace": "MyNamespace",
                "success": false,
                "problems": [{"line": 14, "column": 9, "message": "Variable does not exist: var1"}],
                "warnings": [{"line": 0, "column": 0, "message": "Apex API version 18.0 is scheduled for retirement."}]
            }]
        });
        let out: ApexCompileResults = serde_json::from_value(body).must();
        assert_eq!(out.status, CompileStatus::PartialFailure);
        assert_eq!(out.results[0].problems[0].line, 14);
        assert_eq!(out.results[0].warnings.len(), 1);
        assert!(!out.results[0].success);
    }

    #[test]
    fn test_decode_unknown_status_and_missing_arrays() {
        let out: ApexCompileResults =
            serde_json::from_value(json!({"status": "SOMETHING_NEW"})).must();
        assert_eq!(out.status, CompileStatus::Unknown);
        assert!(out.results.is_empty());
    }

    #[tokio::test]
    async fn test_apex_compile_results_posts_empty_object() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/services/data/v68.0/tooling/apexCompileResults/"))
            .and(body_json(json!({})))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"status": "OK", "results": []})),
            )
            .expect(1)
            .mount(&server)
            .await;
        let client = builder()
            .authenticate(MockAuthenticator::new("t", &server.uri()))
            .build()
            .await
            .must();

        let out = client.tooling().apex_compile_results().await.must();
        assert_eq!(out.status, CompileStatus::Ok);
        assert!(out.results.is_empty());
    }

    #[tokio::test]
    async fn test_apex_compile_results_error_status() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/services/data/v68.0/tooling/apexCompileResults/"))
            .respond_with(ResponseTemplate::new(403).set_body_json(json!([
                {"errorCode": "INSUFFICIENT_ACCESS", "message": "Author Apex required"}
            ])))
            .mount(&server)
            .await;
        let client = builder()
            .authenticate(MockAuthenticator::new("t", &server.uri()))
            .build()
            .await
            .must();

        assert!(client.tooling().apex_compile_results().await.is_err());
    }
}
