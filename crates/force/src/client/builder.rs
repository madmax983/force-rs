//! Builder for `ForceClient`.

use crate::auth::Authenticator;
use crate::client::ForceClient;
use crate::config::ClientConfig;
use crate::error::Result;

/// Builder for `ForceClient`.
#[derive(Debug, Default)]
pub struct ForceClientBuilder {
    config: Option<ClientConfig>,
}

impl ForceClientBuilder {
    /// Creates a new builder.
    #[must_use]
    pub const fn new() -> Self {
        Self { config: None }
    }

    /// Sets the client configuration.
    #[must_use]
    pub fn config(mut self, config: ClientConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Builds the `ForceClient` with the given authenticator.
    ///
    /// # Errors
    ///
    /// Returns an error if HTTP client construction fails.
    pub async fn build<A: Authenticator>(self, authenticator: A) -> Result<ForceClient<A>> {
        use crate::http::HttpExecutor;
        use std::sync::Arc;

        let config = self.config.unwrap_or_default();

        // Build HTTP client with timeout from config
        let http_client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(crate::error::HttpError::from)?;

        let http_executor = HttpExecutor::with_client(
            http_client.clone(),
            config.max_retries,
            config.timeout,
        );

        // Create token manager with the authenticator
        let token_manager = Arc::new(crate::auth::TokenManager::new(authenticator));

        Ok(ForceClient {
            config,
            http_client,
            http_executor,
            token_manager,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{AccessToken, Authenticator};
    use crate::config::{ClientConfig, Environment};
    use crate::test_support::Must;
    use async_trait::async_trait;

    // Mock authenticator for testing
    #[derive(Debug, Clone)]
    struct MockAuth;

    #[async_trait]
    impl Authenticator for MockAuth {
        async fn authenticate(&self) -> Result<AccessToken> {
            // Create a mock token for testing
            Ok(AccessToken::new(
                "mock_token".to_string(),
                "https://test.salesforce.com".to_string(),
                None,
            ))
        }

        async fn refresh(&self) -> Result<AccessToken> {
            self.authenticate().await
        }
    }

    #[test]
    fn test_builder_new() {
        let _builder = ForceClientBuilder::new();
    }

    #[test]
    fn test_builder_config_chainable() {
        let config = ClientConfig::default();
        let _builder = ForceClientBuilder::new().config(config);
    }

    #[tokio::test]
    async fn test_builder_builds_client() {
        let client = ForceClientBuilder::new()
            .build(MockAuth)
            .await
            .must();

        // Verify client is configured with defaults
        let config = client.config();
        assert_eq!(config.api_version, "v60.0");
        assert_eq!(config.environment, Environment::Production);
        assert_eq!(config.timeout, std::time::Duration::from_secs(30));
        assert_eq!(config.max_retries, 3);
    }

    #[tokio::test]
    async fn test_full_builder_flow() {
        let config = ClientConfig {
            api_version: "v60.0".to_string(),
            environment: Environment::Sandbox,
            timeout: std::time::Duration::from_secs(30),
            max_retries: 3,
        };

        let client = ForceClientBuilder::new()
            .config(config.clone())
            .build(MockAuth)
            .await
            .must();

        assert_eq!(client.config().api_version, "v60.0");
        assert_eq!(client.config().environment, Environment::Sandbox);
    }
}
