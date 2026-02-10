//! Builder for `ForceClient` with type-state pattern for authentication safety.

use crate::auth::{Authenticator, TokenManager};
use crate::client::ForceClient;
use crate::config::ClientConfig;
use crate::error::Result;
use std::marker::PhantomData;

/// Marker type indicating no authentication has been configured.
#[derive(Debug, Clone)]
pub struct NoAuth;

/// Marker type indicating authentication has been configured.
#[derive(Debug, Clone)]
pub struct HasAuth;

/// Builder for `ForceClient` with compile-time authentication safety.
///
/// The builder uses phantom types to track authentication state at compile-time,
/// ensuring that clients cannot be built without proper authentication.
///
/// In the `NoAuth` state, there is no authenticator. In the `HasAuth` state,
/// the builder becomes generic over the authenticator type.
#[derive(Debug)]
pub struct ForceClientBuilder<Auth = NoAuth> {
    config: Option<ClientConfig>,
    _auth: PhantomData<Auth>,
}

/// Builder in the authenticated state, generic over the authenticator.
#[derive(Debug)]
pub struct AuthenticatedBuilder<A: Authenticator> {
    config: Option<ClientConfig>,
    authenticator: A,
}

impl ForceClientBuilder<NoAuth> {
    /// Creates a new builder in the unauthenticated state.
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self {
            config: None,
            _auth: PhantomData,
        }
    }

    /// Sets the client configuration.
    #[must_use]
    pub fn config(mut self, config: ClientConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Configures authentication using the provided authenticator.
    ///
    /// This transitions the builder to the authenticated state.
    pub fn authenticate<A: Authenticator>(self, authenticator: A) -> AuthenticatedBuilder<A> {
        AuthenticatedBuilder {
            config: self.config,
            authenticator,
        }
    }
}

impl<A: Authenticator> AuthenticatedBuilder<A> {
    /// Sets the client configuration.
    #[must_use]
    pub fn config(mut self, config: ClientConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Builds the `ForceClient`.
    ///
    /// This method is only available when authentication has been configured.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - HTTP client construction fails
    #[allow(clippy::unused_async)] // Async signature for future auth initialization
    pub async fn build(self) -> Result<ForceClient<A>> {
        use crate::client::Inner;
        use std::sync::Arc;

        let config = self.config.unwrap_or_default();

        // Build HTTP client with timeout from config
        let http_client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(crate::error::HttpError::from)?;
        let http_executor = crate::http::HttpExecutor::with_client(
            http_client.clone(),
            config.max_retries,
            config.timeout,
        );

        // Create token manager with the authenticator
        let token_manager = Arc::new(TokenManager::new(self.authenticator));

        let inner = Inner {
            config,
            http_client,
            http_executor,
            token_manager,
        };

        Ok(ForceClient {
            inner: Arc::new(inner),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AccessToken;
    use crate::config::{ClientConfig, Environment};
    use crate::test_support::Must;
    use async_trait::async_trait;
    use std::sync::Arc;

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
    fn test_builder_new_creates_noauth() {
        let _builder: ForceClientBuilder<NoAuth> = ForceClientBuilder::new();
    }

    #[test]
    fn test_builder_config_chainable() {
        let config = ClientConfig::default();
        let builder = ForceClientBuilder::new().config(config);
        // Verify it's still NoAuth after setting config
        let _: ForceClientBuilder<NoAuth> = builder;
    }

    #[test]
    fn test_builder_authenticate_transitions_state() {
        let builder = ForceClientBuilder::new();
        let auth_builder = builder.authenticate(MockAuth);
        // Verify it's now AuthenticatedBuilder
        let _: AuthenticatedBuilder<MockAuth> = auth_builder;
    }

    #[tokio::test]
    async fn test_builder_builds_client() {
        let client = ForceClientBuilder::new()
            .authenticate(MockAuth)
            .build()
            .await
            .must();

        // Verify client was created
        assert!(Arc::strong_count(&client.inner) == 1);
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
            .authenticate(MockAuth)
            .build()
            .await
            .must();

        assert_eq!(client.config().api_version, "v60.0");
        assert_eq!(client.config().environment, Environment::Sandbox);
    }

    #[tokio::test]
    async fn test_builder_config_after_auth() {
        let config = ClientConfig {
            api_version: "v61.0".to_string(),
            environment: Environment::Production,
            timeout: std::time::Duration::from_secs(45),
            max_retries: 5,
        };

        let client = ForceClientBuilder::new()
            .authenticate(MockAuth)
            .config(config)
            .build()
            .await
            .must();

        assert_eq!(client.config().api_version, "v61.0");
    }
}
