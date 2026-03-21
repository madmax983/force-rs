//! CPQ Quote lifecycle operations.
//!
//! Provides the core quote read/save/calculate loop and product addition.

use super::CpqHandler;
use super::types::{AddProductsRequest, QuoteModel, ServiceRouterRequest};
use crate::error::Result;

/// CPQ Quote API loader class names.
mod loaders {
    pub const QUOTE_READER: &str = "SBQQ.QuoteAPI.QuoteReader";
    pub const QUOTE_SAVER: &str = "SBQQ.QuoteAPI.QuoteSaver";
    pub const QUOTE_CALCULATOR: &str = "SBQQ.QuoteAPI.QuoteCalculator";
    pub const QUOTE_PRODUCT_ADDER: &str = "SBQQ.QuoteAPI.QuoteProductAdder";
}

impl<A: crate::auth::Authenticator> CpqHandler<A> {
    /// Reads a quote by its Salesforce ID.
    ///
    /// Returns the full [`QuoteModel`] including line items.
    ///
    /// # Errors
    ///
    /// Returns an error if the quote does not exist or the request fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let quote = client.cpq().read_quote("a0x000000000001AAA").await?;
    /// println!("Status: {:?}", quote.status);
    /// ```
    pub async fn read_quote(&self, quote_id: &str) -> Result<QuoteModel> {
        let model = serde_json::json!({"quoteId": quote_id});
        let envelope = ServiceRouterRequest::new(loaders::QUOTE_READER, &model).map_err(|e| {
            crate::error::ForceError::Serialization(crate::error::SerializationError::from(e))
        })?;

        self.service_router_post(loaders::QUOTE_READER, &envelope)
            .await
    }

    /// Saves a quote model back to Salesforce.
    ///
    /// Persists all changes made to the [`QuoteModel`] including line items.
    ///
    /// # Errors
    ///
    /// Returns an error if validation fails or the request fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut quote = client.cpq().read_quote("a0x000000000001AAA").await?;
    /// // ... modify quote ...
    /// let saved = client.cpq().save_quote(&quote).await?;
    /// ```
    pub async fn save_quote(&self, quote: &QuoteModel) -> Result<QuoteModel> {
        let envelope = ServiceRouterRequest::new(loaders::QUOTE_SAVER, quote).map_err(|e| {
            crate::error::ForceError::Serialization(crate::error::SerializationError::from(e))
        })?;

        self.service_router_post(loaders::QUOTE_SAVER, &envelope)
            .await
    }

    /// Calculates pricing for a quote model.
    ///
    /// Applies price rules, discount schedules, and other CPQ pricing logic
    /// to the quote. Returns the updated [`QuoteModel`] with calculated amounts.
    ///
    /// **Note:** This operation uses HTTP PATCH, not POST.
    ///
    /// # Errors
    ///
    /// Returns an error if pricing calculation fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let quote = client.cpq().read_quote("a0x000000000001AAA").await?;
    /// let calculated = client.cpq().calculate_quote(&quote).await?;
    /// println!("Net amount: {:?}", calculated.net_amount);
    /// ```
    pub async fn calculate_quote(&self, quote: &QuoteModel) -> Result<QuoteModel> {
        self.service_router_patch(loaders::QUOTE_CALCULATOR, quote)
            .await
    }

