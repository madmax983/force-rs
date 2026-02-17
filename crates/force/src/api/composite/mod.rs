//! Composite API resources.
//!
//! The Composite resource is an enhanced REST API that allows you to execute
//! sequential and dependent requests in a single call.
//!
//! This module provides the `CompositeHandler` which serves as the entry point
//! for composite operations like `batch` and `graph`.

pub mod batch;

use crate::auth::Authenticator;
use crate::client::inner::Inner;
use std::sync::Arc;

/// Composite API handler for Salesforce.
///
/// Provides access to the Composite API resources.
#[derive(Debug)]
pub struct CompositeHandler<A: Authenticator> {
    inner: Arc<Inner<A>>,
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
    pub(crate) fn new(inner: Arc<Inner<A>>) -> Self {
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

    /// Helper to get the API version from config.
    pub(crate) fn api_version(&self) -> &str {
        &self.inner.config.api_version
    }
}
