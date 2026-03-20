//! Tooling API handler for Salesforce.
//!
//! The Tooling API provides REST-style access to metadata types like `ApexClass`,
//! `ApexTrigger`, and others. It supports the same CRUD, Query, and Describe
//! operations as the REST API, plus Tooling-specific endpoints.
//!
//! # URL Routing
//!
//! All requests are routed under the `/tooling/` prefix:
//! - `tooling/sobjects/ApexClass` (CRUD)
//! - `tooling/query?q=SELECT ...` (SOQL)
//! - `tooling/sobjects` (Describe Global)

use crate::api::rest_operation::RestOperation;
use std::sync::Arc;

/// Tooling API handler for Salesforce development operations.
///
/// Provides access to all Tooling API functionality including:
/// - CRUD operations on tooling objects (`ApexClass`, `ApexTrigger`, etc.)
/// - SOQL queries against tooling objects
/// - Metadata describe operations
/// - Execute anonymous Apex (future)
/// - Run tests (future)
///
/// The handler is obtained from a [`ForceClient`](crate::client::ForceClient)
/// and shares its authentication and configuration.
///
/// # Examples
///
/// ```ignore
/// use force::api::rest_operation::RestOperation;
///
/// let client = builder().authenticate(auth).build().await?;
/// let result = client.tooling()
///     .query::<serde_json::Value>("SELECT Id, Name FROM ApexClass")
///     .await?;
/// ```
#[derive(Debug)]
pub struct ToolingHandler<A: crate::auth::Authenticator> {
    inner: Arc<crate::session::Session<A>>,
}

impl<A: crate::auth::Authenticator> Clone for ToolingHandler<A> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<A: crate::auth::Authenticator> ToolingHandler<A> {
    /// Creates a new Tooling API handler for the given session state.
    ///
    /// # Arguments
    ///
    /// * `inner` - The shared session state containing the HTTP client,
    ///   token manager, and configuration.
    #[must_use]
    pub(crate) fn new(inner: Arc<crate::session::Session<A>>) -> Self {
        Self { inner }
    }
}

impl<A: crate::auth::Authenticator> RestOperation<A> for ToolingHandler<A> {
    fn session(&self) -> &Arc<crate::session::Session<A>> {
        &self.inner
    }

    #[allow(clippy::unnecessary_literal_bound)]
    fn path_prefix(&self) -> &str {
        "tooling"
    }
}

#[cfg(test)]
mod tests {
    use crate::api::rest_operation::RestOperation;
    use crate::client::{ForceClient, builder};
    use crate::test_support::{MockAuthenticator, MustMsg};

    async fn create_test_client() -> ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", "https://test.salesforce.com");
        builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("failed to create test client")
    }

    #[tokio::test]
    async fn test_tooling_handler_construction() {
        let client = create_test_client().await;
        let _handler = client.tooling();
    }

    #[tokio::test]
    async fn test_tooling_handler_is_cloneable() {
        let client = create_test_client().await;
        let h1 = client.tooling();
        let _h2 = h1.clone();
    }

    #[tokio::test]
    async fn test_tooling_path_prefix() {
        let client = create_test_client().await;
        let handler = client.tooling();
        assert_eq!(handler.path_prefix(), "tooling");
    }

    #[tokio::test]
    async fn test_tooling_handler_debug() {
        let client = create_test_client().await;
        let handler = client.tooling();
        let debug = format!("{:?}", handler);
        assert!(!debug.is_empty());
    }

    #[tokio::test]
    async fn test_tooling_resolve_api_path_sobjects() {
        let client = create_test_client().await;
        let handler = client.tooling();
        assert_eq!(
            handler.resolve_api_path("sobjects/ApexClass"),
            "tooling/sobjects/ApexClass"
        );
    }

    #[tokio::test]
    async fn test_tooling_resolve_api_path_query() {
        let client = create_test_client().await;
        let handler = client.tooling();
        assert_eq!(handler.resolve_api_path("query"), "tooling/query");
    }

    #[tokio::test]
    async fn test_tooling_resolve_api_path_describe() {
        let client = create_test_client().await;
        let handler = client.tooling();
        assert_eq!(handler.resolve_api_path("sobjects"), "tooling/sobjects");
    }
}