    /// Adds products to a quote.
    ///
    /// Uses the `QuoteProductAdder` loader to add the specified products
    /// to the given quote. Returns the updated [`QuoteModel`] with the
    /// new line items.
    ///
    /// # Errors
    ///
    /// Returns an error if the products cannot be added.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let request = AddProductsRequest::new(vec![
    ///     "01t000000000001AAA".to_string(),
    ///     "01t000000000002AAA".to_string(),
    /// ]);
    /// let updated = client.cpq()
    ///     .add_products("a0x000000000001AAA", &request)
    ///     .await?;
    /// ```
    pub async fn add_products(
        &self,
        quote_id: &str,
        request: &AddProductsRequest,
    ) -> Result<QuoteModel> {
        let payload = serde_json::json!({
            "quoteId": quote_id,
            "products": request
        });
        let envelope =
            ServiceRouterRequest::new(loaders::QUOTE_PRODUCT_ADDER, &payload).map_err(|e| {
                crate::error::ForceError::Serialization(crate::error::SerializationError::from(e))
            })?;

        self.service_router_post(loaders::QUOTE_PRODUCT_ADDER, &envelope)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::loaders;
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

    fn sample_quote_json() -> serde_json::Value {
        serde_json::json!({
            "Id": "a0x000000000001AAA",
            "SBQQ__Status__c": "Draft",
            "SBQQ__NetAmount__c": 1500.0,
            "SBQQ__Primary__c": true,
            "lineItems": [
                {
                    "Id": "a0y000000000001AAA",
                    "SBQQ__Product__c": "01t000000000001AAA",
                    "SBQQ__Quantity__c": 2.0,
                    "SBQQ__NetPrice__c": 750.0,
                    "SBQQ__NetTotal__c": 1500.0
                }
            ]
        })
    }

    // ── read_quote tests ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_read_quote_success() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/apexrest/SBQQ/ServiceRouter"))
            .and(query_param("loader", loaders::QUOTE_READER))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_quote_json()))
            .mount(&server)
            .await;

        let quote = client.cpq().read_quote("a0x000000000001AAA").await.must();
        assert_eq!(quote.id.as_deref(), Some("a0x000000000001AAA"));
        assert_eq!(quote.status.as_deref(), Some("Draft"));
        assert_eq!(quote.net_amount, Some(1500.0));
        assert_eq!(quote.line_items.len(), 1);
    }

    #[tokio::test]
    async fn test_read_quote_not_found() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/apexrest/SBQQ/ServiceRouter"))
            .and(query_param("loader", loaders::QUOTE_READER))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "message": "Quote not found",
                "errorCode": "ENTITY_NOT_FOUND"
            })))
            .mount(&server)
            .await;

        let result = client.cpq().read_quote("nonexistent").await;
        assert!(result.is_err());
    }

    // ── save_quote tests ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_save_quote_success() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/apexrest/SBQQ/ServiceRouter"))
            .and(query_param("loader", loaders::QUOTE_SAVER))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_quote_json()))
            .mount(&server)
            .await;

        let quote: crate::api::cpq::QuoteModel =
            serde_json::from_value(sample_quote_json()).unwrap();

        let saved = client.cpq().save_quote(&quote).await.must();
        assert_eq!(saved.id.as_deref(), Some("a0x000000000001AAA"));
    }

    // ── calculate_quote tests ────────────────────────────────────────

    #[tokio::test]
    async fn test_calculate_quote_uses_patch() {
        let (server, client) = setup().await;

        let calculated_json = serde_json::json!({
            "Id": "a0x000000000001AAA",
            "SBQQ__Status__c": "Draft",
            "SBQQ__NetAmount__c": 1350.0,
            "lineItems": [
                {
                    "Id": "a0y000000000001AAA",
                    "SBQQ__NetPrice__c": 675.0,
                    "SBQQ__NetTotal__c": 1350.0,
                    "SBQQ__Discount__c": 10.0
                }
            ]
        });

        Mock::given(method("PATCH"))
            .and(path("/services/apexrest/SBQQ/ServiceRouter"))
            .and(query_param("loader", loaders::QUOTE_CALCULATOR))
            .respond_with(ResponseTemplate::new(200).set_body_json(&calculated_json))
            .mount(&server)
            .await;

        let quote: crate::api::cpq::QuoteModel =
            serde_json::from_value(sample_quote_json()).unwrap();

        let calculated = client.cpq().calculate_quote(&quote).await.must();
        assert_eq!(calculated.net_amount, Some(1350.0));
        assert_eq!(calculated.line_items[0].discount, Some(10.0));
    }

    // ── add_products tests ───────────────────────────────────────────

    #[tokio::test]
    async fn test_add_products_success() {
        let (server, client) = setup().await;

        let updated_json = serde_json::json!({
            "Id": "a0x000000000001AAA",
            "SBQQ__Status__c": "Draft",
            "SBQQ__NetAmount__c": 2500.0,
            "lineItems": [
                {"Id": "a0y000000000001AAA", "SBQQ__NetTotal__c": 1500.0},
                {"Id": "a0y000000000002AAA", "SBQQ__NetTotal__c": 1000.0}
            ]
        });

        Mock::given(method("POST"))
            .and(path("/services/apexrest/SBQQ/ServiceRouter"))
            .and(query_param("loader", loaders::QUOTE_PRODUCT_ADDER))
            .respond_with(ResponseTemplate::new(200).set_body_json(&updated_json))
            .mount(&server)
            .await;

        let request =
            crate::api::cpq::AddProductsRequest::new(vec!["01t000000000002AAA".to_string()]);

        let updated = client
            .cpq()
            .add_products("a0x000000000001AAA", &request)
            .await
            .must();
        assert_eq!(updated.line_items.len(), 2);
        assert_eq!(updated.net_amount, Some(2500.0));
    }

    // ── error handling tests ─────────────────────────────────────────

    #[tokio::test]
    async fn test_save_quote_validation_error() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/apexrest/SBQQ/ServiceRouter"))
            .and(query_param("loader", loaders::QUOTE_SAVER))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "message": "Validation failed: required fields missing",
                "errorCode": "VALIDATION_ERROR"
            })))
            .mount(&server)
            .await;

        let quote: crate::api::cpq::QuoteModel =
            serde_json::from_value(sample_quote_json()).unwrap();

        let result = client.cpq().save_quote(&quote).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_calculate_quote_server_error() {
        let (server, client) = setup().await;

        Mock::given(method("PATCH"))
            .and(path("/services/apexrest/SBQQ/ServiceRouter"))
            .and(query_param("loader", loaders::QUOTE_CALCULATOR))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let quote: crate::api::cpq::QuoteModel =
            serde_json::from_value(sample_quote_json()).unwrap();

        let result = client.cpq().calculate_quote(&quote).await;
        assert!(result.is_err());
    }
}
