//! Execute Anonymous Apex endpoint for the Tooling API.
//!
//! This module provides the [`ExecuteAnonymousResult`] type and the
//! `execute_anonymous` method on [`ToolingHandler`](super::ToolingHandler).
//!
//! # Salesforce API
//!
//! **Endpoint:** `GET /services/data/v60.0/tooling/executeAnonymous/?anonymousBody={code}`
//!
//! The code is URL-encoded and sent as a query parameter. Salesforce compiles
//! and executes the Apex code in the context of the authenticated user's org.
//!
//! # Examples
//!
//! ```ignore
//! use force::api::rest_operation::RestOperation;
//!
//! let result = client.tooling()
//!     .execute_anonymous("System.debug('Hello from Apex');")
//!     .await?;
//!
//! assert!(result.is_success());
//! ```

use crate::api::rest_operation::RestOperation;
use serde::{Deserialize, Serialize};

/// Result of executing anonymous Apex code via the Tooling API.
///
/// This struct maps directly to the JSON response from
/// `GET /services/data/v60.0/tooling/executeAnonymous/`.
///
/// Three outcome states are possible:
///
/// 1. **Success** -- `compiled == true && success == true`
/// 2. **Compile error** -- `compiled == false` with `compile_problem` populated
/// 3. **Runtime error** -- `compiled == true && success == false` with
///    `exception_message` and `exception_stack_trace` populated
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteAnonymousResult {
    /// Whether the code compiled successfully.
    pub compiled: bool,

    /// Compilation error message (populated when `compiled` is `false`).
    pub compile_problem: Option<String>,

    /// Whether execution completed without an exception.
    pub success: bool,

    /// Runtime exception message (populated when `compiled` is `true`
    /// but `success` is `false`).
    pub exception_message: Option<String>,

    /// Runtime exception stack trace.
    pub exception_stack_trace: Option<String>,

    /// Line number of the error (`-1` if no error).
    pub line: i32,

    /// Column number of the error (`-1` if no error).
    pub column: i32,
}

impl ExecuteAnonymousResult {
    /// Returns `true` if the code compiled and executed without exception.
    pub fn is_success(&self) -> bool {
        self.compiled && self.success
    }

    /// Returns `true` if compilation failed.
    pub fn is_compile_error(&self) -> bool {
        !self.compiled
    }

    /// Returns `true` if the code compiled but threw a runtime exception.
    pub fn is_runtime_error(&self) -> bool {
        self.compiled && !self.success
    }
}

impl<A: crate::auth::Authenticator> super::ToolingHandler<A> {
    /// Executes anonymous Apex code in the authenticated org.
    ///
    /// The code string is URL-encoded and sent as the `anonymousBody` query
    /// parameter to the Tooling API's `executeAnonymous` endpoint.
    ///
    /// # Arguments
    ///
    /// * `code` - The Apex source code to compile and execute.
    ///
    /// # Errors
    ///
    /// Returns [`ForceError`](crate::error::ForceError) if:
    /// - Authentication fails or the token cannot be retrieved
    /// - The HTTP request fails (network error, 5xx, etc.)
    /// - The response body cannot be deserialized
    ///
    /// Note: a *compilation failure* or *runtime exception* is **not** an error
    /// at the HTTP level. Those are represented as a successful HTTP 200
    /// response with `compiled == false` or `success == false` in the result.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let result = client.tooling()
    ///     .execute_anonymous("System.debug('Hello');")
    ///     .await?;
    ///
    /// if result.is_success() {
    ///     println!("Apex executed successfully");
    /// } else if result.is_compile_error() {
    ///     println!("Compile error: {}", result.compile_problem.unwrap_or_default());
    /// } else if result.is_runtime_error() {
    ///     println!("Runtime error: {}", result.exception_message.unwrap_or_default());
    /// }
    /// ```
    pub async fn execute_anonymous(
        &self,
        code: &str,
    ) -> crate::error::Result<ExecuteAnonymousResult> {
        let url = self
            .session()
            .resolve_url("tooling/executeAnonymous")
            .await?;

        let request = self
            .session()
            .get(&url)
            .query(&[("anonymousBody", code)])
            .build()
            .map_err(crate::error::HttpError::from)?;

        self.session()
            .send_request_and_decode(request, "Execute anonymous Apex failed")
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::builder;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // ── Deserialization tests ────────────────────────────────────────

    #[test]
    fn test_deserialize_success_response() {
        let json = serde_json::json!({
            "line": -1,
            "column": -1,
            "compiled": true,
            "success": true,
            "compileProblem": null,
            "exceptionMessage": null,
            "exceptionStackTrace": null
        });

        let result: ExecuteAnonymousResult = serde_json::from_value(json).must();

        assert!(result.compiled);
        assert!(result.success);
        assert_eq!(result.line, -1);
        assert_eq!(result.column, -1);
        assert!(result.compile_problem.is_none());
        assert!(result.exception_message.is_none());
        assert!(result.exception_stack_trace.is_none());
    }

    #[test]
    fn test_deserialize_compile_error_response() {
        let json = serde_json::json!({
            "line": 1,
            "column": 13,
            "compiled": false,
            "success": false,
            "compileProblem": "Unexpected token 'invalid'.",
            "exceptionMessage": null,
            "exceptionStackTrace": null
        });

        let result: ExecuteAnonymousResult = serde_json::from_value(json).must();

        assert!(!result.compiled);
        assert!(!result.success);
        assert_eq!(result.line, 1);
        assert_eq!(result.column, 13);
        assert_eq!(
            result.compile_problem.as_deref(),
            Some("Unexpected token 'invalid'.")
        );
        assert!(result.exception_message.is_none());
        assert!(result.exception_stack_trace.is_none());
    }

    #[test]
    fn test_deserialize_runtime_error_response() {
        let json = serde_json::json!({
            "line": 1,
            "column": -1,
            "compiled": true,
            "success": false,
            "compileProblem": null,
            "exceptionMessage": "System.DmlException: Insert failed...",
            "exceptionStackTrace": "AnonymousBlock: line 1, column 1"
        });

        let result: ExecuteAnonymousResult = serde_json::from_value(json).must();

        assert!(result.compiled);
        assert!(!result.success);
        assert_eq!(result.line, 1);
        assert_eq!(result.column, -1);
        assert!(result.compile_problem.is_none());
        assert_eq!(
            result.exception_message.as_deref(),
            Some("System.DmlException: Insert failed...")
        );
        assert_eq!(
            result.exception_stack_trace.as_deref(),
            Some("AnonymousBlock: line 1, column 1")
        );
    }

    // ── Helper method tests ─────────────────────────────────────────

    #[test]
    fn test_is_success() {
        let result = ExecuteAnonymousResult {
            compiled: true,
            success: true,
            compile_problem: None,
            exception_message: None,
            exception_stack_trace: None,
            line: -1,
            column: -1,
        };

        assert!(result.is_success());
        assert!(!result.is_compile_error());
        assert!(!result.is_runtime_error());
    }

    #[test]
    fn test_is_compile_error() {
        let result = ExecuteAnonymousResult {
            compiled: false,
            success: false,
            compile_problem: Some("Unexpected token".to_string()),
            exception_message: None,
            exception_stack_trace: None,
            line: 1,
            column: 13,
        };

        assert!(!result.is_success());
        assert!(result.is_compile_error());
        assert!(!result.is_runtime_error());
    }

    #[test]
    fn test_is_runtime_error() {
        let result = ExecuteAnonymousResult {
            compiled: true,
            success: false,
            compile_problem: None,
            exception_message: Some("System.DmlException".to_string()),
            exception_stack_trace: Some("AnonymousBlock: line 1".to_string()),
            line: 1,
            column: -1,
        };

        assert!(!result.is_success());
        assert!(!result.is_compile_error());
        assert!(result.is_runtime_error());
    }

    // ── Integration tests (wiremock) ────────────────────────────────

    #[tokio::test]
    async fn test_execute_anonymous_success() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        let apex_code = "System.debug('Hello from Apex');";

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/tooling/executeAnonymous"))
            .and(query_param("anonymousBody", apex_code))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "line": -1,
                "column": -1,
                "compiled": true,
                "success": true,
                "compileProblem": null,
                "exceptionMessage": null,
                "exceptionStackTrace": null
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let result = client.tooling().execute_anonymous(apex_code).await.must();

        assert!(result.is_success());
        assert!(result.compiled);
        assert!(result.success);
        assert_eq!(result.line, -1);
        assert_eq!(result.column, -1);
        assert!(result.compile_problem.is_none());
        assert!(result.exception_message.is_none());
    }

