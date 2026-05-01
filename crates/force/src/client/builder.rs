//! Builder for `ForceClient` with safe authentication guarantees.

use crate::auth::authenticator::Authenticator;
use crate::auth::token_manager::TokenManager;
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

/// Builder for `ForceClient`.
///
/// The builder ensures that you must configure authentication before
/// calling `.build()`, preventing unauthenticated clients from being created.
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
    /// Optional Data Cloud configuration (feature-gated).
    #[cfg(feature = "data_cloud")]
    dc_config: Option<crate::auth::DataCloudConfig>,
}

impl ForceClientBuilder<NoAuth> {
    /// Creates a new builder in the unauthenticated state.
    #[must_use]
    pub const fn new() -> Self {
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
            #[cfg(feature = "data_cloud")]
            dc_config: None,
        }
    }
}

impl Default for ForceClientBuilder<NoAuth> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Authenticator> AuthenticatedBuilder<A> {
    /// Sets the client configuration.
    #[must_use]
    pub fn config(mut self, config: ClientConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Enables Data Cloud API access with the given configuration.
    ///
    /// This creates a secondary session with a `DataCloudAuthenticator`
    /// that performs the two-step token exchange.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use force::auth::DataCloudConfig;
    ///
    /// let client = ForceClient::builder()
    ///     .authenticate(auth)
    ///     .with_data_cloud(DataCloudConfig::default())
    ///     .build()
    ///     .await?;
    /// ```
    #[cfg(feature = "data_cloud")]
    #[must_use]
    pub fn with_data_cloud(mut self, config: crate::auth::DataCloudConfig) -> Self {
        self.dc_config = Some(config);
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
        use crate::session::Session;
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

        // Optionally create the Data Cloud session
        #[cfg(feature = "data_cloud")]
        let dc_session = self.dc_config.map(|dc_config| {
            // Resolve DC-specific API version (or inherit platform)
            let dc_api_version = dc_config
                .api_version
                .clone()
                .unwrap_or_else(|| config.api_version.clone());

            let dc_client_config = ClientConfig {
                api_version: dc_api_version,
                ..config.clone()
            };

            let dc_auth = crate::auth::DataCloudAuthenticator::new(
                Arc::clone(&token_manager),
                http_client.clone(),
                dc_config,
            );
            let dc_token_manager = Arc::new(TokenManager::new(dc_auth));
            let dc_http_executor = crate::http::HttpExecutor::with_client(
                http_client.clone(),
                dc_client_config.max_retries,
                dc_client_config.timeout,
            );

            Arc::new(Session {
                config: dc_client_config,
                http_client: http_client.clone(),
                http_executor: dc_http_executor,
                token_manager: dc_token_manager,
            })
        });

        let session = Session {
            config,
            http_client,
            http_executor,
            token_manager,
        };

        Ok(ForceClient {
            inner: Arc::new(session),
            #[cfg(feature = "data_cloud")]
            dc_session,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ClientConfig, Environment};
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;

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
        let auth_builder = builder.authenticate(MockAuthenticator::new(
            "mock_token",
            "https://test.salesforce.com",
        ));
        // Verify it's now AuthenticatedBuilder
        let _: AuthenticatedBuilder<MockAuthenticator> = auth_builder;
    }

    #[tokio::test]
    async fn test_builder_builds_client() {
        let client = ForceClientBuilder::new()
            .authenticate(MockAuthenticator::new(
                "mock_token",
                "https://test.salesforce.com",
            ))
            .build()
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
            .authenticate(MockAuthenticator::new(
                "mock_token",
                "https://test.salesforce.com",
            ))
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
            .authenticate(MockAuthenticator::new(
                "mock_token",
                "https://test.salesforce.com",
            ))
            .config(config)
            .build()
            .await
            .must();

        assert_eq!(client.config().api_version, "v61.0");
    }
}
