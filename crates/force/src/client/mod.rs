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
    /// Optional Data Cloud session (created when `.with_data_cloud()` is called on the builder).
    #[cfg(feature = "data_cloud")]
    dc_session: Option<Arc<Session<crate::auth::DataCloudAuthenticator<A>>>>,
}

impl<A: crate::auth::authenticator::Authenticator> Clone for ForceClient<A> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            #[cfg(feature = "data_cloud")]
            dc_session: self.dc_session.clone(),
        }
    }
}

/// Public builder constructor (not tied to a specific authenticator).
#[must_use]
pub fn builder() -> ForceClientBuilder<NoAuth> {
    ForceClientBuilder::new()
}

/// Generates a handler accessor method on `ForceClient`.
///
/// Each handler is a lightweight wrapper around `Arc<Session<A>>`.
/// This macro eliminates copy-paste while preserving per-method doc
/// comments and feature gates.
macro_rules! handler_accessor {
    (
        $(#[$meta:meta])*
        pub fn $name:ident -> $Handler:ty
    ) => {
        $(#[$meta])*
        #[must_use]
        pub fn $name(&self) -> $Handler {
            <$Handler>::new(Arc::clone(&self.inner))
        }
    };
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

    /// Returns the shared session state.
    ///
    /// Used by API handlers internally and by extension crates like `force-pubsub`
    /// to access the same authentication and HTTP state without duplicating auth logic.
    #[must_use]
    pub fn session(&self) -> Arc<Session<A>> {
        Arc::clone(&self.inner)
    }

    handler_accessor! {
        /// Creates a REST API handler for this client.
        ///
        /// Provides CRUD operations, queries, search, describe, and limits.
        #[cfg(feature = "rest")]
        pub fn rest -> crate::api::rest::RestHandler<A>
    }

    handler_accessor! {
        /// Creates a Bulk API 2.0 handler for this client.
        ///
        /// Provides high-volume data operations and bulk queries.
        #[cfg(feature = "bulk")]
        pub fn bulk -> crate::api::bulk::BulkHandler<A>
    }

    handler_accessor! {
        /// Creates a Tooling API handler for this client.
        ///
        /// Provides CRUD, queries, and metadata for development objects.
        #[cfg(feature = "tooling")]
        pub fn tooling -> crate::api::tooling::ToolingHandler<A>
    }

    handler_accessor! {
        /// Creates a Composite API handler for this client.
        ///
        /// Provides batch and graph operations.
        #[cfg(feature = "composite")]
        pub fn composite -> crate::api::composite::CompositeHandler<A>
    }

    handler_accessor! {
        /// Creates a UI API handler for this client.
        ///
        /// Provides layout-aware records, object metadata, list views,
        /// actions, lookups, and favorites.
        #[cfg(feature = "ui")]
        pub fn ui -> crate::api::ui::UiHandler<A>
    }

    handler_accessor! {
        /// Creates a GraphQL API handler for this client.
        ///
        /// Provides queries and mutations via a single POST endpoint.
        #[cfg(feature = "graphql")]
        pub fn graphql -> crate::api::graphql::GraphqlHandler<A>
    }

    handler_accessor! {
        /// Creates an Apex REST API handler for this client.
        ///
        /// Provides generic HTTP access to custom `/services/apexrest/` endpoints.
        #[cfg(feature = "apex_rest")]
        pub fn apex_rest -> crate::api::apex_rest::ApexRestHandler<A>
    }

    handler_accessor! {
        /// Creates a CPQ API handler for this client.
        ///
        /// Provides typed access to the Salesforce CPQ ServiceRouter.
        #[cfg(feature = "cpq")]
        pub fn cpq -> crate::api::cpq::CpqHandler<A>
    }

    handler_accessor! {
        /// Creates a Consent & Portability API handler for this client.
        ///
        /// Provides consent status checks and GDPR/CCPA data portability.
        #[cfg(feature = "consent")]
        pub fn consent -> crate::api::consent::ConsentHandler<A>
    }

    /// Creates a Data Cloud API handler for this client.
    ///
    /// The Data Cloud handler provides access to the Salesforce Data Cloud
    /// REST Connect API, which uses a separate tenant endpoint and
    /// token exchange flow.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::MissingValue` if the client was not built with
    /// `.with_data_cloud()`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use force::auth::DataCloudConfig;
    ///
    /// let client = builder()
    ///     .authenticate(auth)
    ///     .with_data_cloud(DataCloudConfig::default())
    ///     .build()
    ///     .await?;
    ///
    /// let dc = client.data_cloud()?;
    /// ```
    #[cfg(feature = "data_cloud")]
    pub fn data_cloud(&self) -> crate::error::Result<crate::api::data_cloud::DataCloudHandler<A>> {
        let dc = self.dc_session.as_ref().ok_or_else(|| {
            crate::error::ForceError::Config(crate::error::ConfigError::MissingValue(
                "Data Cloud not configured — call .with_data_cloud() on the builder".into(),
            ))
        })?;
        Ok(crate::api::data_cloud::DataCloudHandler::new(Arc::clone(
            dc,
        )))
    }

    handler_accessor! {
        /// Creates a Files API handler for this client.
        ///
        /// Provides capabilities to upload, download, and link ContentVersions.
        #[cfg(feature = "files")]
        pub fn files -> crate::api::files::FilesHandler<A>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;

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
        let s1 = client.session();
        let s2 = cloned_client.session();
        assert!(Arc::ptr_eq(&s1, &s2));
    }
}
