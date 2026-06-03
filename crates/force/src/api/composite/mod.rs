//! Composite API resources.
//!
//! The Composite resource is an enhanced REST API that allows you to execute
//! sequential and dependent requests in a single call.
//!
//! This module provides the `CompositeHandler` which serves as the entry point
//! for composite operations like `batch` and `graph`.

pub(crate) mod batch;
#[cfg(feature = "composite_graph")]
pub(crate) mod graph;
#[cfg(feature = "composite")]
pub(crate) mod query_batch;
#[cfg(feature = "composite")]
pub(crate) mod soql_mass_op;

pub use batch::{BatchRequest, BatchResponse, BatchSubResponse};
#[cfg(feature = "composite_graph")]
pub use graph::{
    CompositeGraphRequest, Graph, GraphErrorResponse, GraphRequest, GraphResponse, GraphResult,
    GraphSubResponse,
};
#[cfg(feature = "composite")]
pub use query_batch::{BatchOp, BatchStats, QueryBatch};
#[cfg(feature = "composite")]
pub use soql_mass_op::SoqlMassOp;

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

    /// Creates a new batch request.
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
    pub fn batch(&self) -> batch::BatchRequest<A> {
        batch::BatchRequest::new(self.clone())
    }

    /// Creates a new graph request.
    ///
    /// The Composite Graph API allows you to execute complex, dependent requests
    /// (up to 500 nodes) in a single call.
    #[cfg(feature = "composite_graph")]
    #[must_use]
    pub fn graph(&self) -> graph::CompositeGraphRequest<A> {
        graph::CompositeGraphRequest::new(self.clone())
    }

    /// Creates a new query batch processor.
    #[cfg(feature = "composite")]
    #[must_use]
    pub fn query_batch(&self, query: impl Into<String>) -> QueryBatch<A> {
        QueryBatch::new(self.inner.clone(), query)
    }

    /// Creates a new SOQL mass operations processor.
    #[cfg(feature = "composite")]
    #[must_use]
    pub fn soql_mass_op(&self, query: crate::api::soql::SoqlQueryBuilder) -> SoqlMassOp<A> {
        SoqlMassOp::new(self.inner.clone(), query)
    }
}
