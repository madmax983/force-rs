//! Core REST handler implementation.
//!
//! This module provides the `RestHandler` struct and its base methods.
//! Additional functionality is implemented in other modules.

use super::query_stream;
use crate::error::Result;
use serde::de::DeserializeOwned;
use std::sync::Arc;

/// REST API handler for performing Salesforce REST operations.
///
/// The handler provides access to all REST API functionality including:
/// - CRUD operations (Create, Read, Update, Delete, Upsert)
/// - SOQL queries with pagination
/// - SOSL searches
/// - Metadata operations (Describe API)
/// - Org limits
///
/// The handler is obtained from a `ForceClient` and shares its authentication
/// and configuration.
#[derive(Debug)]
pub struct RestHandler<A: crate::auth::Authenticator> {
    /// Reference to the client's inner state.
    pub(crate) inner: Arc<crate::session::Session<A>>,
}

impl<A: crate::auth::Authenticator> Clone for RestHandler<A> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<A: crate::auth::Authenticator> RestHandler<A> {
    /// Creates a new REST handler for the given client inner state.
    ///
    /// # Arguments
    ///
    /// * `inner` - The client's inner state
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let handler = RestHandler::new(inner);
    /// ```
    #[must_use]
    pub(crate) fn new(inner: Arc<crate::session::Session<A>>) -> Self {
        Self { inner }
    }

    /// Creates a stream of query results for the given SOQL.
    ///
    /// This method simplifies paginated queries by returning a stream that automatically
    /// fetches subsequent pages of results as needed.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let stream = client.rest().query_stream::<Account>("SELECT Id FROM Account");
    /// ```
    pub fn query_stream<T>(&self, soql: impl Into<String>) -> query_stream::QueryStream<T, A>
    where
        T: DeserializeOwned + Unpin,
    {
        query_stream::QueryStream::new(self.clone(), soql)
    }

    /// Constructs the base URL for REST API operations.
    ///
    /// The base URL is constructed as: `{instance_url}/services/data/{api_version}`
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
    /// // Returns: "https://na1.salesforce.com/services/data/v60.0"
    /// ```
    pub async fn base_url(&self) -> Result<String> {
        self.inner.resolve_url("").await
    }

    /// Helper method to execute a GET request and deserialize the response.
    pub(crate) async fn execute_get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: Option<&[(&str, &str)]>,
        error_msg: &str,
    ) -> Result<T> {
        let url = self.inner.resolve_url(path).await?;
        let mut request = self.inner.get(&url);

        if let Some(params) = query {
            request = request.query(params);
        }

        let request = request.build().map_err(crate::error::HttpError::from)?;
        self.inner.send_request_and_decode(request, error_msg).await
    }

    /// Helper method to execute a POST request and deserialize the response.
    pub(crate) async fn execute_post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &(impl serde::Serialize + Send + Sync),
        error_msg: &str,
    ) -> Result<T> {
        let url = self.inner.resolve_url(path).await?;
        let request = self
            .inner
            .post(&url)
            .json(body)
            .build()
            .map_err(crate::error::HttpError::from)?;
        self.inner.send_request_and_decode(request, error_msg).await
    }

    /// Helper method to execute a PATCH request and expect an empty success response.
    pub(crate) async fn execute_patch_empty(
        &self,
        path: &str,
        body: &(impl serde::Serialize + Send + Sync),
        error_msg: &str,
    ) -> Result<()> {
        let url = self.inner.resolve_url(path).await?;
        let request = self
            .inner
            .patch(&url)
            .json(body)
            .build()
            .map_err(crate::error::HttpError::from)?;
        let response = self.inner.execute_request(request).await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(crate::http::response_to_force_error(response, error_msg).await)
        }
    }

    /// Helper method to execute a DELETE request and expect an empty success response.
    pub(crate) async fn execute_delete_empty(&self, path: &str, error_msg: &str) -> Result<()> {
        let url = self.inner.resolve_url(path).await?;
        let request = self
            .inner
            .delete(&url)
            .build()
            .map_err(crate::error::HttpError::from)?;
        let response = self.inner.execute_request(request).await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(crate::http::response_to_force_error(response, error_msg).await)
        }
    }
}
#[cfg(test)]
mod tests {
    use crate::client::{ForceClient, builder};
    use crate::config::ClientConfigBuilder;
    use crate::test_support::{MockAuthenticator, Must, MustMsg};

    async fn create_test_client() -> ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", "https://test.salesforce.com");
        builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("failed to create test client")
    }

    #[tokio::test]
    async fn test_rest_handler_construction() {
        let client: ForceClient<MockAuthenticator> = create_test_client().await;
        let _handler = client.rest();
        // If we get here, handler was created successfully
    }

    #[tokio::test]
    async fn test_rest_handler_is_cloneable() {
        let client: ForceClient<MockAuthenticator> = create_test_client().await;
        let handler1 = client.rest();
        let handler2 = handler1.clone();

        // Both should produce the same base URL
        let url1: String = handler1.base_url().await.must();
        let url2: String = handler2.base_url().await.must();
        assert_eq!(url1, url2);
    }

    #[tokio::test]
    async fn test_base_url_construction() {
        let client: ForceClient<MockAuthenticator> = create_test_client().await;
        let handler = client.rest();

        let base_url: String = handler.base_url().await.must();
        assert!(base_url.starts_with("https://test.salesforce.com"));
        assert!(base_url.contains("/services/data/"));
        assert!(base_url.ends_with("v60.0")); // Default API version
    }

    #[tokio::test]
    async fn test_base_url_with_custom_api_version() {
        let auth = MockAuthenticator::new("test_token", "https://custom.salesforce.com");
        let config = ClientConfigBuilder::new().api_version("v59.0").build();
        let client = builder()
            .authenticate(auth)
            .config(config)
            .build()
            .await
            .must();

        let handler = client.rest();
        let base_url = handler.base_url().await.must();

        assert_eq!(
            base_url,
            "https://custom.salesforce.com/services/data/v59.0"
        );
    }

    #[tokio::test]
    async fn test_base_url_with_different_instance() {
        let auth = MockAuthenticator::new("token", "https://na139.salesforce.com");
        let client = builder().authenticate(auth).build().await.must();

        let handler = client.rest();
        let base_url = handler.base_url().await.must();

        assert!(base_url.starts_with("https://na139.salesforce.com"));
    }

    #[tokio::test]
    async fn test_rest_handler_shares_client_config() {
        let auth = MockAuthenticator::new("token", "https://shared.salesforce.com");
        let config = ClientConfigBuilder::new().api_version("v58.0").build();
        let client = builder()
            .authenticate(auth)
            .config(config)
            .build()
            .await
            .must();

        let handler = client.rest();
        let base_url = handler.base_url().await.must();

        // Verify the handler uses the same config as the client
        assert!(base_url.contains("shared.salesforce.com"));
        assert!(base_url.contains("v58.0"));
    }

    #[tokio::test]
    async fn test_multiple_handlers_from_same_client() {
        let client: ForceClient<MockAuthenticator> = create_test_client().await;

        let handler1 = client.rest();
        let handler2 = client.rest();

        // Both should have the same base URL
        let url1: String = handler1.base_url().await.must();
        let url2: String = handler2.base_url().await.must();
        assert_eq!(url1, url2);
    }

    #[tokio::test]
    async fn test_handler_debug_impl() {
        let client: ForceClient<MockAuthenticator> = create_test_client().await;
        let handler = client.rest();

        let debug_str = format!("{:?}", handler);
        assert!(!debug_str.is_empty());
    }
}
