//! Composite API resources.
//!
//! The Composite resource is an enhanced REST API that allows you to execute
//! sequential and dependent requests in a single call.
//!
//! This module provides the `CompositeHandler` which serves as the entry point
//! for composite operations like `batch` and `graph`.

pub mod batch;
#[cfg(feature = "nova")]
pub mod graph;

use crate::auth::Authenticator;
use crate::session::Session;
use std::sync::Arc;

/// Composite API handler for Salesforce.
///
/// Provides access to the Composite API resources.
#[derive(Debug)]
pub struct CompositeHandler<A: Authenticator> {
    pub(crate) inner: Arc<Session<A>>,
}

impl<A: Authenticator> Clone for CompositeHandler<A> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<A: Authenticator> CompositeHandler<A> {
    /// Creates a new Composite handler.
    #[must_use]
    pub(crate) fn new(inner: Arc<Session<A>>) -> Self {
        Self { inner }
    }

    /// Creates a new batch request builder.
    ///
    /// A batch request can contain up to 25 subrequests. Subrequests are independent
    /// and can be of different types (GET, POST, PATCH, DELETE).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let batch = client.composite().batch();
    /// batch.get("Account", "001...");
    /// batch.post("Contact", json!({...}));
    /// let results = batch.execute().await?;
    /// ```
    #[must_use]
    pub fn batch(&self) -> batch::BatchBuilder<A> {
        batch::BatchBuilder::new(self.clone())
    }

    /// Creates a new graph request builder.
    ///
    /// The Composite Graph API allows you to execute complex, dependent requests
    /// (up to 500 nodes) in a single call.
    #[cfg(feature = "nova")]
    #[must_use]
    pub fn graph(&self) -> graph::GraphBuilder<A> {
        graph::GraphBuilder::new(self.clone())
    }

    /// Helper to get the API version from config.
    pub(crate) fn api_version(&self) -> &str {
        &self.inner.config.api_version
    }

    /// Constructs the base URL for Composite API operations.
    ///
    /// The base URL is constructed as: `{instance_url}/services/data/{api_version}`
    ///
    /// This method requires token access to get the instance URL from authentication.
    pub async fn base_url(&self) -> crate::error::Result<String> {
        self.inner.resolve_url("").await
    }
}
