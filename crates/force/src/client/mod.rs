//! Salesforce API client with compile-time authentication safety.
//!
//! This module provides the core `ForceClient` and builder types with phantom type
//! markers to ensure authentication is handled at compile-time.

mod builder;

pub use builder::{AuthenticatedBuilder, ForceClientBuilder, HasAuth, NoAuth};

use crate::auth::TokenManager;
use crate::config::ClientConfig;
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
    /// Token manager for automatic token refresh (wrapped in Arc for cloning).
    pub(crate) token_manager: Arc<TokenManager<A>>,
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
