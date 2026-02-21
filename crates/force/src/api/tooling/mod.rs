//! Tooling API handler.
//!
//! The Tooling API provides access to fine-grained metadata and developer tools,
//! including Apex execution, debug logs, and code coverage.

use crate::auth::Authenticator;
use crate::client::inner::Inner;
use crate::error::Result;
use serde::Deserialize;
use std::sync::Arc;

/// Tooling API handler for Salesforce.
///
/// Provides access to developer tooling features like executing anonymous Apex.
#[derive(Debug)]
pub struct ToolingHandler<A: Authenticator> {
    inner: Arc<Inner<A>>,
}

impl<A: Authenticator> Clone for ToolingHandler<A> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<A: Authenticator> ToolingHandler<A> {
    /// Creates a new Tooling handler.
    #[must_use]
    pub(crate) fn new(inner: Arc<Inner<A>>) -> Self {
        Self { inner }
    }

    /// Executes a block of Apex code anonymously.
    ///
    /// This is equivalent to the "Execute Anonymous" feature in the Developer Console.
    ///
    /// # Arguments
    ///
    /// * `script` - The Apex code to execute.
    ///
    /// # Returns
    ///
    /// A result containing execution details (success status, compilation errors, etc.).
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails (network error, auth error).
    /// Note: Compilation errors or runtime exceptions in Apex are returned as `Ok(ExecuteAnonymousResult)`, not `Err`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let result = client.tooling().execute_anonymous("System.debug('Hello World');").await?;
    /// if result.success {
    ///     println!("Executed successfully!");
    /// } else {
    ///     println!("Error: {}", result.compile_problem.unwrap_or_default());
    /// }
    /// ```
    pub async fn execute_anonymous(&self, script: &str) -> Result<ExecuteAnonymousResult> {
        let api_version = &self.inner.config.api_version;
        let token = self.inner.token_manager.get_token_arc().await?;

        // Tooling API Execute Anonymous is a GET request with query param
        let url = format!(
            "{}/services/data/{}/tooling/executeAnonymous",
            token.instance_url(),
            api_version
        );

        let request = self
            .inner
            .http_client
            .get(&url)
            .query(&[("anonymousBody", script)])
            .build()
            .map_err(crate::error::HttpError::from)?;

        self.inner
            .send_request_and_decode(request, "Execute Anonymous failed")
            .await
    }
}

/// Result of an anonymous Apex execution.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteAnonymousResult {
    /// Line number where the error occurred (if any).
    pub line: i32,
    /// Column number where the error occurred (if any).
    pub column: i32,
    /// Whether the code compiled successfully.
    pub compiled: bool,
    /// Whether the execution was successful.
    pub success: bool,
    /// Compilation problem description (if any).
    pub compile_problem: Option<String>,
    /// Runtime exception message (if any).
    pub exception_message: Option<String>,
    /// Stack trace of the exception (if any).
    pub exception_stack_trace: Option<String>,
}

impl ExecuteAnonymousResult {
    /// Returns true if the execution was successful (compiled and ran without exception).
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.success
    }

    /// Returns the error message (compile problem or exception message).
    #[must_use]
    pub fn error_message(&self) -> Option<&str> {
        self.compile_problem
            .as_deref()
            .or(self.exception_message.as_deref())
    }
}
