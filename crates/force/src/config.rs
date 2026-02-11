//! Client configuration types for the Force API client.
//!
//! This module provides configuration primitives for the Salesforce client,
//! including environment management and client settings.

use std::time::Duration;
use crate::types::ApiVersion;

/// Salesforce environment endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Environment {
    /// Production environment (login.salesforce.com)
    Production,
    /// Sandbox environment (test.salesforce.com)
    Sandbox,
    /// Custom Salesforce instance
    Custom(String),
}

impl Environment {
    /// Returns the instance URL for this environment.
    #[must_use]
    pub const fn instance_url(&self) -> &str {
        match self {
            Self::Production => "https://login.salesforce.com",
            Self::Sandbox => "https://test.salesforce.com",
            Self::Custom(url) => url.as_str(),
        }
    }
}

/// Configuration for the Force API client.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// API version to use (e.g., "v60.0")
    pub api_version: String,
    /// Salesforce environment
    pub environment: Environment,
    /// Request timeout
    pub timeout: Duration,
    /// Maximum number of retry attempts
    pub max_retries: u32,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            api_version: ApiVersion::DEFAULT.as_str(),
            environment: Environment::Production,
            timeout: Duration::from_secs(30),
            max_retries: 3,
        }
    }
}

/// Builder for constructing `ClientConfig` instances.
#[derive(Debug, Default, Clone)]
pub struct ClientConfigBuilder {
    api_version: Option<String>,
    environment: Option<Environment>,
    timeout: Option<Duration>,
    max_retries: Option<u32>,
}

impl ClientConfigBuilder {
    /// Creates a new builder with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the API version.
    #[must_use]
    pub fn api_version(mut self, version: impl Into<String>) -> Self {
        self.api_version = Some(version.into());
        self
    }

    /// Sets the environment.
    #[must_use]
    pub fn environment(mut self, env: Environment) -> Self {
        self.environment = Some(env);
        self
    }

    /// Sets the request timeout.
    #[must_use]
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Sets the maximum retry attempts.
    #[must_use]
    pub const fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = Some(retries);
        self
    }

    /// Builds the `ClientConfig`.
    #[must_use]
    pub fn build(self) -> ClientConfig {
        ClientConfig {
            api_version: self.api_version.unwrap_or_else(|| ApiVersion::DEFAULT.as_str()),
            environment: self.environment.unwrap_or(Environment::Production),
            timeout: self.timeout.unwrap_or_else(|| Duration::from_secs(30)),
            max_retries: self.max_retries.unwrap_or(3),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    mod environment {
        use super::*;

        #[test]
        fn test_production_instance_url() {
            let env = Environment::Production;
            assert_eq!(env.instance_url(), "https://login.salesforce.com");
        }

        #[test]
        fn test_sandbox_instance_url() {
            let env = Environment::Sandbox;
            assert_eq!(env.instance_url(), "https://test.salesforce.com");
        }

        #[test]
        fn test_custom_instance_url() {
            let env = Environment::Custom("https://custom.my.salesforce.com".to_string());
            assert_eq!(env.instance_url(), "https://custom.my.salesforce.com");
        }
    }

    mod client_config {
        use super::*;

        #[test]
        fn test_default_config() {
            let config = ClientConfig::default();
            assert_eq!(config.api_version, "v60.0");
            assert_eq!(config.environment, Environment::Production);
            assert_eq!(config.timeout, Duration::from_secs(30));
            assert_eq!(config.max_retries, 3);
        }
    }

    mod builder {
        use super::*;

        #[test]
        fn test_builder_default() {
            let config = ClientConfigBuilder::new().build();
            assert_eq!(config.api_version, "v60.0");
            assert_eq!(config.environment, Environment::Production);
            assert_eq!(config.timeout, Duration::from_secs(30));
            assert_eq!(config.max_retries, 3);
        }

        #[test]
        fn test_builder_custom_api_version() {
            let config = ClientConfigBuilder::new().api_version("v61.0").build();
            assert_eq!(config.api_version, "v61.0");
        }

        #[test]
        fn test_builder_custom_environment() {
            let config = ClientConfigBuilder::new()
                .environment(Environment::Sandbox)
                .build();
            assert_eq!(config.environment, Environment::Sandbox);
        }

        #[test]
        fn test_builder_custom_timeout() {
            let config = ClientConfigBuilder::new()
                .timeout(Duration::from_secs(60))
                .build();
            assert_eq!(config.timeout, Duration::from_secs(60));
        }

        #[test]
        fn test_builder_custom_retries() {
            let config = ClientConfigBuilder::new().max_retries(5).build();
            assert_eq!(config.max_retries, 5);
        }

        #[test]
        fn test_builder_all_custom() {
            let config = ClientConfigBuilder::new()
                .api_version("v59.0")
                .environment(Environment::Custom("https://my.salesforce.com".to_string()))
                .timeout(Duration::from_secs(45))
                .max_retries(2)
                .build();

            assert_eq!(config.api_version, "v59.0");
            assert_eq!(
                config.environment,
                Environment::Custom("https://my.salesforce.com".to_string())
            );
            assert_eq!(config.timeout, Duration::from_secs(45));
            assert_eq!(config.max_retries, 2);
        }
    }
}
