//! Client configuration types for the Force API client.
//!
//! This module provides configuration primitives for the Salesforce client,
//! including environment management and client settings.

use crate::types::ApiVersion;
use std::time::Duration;

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
            api_version: ApiVersion::DEFAULT.to_string(),
            environment: Environment::Production,
            timeout: Duration::from_secs(30),
            max_retries: 3,
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

    mod client_config_custom {
        use super::*;

        #[test]
        fn test_custom_api_version() {
            let config = ClientConfig {
                api_version: "v61.0".into(),
                ..Default::default()
            };
            assert_eq!(config.api_version, "v61.0");
        }

        #[test]
        fn test_custom_environment() {
            let config = ClientConfig {
                environment: Environment::Sandbox,
                ..Default::default()
            };
            assert_eq!(config.environment, Environment::Sandbox);
        }

        #[test]
        fn test_custom_timeout() {
            let config = ClientConfig {
                timeout: Duration::from_secs(60),
                ..Default::default()
            };
            assert_eq!(config.timeout, Duration::from_secs(60));
        }

        #[test]
        fn test_custom_retries() {
            let config = ClientConfig {
                max_retries: 5,
                ..Default::default()
            };
            assert_eq!(config.max_retries, 5);
        }

        #[test]
        fn test_all_custom() {
            let config = ClientConfig {
                api_version: "v59.0".into(),
                environment: Environment::Custom("https://my.salesforce.com".to_string()),
                timeout: Duration::from_secs(45),
                max_retries: 2,
            };

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
