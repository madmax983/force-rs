//! Salesforce Consent & Portability API handler.
//!
//! Provides consent status checks and GDPR/CCPA data portability requests
//! via the `/consent/` and `/portability/` REST endpoints.
//!
//! # Feature Flag
//!
//! This module requires the `consent` feature flag:
//! ```toml
//! [dependencies]
//! force = { version = "...", features = ["consent"] }
//! ```
//!
//! # Usage
//!
//! ```ignore
//! let client = builder().authenticate(auth).build().await?;
//! let consent = client.consent();
//!
//! // Check email consent for multiple records
//! let result = consent.read_consent("email", &["001xx000003GYk1"]).await?;
//!
//! // Request data portability export
//! let req = PortabilityRequest::new("Contact", vec!["003xx000003GYk1".into()]);
//! let resp = consent.request_portability(&req).await?;
//! ```

pub(crate) mod action;
pub(crate) mod portability;
pub(crate) mod types;

// Re-export primary types at module level.
pub use types::{
    ConsentRecord, ConsentResponse, ConsentResult, ConsentValue, PortabilityRequest,
    PortabilityResponse, PortabilityStatus,
};

use crate::error::Result;
use std::sync::Arc;

/// Consent & Portability API handler.
///
/// Provides access to consent status checks and GDPR/CCPA data portability
/// requests. Consent writes (create/update consent records) are handled via
/// standard sObject CRUD through [`RestHandler`](crate::api::rest::RestHandler).
///
/// Obtain a handler from [`ForceClient::consent`](crate::client::ForceClient::consent).
#[derive(Debug)]
pub struct ConsentHandler<A: crate::auth::Authenticator> {
    inner: Arc<crate::session::Session<A>>,
}

impl<A: crate::auth::Authenticator> Clone for ConsentHandler<A> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<A: crate::auth::Authenticator> ConsentHandler<A> {
    /// Creates a new Consent handler wrapping the given session.
    #[must_use]
    pub(crate) fn new(inner: Arc<crate::session::Session<A>>) -> Self {
        Self { inner }
    }

    /// Resolves a consent API path to a full URL.
    ///
    /// Constructs: `{instance_url}/services/data/{version}/consent/{path}`
    pub(crate) async fn resolve_consent_url(&self, path: &str) -> Result<String> {
        let clean = path.trim_start_matches('/');
        if clean.is_empty() {
            self.inner.resolve_url("consent").await
        } else {
            self.inner.resolve_url(&format!("consent/{clean}")).await
        }
    }

    /// Resolves a portability API path to a full URL.
    ///
    /// Constructs: `{instance_url}/services/data/{version}/portability/{path}`
    pub(crate) async fn resolve_portability_url(&self, path: &str) -> Result<String> {
        let clean = path.trim_start_matches('/');
        if clean.is_empty() {
            self.inner.resolve_url("portability").await
        } else {
            self.inner
                .resolve_url(&format!("portability/{clean}"))
                .await
        }
    }
}
