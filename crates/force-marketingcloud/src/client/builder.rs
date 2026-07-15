//! Builder for [`MarketingCloudClient`].

use crate::auth::{InstalledPackageCredentials, TokenManager};
use crate::client::MarketingCloudClient;
use crate::error::{MarketingCloudError, Result};
use secrecy::SecretString;
use std::sync::Arc;

/// Fluent builder for a [`MarketingCloudClient`].
///
/// A tenant subdomain (or an explicit auth URL) plus client credentials are
/// required. `build` is synchronous and does not perform any network I/O;
/// authentication happens lazily on the first API call.
#[derive(Debug, Default)]
pub struct MarketingCloudClientBuilder {
    /// Per-tenant subdomain used to derive the auth host.
    tenant_subdomain: Option<String>,

    /// Installed Package client id.
    client_id: Option<String>,

    /// Installed Package client secret.
    client_secret: Option<SecretString>,

    /// Default business unit (MID).
    account_id: Option<String>,

    /// Optional space-separated scope subset.
    scope: Option<String>,

    /// Optional injected HTTP client (for timeouts / testing).
    http_client: Option<reqwest::Client>,

    /// Explicit `v2/token` URL override (primarily for tests).
    auth_url: Option<String>,
}

impl MarketingCloudClientBuilder {
    /// Creates an empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the tenant subdomain (the per-org string shared by every MC host).
    #[must_use]
    pub fn tenant_subdomain(mut self, subdomain: impl Into<String>) -> Self {
        self.tenant_subdomain = Some(subdomain.into());
        self
    }

    /// Sets the Installed Package client id and secret.
    #[must_use]
    pub fn client_credentials(
        mut self,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
    ) -> Self {
        self.client_id = Some(client_id.into());
        self.client_secret = Some(SecretString::new(client_secret.into().into()));
        self
    }

    /// Sets the default business unit (MID) for token requests.
    #[must_use]
    pub fn account_id(mut self, account_id: impl Into<String>) -> Self {
        self.account_id = Some(account_id.into());
        self
    }

    /// Sets an optional space-separated scope subset.
    #[must_use]
    pub fn scope(mut self, scope: impl Into<String>) -> Self {
        self.scope = Some(scope.into());
        self
    }

    /// Injects a custom [`reqwest::Client`] (timeouts, proxies, test hooks).
    #[must_use]
    pub fn http_client(mut self, client: reqwest::Client) -> Self {
        self.http_client = Some(client);
        self
    }

    /// Overrides the full `v2/token` URL.
    ///
    /// Primarily a testing hook so the auth endpoint can be pointed at a mock
    /// server; in production the URL is derived from the tenant subdomain.
    #[doc(hidden)]
    #[must_use]
    pub fn auth_url(mut self, url: impl Into<String>) -> Self {
        self.auth_url = Some(url.into());
        self
    }

    /// Builds the client, validating required configuration.
    ///
    /// # Errors
    ///
    /// Returns [`MarketingCloudError::Configuration`] if credentials are missing
    /// or neither a tenant subdomain nor an explicit auth URL was provided.
    pub fn build(self) -> Result<MarketingCloudClient> {
        let client_id = self
            .client_id
            .ok_or_else(|| MarketingCloudError::Configuration("missing client_id".to_string()))?;
        let client_secret = self.client_secret.ok_or_else(|| {
            MarketingCloudError::Configuration("missing client_secret".to_string())
        })?;

        let token_url = if let Some(url) = self.auth_url {
            url
        } else {
            let subdomain = self.tenant_subdomain.as_deref().ok_or_else(|| {
                MarketingCloudError::Configuration(
                    "missing tenant_subdomain (or auth_url override)".to_string(),
                )
            })?;
            if subdomain.trim().is_empty() {
                return Err(MarketingCloudError::Configuration(
                    "tenant_subdomain must not be empty".to_string(),
                ));
            }
            format!("https://{subdomain}.auth.marketingcloudapis.com/v2/token")
        };

        let http = self.http_client.unwrap_or_default();

        let credentials = InstalledPackageCredentials::new(
            client_id,
            client_secret,
            token_url,
            self.account_id,
            self.scope,
            http.clone(),
        );

        let token_manager = Arc::new(TokenManager::new(Arc::new(credentials)));
        Ok(MarketingCloudClient::from_parts(token_manager, http))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn build_requires_client_credentials() {
        let err = MarketingCloudClientBuilder::new()
            .tenant_subdomain("sub")
            .build()
            .unwrap_err();
        assert!(matches!(err, MarketingCloudError::Configuration(_)));
    }

    #[test]
    fn build_requires_tenant_or_auth_url() {
        let err = MarketingCloudClientBuilder::new()
            .client_credentials("id", "secret")
            .build()
            .unwrap_err();
        assert!(matches!(err, MarketingCloudError::Configuration(_)));
    }

    #[test]
    fn build_succeeds_with_tenant_and_credentials() {
        let client = MarketingCloudClientBuilder::new()
            .tenant_subdomain("sub")
            .client_credentials("id", "secret")
            .build();
        assert!(client.is_ok());
    }

    #[test]
    fn build_rejects_empty_tenant() {
        let err = MarketingCloudClientBuilder::new()
            .tenant_subdomain("  ")
            .client_credentials("id", "secret")
            .build()
            .unwrap_err();
        assert!(matches!(err, MarketingCloudError::Configuration(_)));
    }

    #[test]
    fn auth_url_override_bypasses_tenant_requirement() {
        let client = MarketingCloudClientBuilder::new()
            .auth_url("http://localhost:1234/v2/token")
            .client_credentials("id", "secret")
            .build();
        assert!(client.is_ok());
    }
}
