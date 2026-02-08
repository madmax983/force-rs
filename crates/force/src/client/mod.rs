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
struct Inner<A: crate::auth::Authenticator> {
    /// Client configuration.
    config: ClientConfig,
    /// HTTP client for making requests.
    http_client: reqwest::Client,
    /// Token manager for automatic token refresh (wrapped in Arc for cloning).
    token_manager: Arc<TokenManager<A>>,
}

/// Salesforce API client with compile-time authentication safety.
///
/// This client uses phantom types to ensure authentication is required before
/// API calls can be made. Clients are cheaply cloneable via `Arc`.
///
/// The client is generic over the authenticator type for zero-cost abstraction.
#[derive(Debug, Clone)]
pub struct ForceClient<A: crate::auth::Authenticator> {
    inner: Arc<Inner<A>>,
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
