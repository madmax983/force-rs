//! Bulk API handler implementation.
//!
//! This module provides the `BulkHandler` struct and its core methods.
//! Additional functionality is implemented in `ingest.rs` and `query.rs`.

use crate::error::Result;
use std::sync::Arc;

/// Bulk API 2.0 handler for performing Salesforce bulk operations.
///
/// The handler provides access to all Bulk API functionality including:
/// - Ingest jobs for high-volume data operations
/// - Bulk queries for retrieving large datasets
/// - Job state management
/// - CSV data upload/download
///
/// The handler is obtained from a `ForceClient` and shares its authentication
/// and configuration.
#[derive(Debug, Clone)]
pub struct BulkHandler<A: crate::auth::Authenticator> {
    /// Reference to the client's inner state.
    pub(crate) inner: Arc<crate::session::Session<A>>,
}

impl<A: crate::auth::Authenticator> BulkHandler<A> {
    /// Creates a new Bulk handler for the given client inner state.
    ///
    /// # Arguments
    ///
    /// * `inner` - The client's inner state
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let handler = BulkHandler::new(inner);
    /// ```
    #[must_use]
    pub(crate) fn new(inner: Arc<crate::session::Session<A>>) -> Self {
        Self { inner }
    }

    /// Constructs the base URL for Bulk API 2.0 operations.
    ///
    /// The base URL is constructed as: `{instance_url}/services/data/{api_version}/jobs/ingest`
    ///
    /// This method requires token access to get the instance URL from authentication.
    ///
    /// # Errors
    ///
    /// Returns an error if token retrieval fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let base = handler.base_url().await?;
    /// // Returns: "https://na1.salesforce.com/services/data/v67.0/jobs/ingest"
    /// ```
    pub async fn base_url(&self) -> Result<String> {
        self.inner.resolve_url("jobs/ingest").await
    }
}

#[cfg(test)]
mod tests {
    use crate::client::{ForceClient, builder};
    use crate::config::ClientConfig;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::{Must, MustMsg};
    use wiremock::MockServer;

    async fn create_test_client(mock_server_url: String) -> ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", &mock_server_url);
        builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("failed to create test client")
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_bulk_handler_construction() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(mock_server.uri()).await;
        let _handler = client.bulk();
        // If we get here, handler was created successfully
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_bulk_handler_is_cloneable() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(mock_server.uri()).await;
        let handler1 = client.bulk();
        let handler2 = handler1.clone();

        // Both should produce the same base URL
        let url1 = handler1.base_url().await.must();
        let url2 = handler2.base_url().await.must();
        assert_eq!(url1, url2);
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_base_url_construction() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(mock_server.uri()).await;
        let handler = client.bulk();

        let base_url = handler.base_url().await.must();
        assert!(base_url.contains(&mock_server.uri()));
        assert!(base_url.contains("/services/data/"));
        assert!(base_url.ends_with("v67.0/jobs/ingest")); // Default API version
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_base_url_with_custom_api_version() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let config = ClientConfig {
            api_version: "v59.0".parse().expect("valid version"),
            ..Default::default()
        };
        let client = builder()
            .authenticate(auth)
            .config(config)
            .build()
            .await
            .must();

        let handler = client.bulk();
        let base_url = handler.base_url().await.must();

        assert!(base_url.ends_with("v59.0/jobs/ingest"));
    }
}
