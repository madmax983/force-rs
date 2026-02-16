//! Salesforce API client with compile-time authentication safety.
//!
//! This module provides the core `ForceClient` and builder types with phantom type
//! markers to ensure authentication is handled at compile-time.

mod builder;
pub(crate) mod inner;

pub use builder::{AuthenticatedBuilder, ForceClientBuilder, HasAuth, NoAuth};
pub(crate) use inner::Inner;

use crate::config::ClientConfig;
use std::sync::Arc;

/// Salesforce API client with compile-time authentication safety.
///
/// This client uses phantom types to ensure authentication is required before
/// API calls can be made. Clients are cheaply cloneable via `Arc`.
///
/// The client is generic over the authenticator type for zero-cost abstraction.
#[derive(Debug)]
pub struct ForceClient<A: crate::types::authenticator::Authenticator> {
    inner: Arc<Inner<A>>,
}

impl<A: crate::types::authenticator::Authenticator> Clone for ForceClient<A> {
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

impl<A: crate::types::authenticator::Authenticator> ForceClient<A> {
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
    pub async fn token(&self) -> crate::error::Result<crate::types::token::AccessToken> {
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
