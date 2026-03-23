//! Salesforce CPQ API handler.
//!
//! The CPQ (Configure, Price, Quote) API routes through the Salesforce
//! ServiceRouter at `/services/apexrest/SBQQ/ServiceRouter`. All operations
//! are dispatched via a `loader` query parameter that selects the API class.
//!
//! # Feature Flag
//!
//! This module requires the `cpq` feature flag:
//! ```toml
//! [dependencies]
//! force = { version = "...", features = ["cpq"] }
//! ```
//!
//! # Usage
//!
//! ```ignore
//! let client = builder().authenticate(auth).build().await?;
//! let cpq = client.cpq();
//!
//! // Read a quote
//! let quote = cpq.read_quote("a0x000000000001AAA").await?;
//!
//! // Calculate pricing
//! let calculated = cpq.calculate_quote(&quote).await?;
//!
//! // Save the quote
//! let saved = cpq.save_quote(&calculated).await?;
//! ```

pub(crate) mod config;
pub(crate) mod contract;
pub(crate) mod document;
pub(crate) mod error;
pub(crate) mod product;
pub(crate) mod quote;
pub(crate) mod types;

// Re-export primary types at module level.
pub use error::CpqErrorResponse;
pub use types::{
    AddProductsRequest, ConfigurationModel, GenerateDocumentRequest, ProductModel, QuoteLineModel,
    QuoteModel, ServiceRouterRequest,
};

use crate::error::{HttpError, Result};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::sync::Arc;

/// Salesforce CPQ API handler.
///
/// Provides typed access to the CPQ ServiceRouter for quote lifecycle,
/// product configuration, document generation, and contract amendment.
///
/// Obtain a handler from [`ForceClient::cpq`](crate::client::ForceClient::cpq).
#[derive(Debug)]
pub struct CpqHandler<A: crate::auth::Authenticator> {
    inner: Arc<crate::session::Session<A>>,
}

impl<A: crate::auth::Authenticator> Clone for CpqHandler<A> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

/// The ServiceRouter Apex REST endpoint path.
const SERVICE_ROUTER: &str = "SBQQ/ServiceRouter";

impl<A: crate::auth::Authenticator> CpqHandler<A> {
    /// Creates a new CPQ handler wrapping the given session.
    #[must_use]
    pub(crate) fn new(inner: Arc<crate::session::Session<A>>) -> Self {
        Self { inner }
    }

    /// POST to the ServiceRouter with a `loader` query parameter.
    ///
    /// This is the primary dispatch mechanism for most CPQ operations.
    pub(crate) async fn service_router_post<T: DeserializeOwned>(
        &self,
        loader: &str,
        body: &(impl Serialize + Sync),
    ) -> Result<T> {
        let url = self.inner.resolve_apex_rest_url(SERVICE_ROUTER).await?;
        let request = self
            .inner
            .post(&url)
            .query(&[("loader", loader)])
            .json(body)
            .build()
            .map_err(HttpError::from)?;
        self.inner
            .send_request_and_decode(request, &format!("CPQ {loader} failed"))
            .await
    }

    /// PATCH to the ServiceRouter with a `loader` query parameter.
    ///
    /// Used by `QuoteCalculator` which requires PATCH instead of POST.
    pub(crate) async fn service_router_patch<T: DeserializeOwned>(
        &self,
        loader: &str,
        body: &(impl Serialize + Sync),
    ) -> Result<T> {
        let url = self.inner.resolve_apex_rest_url(SERVICE_ROUTER).await?;
        let request = self
            .inner
            .patch(&url)
            .query(&[("loader", loader)])
            .json(body)
            .build()
            .map_err(HttpError::from)?;
        self.inner
            .send_request_and_decode(request, &format!("CPQ {loader} failed"))
            .await
    }
}