    #[tokio::test]
    async fn test_execute_anonymous_compile_error() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        let apex_code = "invalid apex code here;";

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/tooling/executeAnonymous"))
            .and(query_param("anonymousBody", apex_code))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "line": 1,
                "column": 13,
                "compiled": false,
                "success": false,
                "compileProblem": "Unexpected token 'invalid'.",
                "exceptionMessage": null,
                "exceptionStackTrace": null
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let result = client.tooling().execute_anonymous(apex_code).await.must();

        assert!(result.is_compile_error());
        assert!(!result.compiled);
        assert!(!result.success);
        assert_eq!(result.line, 1);
        assert_eq!(result.column, 13);
        assert_eq!(
            result.compile_problem.as_deref(),
            Some("Unexpected token 'invalid'.")
        );
    }

    #[tokio::test]
    async fn test_execute_anonymous_runtime_error() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        let apex_code = "Account a = new Account(); insert a;";

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/tooling/executeAnonymous"))
            .and(query_param("anonymousBody", apex_code))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "line": 1,
                "column": -1,
                "compiled": true,
                "success": false,
                "compileProblem": null,
                "exceptionMessage": "System.DmlException: Insert failed. First exception on row 0; first error: REQUIRED_FIELD_MISSING, Required fields are missing: [Name]: [Name]",
                "exceptionStackTrace": "AnonymousBlock: line 1, column 1"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let result = client.tooling().execute_anonymous(apex_code).await.must();

        assert!(result.is_runtime_error());
        assert!(result.compiled);
        assert!(!result.success);
        assert!(
            result
                .exception_message
                .as_deref()
                .is_some_and(|m| m.contains("System.DmlException"))
        );
        assert!(
            result
                .exception_stack_trace
                .as_deref()
                .is_some_and(|t| t.contains("AnonymousBlock"))
        );
    }

    #[tokio::test]
    async fn test_execute_anonymous_http_error() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/tooling/executeAnonymous"))
            .respond_with(
                ResponseTemplate::new(500).set_body_json(serde_json::json!([{
                    "message": "Internal Server Error",
                    "errorCode": "UNKNOWN_EXCEPTION"
                }])),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let result = client
            .tooling()
            .execute_anonymous("System.debug('boom');")
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

    #[test]
    fn test_execute_anonymous_result_roundtrip_serialization() {
        let original = ExecuteAnonymousResult {
            compiled: true,
            success: true,
            compile_problem: None,
            exception_message: None,
            exception_stack_trace: None,
            line: -1,
            column: -1,
        };

        let json = serde_json::to_value(&original).must();
        let deserialized: ExecuteAnonymousResult = serde_json::from_value(json).must();

        assert_eq!(original, deserialized);
    }
}
