//! CPQ Configuration API operations.
//!
//! Provides configuration loading and validation via the ServiceRouter.

use super::CpqHandler;
use super::types::{ConfigurationModel, ServiceRouterRequest};
use crate::error::Result;

/// CPQ Config API loader class names.
mod loaders {
    pub const CONFIG_LOADER: &str = "SBQQ.ConfigAPI.ConfigLoader";
    pub const CONFIG_VALIDATOR: &str = "SBQQ.ConfigAPI.ConfigurationValidator";
}

impl<A: crate::auth::Authenticator> CpqHandler<A> {
    /// Loads a product configuration model.
    ///
    /// Returns the [`ConfigurationModel`] with available options and
    /// current selections for the specified product on the given quote.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration cannot be loaded.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let config = client.cpq()
    ///     .load_config("a0x000000000001AAA", "01t000000000001AAA")
    ///     .await?;
    /// println!("Valid: {:?}", config.valid);
    /// ```
    pub async fn load_config(
        &self,
        quote_id: &str,
        product_id: &str,
    ) -> Result<ConfigurationModel> {
        let model = serde_json::json!({
            "quoteId": quote_id,
            "productId": product_id
        });
        let envelope = ServiceRouterRequest::new(loaders::CONFIG_LOADER, &model).map_err(|e| {
            crate::error::ForceError::Serialization(crate::error::SerializationError::from(e))
        })?;

        self.service_router_post(loaders::CONFIG_LOADER, &envelope)
            .await
    }

    /// Validates a product configuration.
    ///
    /// Checks whether the current [`ConfigurationModel`] is valid according
    /// to CPQ rules. Returns the validated model with any validation messages.
    ///
    /// # Errors
    ///
    /// Returns an error if the validation request fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let config = client.cpq()
    ///     .load_config("a0x000000000001AAA", "01t000000000001AAA")
    ///     .await?;
    /// let validated = client.cpq().validate_config(&config).await?;
    /// if validated.valid == Some(true) {
    ///     println!("Configuration is valid");
    /// }
    /// ```
    pub async fn validate_config(&self, config: &ConfigurationModel) -> Result<ConfigurationModel> {
        let envelope =
            ServiceRouterRequest::new(loaders::CONFIG_VALIDATOR, config).map_err(|e| {
                crate::error::ForceError::Serialization(crate::error::SerializationError::from(e))
            })?;

        self.service_router_post(loaders::CONFIG_VALIDATOR, &envelope)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::loaders;
    use crate::api::cpq::ConfigurationModel;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn setup() -> (MockServer, crate::client::ForceClient<MockAuthenticator>) {
        let server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &server.uri());
        let client = crate::client::builder()
            .authenticate(auth)
            .build()
            .await
            .must();
        (server, client)
    }

    #[tokio::test]
    async fn test_load_config_success() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/apexrest/SBQQ/ServiceRouter"))
            .and(query_param("loader", loaders::CONFIG_LOADER))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "configuredProductId": "01t000000000001AAA",
                "optionConfigurations": [
                    {"optionId": "opt1", "selected": true},
                    {"optionId": "opt2", "selected": false}
                ],
                "valid": true,
                "validationMessages": []
            })))
            .mount(&server)
            .await;

        let config = client
            .cpq()
            .load_config("a0x000000000001AAA", "01t000000000001AAA")
            .await
            .must();
        assert_eq!(
            config.configured_product_id.as_deref(),
            Some("01t000000000001AAA")
        );
        assert_eq!(config.option_configurations.len(), 2);
        assert_eq!(config.valid, Some(true));
    }

    #[tokio::test]
    async fn test_validate_config_valid() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/apexrest/SBQQ/ServiceRouter"))
            .and(query_param("loader", loaders::CONFIG_VALIDATOR))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "configuredProductId": "01t000000000001AAA",
                "optionConfigurations": [],
                "valid": true,
                "validationMessages": []
            })))
            .mount(&server)
            .await;

        let config = ConfigurationModel {
            configured_product_id: Some("01t000000000001AAA".to_string()),
            option_configurations: vec![],
            valid: None,
            validation_messages: vec![],
            extra: std::collections::HashMap::new(),
        };

        let validated = client.cpq().validate_config(&config).await.must();
        assert_eq!(validated.valid, Some(true));
    }

    #[tokio::test]
    async fn test_validate_config_invalid() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/apexrest/SBQQ/ServiceRouter"))
            .and(query_param("loader", loaders::CONFIG_VALIDATOR))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "configuredProductId": "01t000000000001AAA",
                "optionConfigurations": [],
                "valid": false,
                "validationMessages": ["Required option 'Base License' is not selected"]
            })))
            .mount(&server)
            .await;

        let config = ConfigurationModel {
            configured_product_id: Some("01t000000000001AAA".to_string()),
            option_configurations: vec![],
            valid: None,
            validation_messages: vec![],
            extra: std::collections::HashMap::new(),
        };

        let validated = client.cpq().validate_config(&config).await.must();
        assert_eq!(validated.valid, Some(false));
        assert_eq!(validated.validation_messages.len(), 1);
        assert!(validated.validation_messages[0].contains("Required option"));
    }

    #[tokio::test]
    async fn test_load_config_error() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/apexrest/SBQQ/ServiceRouter"))
            .and(query_param("loader", loaders::CONFIG_LOADER))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let result = client
            .cpq()
            .load_config("a0x000000000001AAA", "01t000000000001AAA")
            .await;
        assert!(result.is_err());
    }
}
