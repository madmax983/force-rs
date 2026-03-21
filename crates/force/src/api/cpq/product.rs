//! CPQ Product API operations.
//!
//! Provides product loading via the ServiceRouter.

use super::CpqHandler;
use super::types::{ProductModel, ServiceRouterRequest};
use crate::error::Result;

/// CPQ Product API loader class name.
const PRODUCT_LOADER: &str = "SBQQ.ProductAPI.ProductLoader";

impl<A: crate::auth::Authenticator> CpqHandler<A> {
    /// Loads a product with its options, features, and configuration.
    ///
    /// Returns the full [`ProductModel`] including feature categories,
    /// features, and available options.
    ///
    /// # Errors
    ///
    /// Returns an error if the product does not exist or the request fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let product = client.cpq().load_product("01t000000000001AAA").await?;
    /// println!("Options: {}", product.options.len());
    /// ```
    pub async fn load_product(&self, product_id: &str) -> Result<ProductModel> {
        let model = serde_json::json!({"productId": product_id});
        let envelope = ServiceRouterRequest::new(PRODUCT_LOADER, &model).map_err(|e| {
            crate::error::ForceError::Serialization(crate::error::SerializationError::from(e))
        })?;

        self.service_router_post(PRODUCT_LOADER, &envelope).await
    }
}

#[cfg(test)]
mod tests {
    use super::PRODUCT_LOADER;
    use crate::test_support::{MockAuthenticator, Must};
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
    async fn test_load_product_success() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/apexrest/SBQQ/ServiceRouter"))
            .and(query_param("loader", PRODUCT_LOADER))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "record": {"Id": "01t000000000001AAA", "Name": "Enterprise Widget"},
                "featureCategories": [{"name": "Base Features"}],
                "features": [{"name": "Feature A"}],
                "options": [
                    {"id": "opt1", "name": "Option 1"},
                    {"id": "opt2", "name": "Option 2"}
                ]
            })))
            .mount(&server)
            .await;

        let product = client.cpq().load_product("01t000000000001AAA").await.must();
        assert!(product.record.is_some());
        assert_eq!(product.feature_categories.len(), 1);
        assert_eq!(product.features.len(), 1);
        assert_eq!(product.options.len(), 2);
    }

    #[tokio::test]
    async fn test_load_product_not_found() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/apexrest/SBQQ/ServiceRouter"))
            .and(query_param("loader", PRODUCT_LOADER))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "message": "Product not found",
                "errorCode": "ENTITY_NOT_FOUND"
            })))
            .mount(&server)
            .await;

        let result = client.cpq().load_product("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_load_product_minimal_response() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/apexrest/SBQQ/ServiceRouter"))
            .and(query_param("loader", PRODUCT_LOADER))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "record": {"Id": "01t000000000001AAA"}
            })))
            .mount(&server)
            .await;

        let product = client.cpq().load_product("01t000000000001AAA").await.must();
        assert!(product.features.is_empty());
        assert!(product.options.is_empty());
    }
}
