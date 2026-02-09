//! Salesforce API client with compile-time authentication safety.
//!
//! This module provides the core `ForceClient` and builder types with phantom type
//! markers to ensure authentication is handled at compile-time.

mod builder;

pub use builder::{AuthenticatedBuilder, ForceClientBuilder, HasAuth, NoAuth};

use crate::auth::TokenManager;
use crate::config::ClientConfig;
use crate::http::HttpExecutor;
use crate::http::RequestRetryClass;
use std::sync::Arc;

/// Inner state shared across cloned clients.
///
/// This is generic over the authenticator type to avoid trait object overhead.
#[derive(Debug, Clone)]
pub(crate) struct Inner<A: crate::auth::Authenticator> {
    /// Client configuration.
    pub(crate) config: ClientConfig,
    /// HTTP client for making requests.
    pub(crate) http_client: reqwest::Client,
    /// Shared HTTP executor for auth/retry/timeout middleware.
    pub(crate) http_executor: HttpExecutor,
    /// Token manager for automatic token refresh (wrapped in Arc for cloning).
    pub(crate) token_manager: Arc<TokenManager<A>>,
}

impl<A: crate::auth::Authenticator> Inner<A> {
    /// Executes a request through the shared middleware pipeline.
    pub(crate) async fn execute_request(
        &self,
        request: reqwest::Request,
    ) -> crate::error::Result<reqwest::Response> {
        let token = self.token_manager.token().await?;
        let token_manager = Arc::clone(&self.token_manager);
        self.http_executor
            .execute_response(request, &token, move || {
                let token_manager = Arc::clone(&token_manager);
                async move { token_manager.force_refresh().await }
            })
            .await
    }

    /// Executes a request with an explicit retry class override.
    pub(crate) async fn execute_request_with_retry_class(
        &self,
        request: reqwest::Request,
        retry_class: RequestRetryClass,
    ) -> crate::error::Result<reqwest::Response> {
        let token = self.token_manager.token().await?;
        let token_manager = Arc::clone(&self.token_manager);
        self.http_executor
            .execute_response_with_retry_class(request, &token, move || {
                let token_manager = Arc::clone(&token_manager);
                async move { token_manager.force_refresh().await }
            }, retry_class)
            .await
    }
}

/// Salesforce API client with compile-time authentication safety.
///
/// This client uses phantom types to ensure authentication is required before
/// API calls can be made. Clients are cheaply cloneable via `Arc`.
///
/// The client is generic over the authenticator type for zero-cost abstraction.
#[derive(Debug)]
pub struct ForceClient<A: crate::auth::Authenticator> {
    inner: Arc<Inner<A>>,
}

impl<A: crate::auth::Authenticator> Clone for ForceClient<A> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

/// Public builder constructor (not tied to a specific authenticator).
#[must_use]
pub fn builder() -> ForceClientBuilder<NoAuth> {
    ForceClientBuilder::new()
}

impl<A: crate::auth::Authenticator> ForceClient<A> {
    /// Returns the client configuration.
    #[must_use]
    pub fn config(&self) -> &ClientConfig {
        &self.inner.config
    }

    /// Returns the current access token, refreshing if necessary.
    ///
    /// # Errors
    ///
    /// Returns an error if authentication or token refresh fails.
    pub async fn token(&self) -> crate::error::Result<crate::auth::AccessToken> {
        self.inner.token_manager.token().await
    }

    /// Returns a reference to the inner state (for internal use by handlers).
    ///
    /// # Internal API
    ///
    /// This method is internal to the crate and should not be used directly.
    #[must_use]
    pub(crate) fn inner(&self) -> &Arc<Inner<A>> {
        &self.inner
    }

    /// Creates a REST API handler for this client.
    ///
    /// The REST handler provides access to CRUD operations, queries, and metadata.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let client = builder().authenticate(auth).build().await?;
    /// let rest = client.rest();
    /// ```
    #[cfg(feature = "rest")]
    #[must_use]
    pub fn rest(&self) -> crate::api::rest::RestHandler<A> {
        crate::api::rest::RestHandler::new(Arc::clone(&self.inner))
    }

    /// Creates a Bulk API 2.0 handler for this client.
    ///
    /// The Bulk handler provides access to high-volume data operations and bulk queries.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let client = builder().authenticate(auth).build().await?;
    /// let bulk = client.bulk();
    /// ```
    #[cfg(feature = "bulk")]
    #[must_use]
    pub fn bulk(&self) -> crate::api::bulk::BulkHandler<A> {
        crate::api::bulk::BulkHandler::new(Arc::clone(&self.inner))
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_creates_noauth_state() {
        let _builder = builder();
        // Compile-time check: builder starts in NoAuth state
    }
}




