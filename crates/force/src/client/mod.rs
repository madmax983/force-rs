//! Salesforce API client.
//!
//! This module provides the core `ForceClient` and builder types.

mod builder;

pub use builder::{AuthenticatedBuilder, ForceClientBuilder, HasAuth, NoAuth};

use crate::config::ClientConfig;
use crate::session::Session;
use std::sync::Arc;

/// Salesforce API client.
///
/// The client handles authentication and provides access to API handlers.
/// Clients are cheaply cloneable via `Arc`.
#[derive(Debug)]
pub struct ForceClient<A: crate::auth::authenticator::Authenticator> {
    inner: Arc<Session<A>>,
}

impl<A: crate::auth::authenticator::Authenticator> Clone for ForceClient<A> {
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

impl<A: crate::auth::authenticator::Authenticator> ForceClient<A> {
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
    pub async fn token(&self) -> crate::error::Result<crate::auth::token::AccessToken> {
        self.inner.token_manager.token().await
    }

    /// Returns a reference to the inner state (for internal use by handlers).
    ///
    /// # Internal API
    ///
    /// This method is internal to the crate and should not be used directly.
    #[must_use]
    pub(crate) fn inner(&self) -> &Arc<Session<A>> {
        &self.inner
    }

    /// Returns the shared session state for use by extension crates.
    ///
    /// This allows extension crates like `force-pubsub` to access the same
    /// authentication and HTTP state without duplicating auth logic.
    #[must_use]
    pub fn session(&self) -> Arc<crate::session::Session<A>> {
        Arc::clone(&self.inner)
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

    /// Creates a Tooling API handler for this client.
    ///
    /// The Tooling handler provides access to CRUD operations, queries,
    /// and metadata for Salesforce development objects (`ApexClass`,
    /// `ApexTrigger`, etc.).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use force::api::rest_operation::RestOperation;
    ///
    /// let client = builder().authenticate(auth).build().await?;
    /// let tooling = client.tooling();
    /// ```
    #[cfg(feature = "tooling")]
    #[must_use]
    pub fn tooling(&self) -> crate::api::tooling::ToolingHandler<A> {
        crate::api::tooling::ToolingHandler::new(Arc::clone(&self.inner))
    }

    /// Creates a Composite API handler for this client.
    ///
    /// The Composite handler provides access to batch and graph operations.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let client = builder().authenticate(auth).build().await?;
    /// let composite = client.composite();
    /// ```
    #[cfg(feature = "composite")]
    #[must_use]
    pub fn composite(&self) -> crate::api::composite::CompositeHandler<A> {
        crate::api::composite::CompositeHandler::new(Arc::clone(&self.inner))
    }

    /// Creates a UI API handler for this client.
    ///
    /// The UI handler provides layout-aware record data, object metadata,
    /// list views, actions, lookups, and favorites.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let client = builder().authenticate(auth).build().await?;
    /// let ui = client.ui();
    /// let record_ui = ui.record_ui(&["001000000000001AAA"], None, None).await?;
    /// ```
    #[cfg(feature = "ui")]
    #[must_use]
    pub fn ui(&self) -> crate::api::ui::UiHandler<A> {
        crate::api::ui::UiHandler::new(Arc::clone(&self.inner))
    }

    /// Creates a GraphQL API handler for this client.
    ///
    /// The GraphQL handler provides access to the Salesforce GraphQL API,
    /// which supports queries and mutations via a single POST endpoint.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use force::api::graphql::GraphqlRequest;
    ///
    /// let client = builder().authenticate(auth).build().await?;
    /// let gql = client.graphql();
    /// let data = gql.query_raw("{ uiapi { query { Account { edges { node { Id } } } } } }", None).await?;
    /// ```
    #[cfg(feature = "graphql")]
    #[must_use]
    pub fn graphql(&self) -> crate::api::graphql::GraphqlHandler<A> {
        crate::api::graphql::GraphqlHandler::new(Arc::clone(&self.inner))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::MockAuthenticator;
    use crate::test_support::Must;

    #[test]
    fn test_builder_creates_noauth_state() {
        let _builder = builder();
        // Check that the builder is initialized properly.
    }

    #[tokio::test]
    async fn test_force_client_clone() {
        let auth = MockAuthenticator::new("test_token", "https://test.salesforce.com");
        let client = builder().authenticate(auth).build().await.must();

        let cloned_client = client.clone();

        // Assert that the cloned client points to the same underlying Arc
        assert!(Arc::ptr_eq(client.inner(), cloned_client.inner()));
    }
}
