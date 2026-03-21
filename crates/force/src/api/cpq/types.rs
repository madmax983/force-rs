//! CPQ API data model types.
//!
//! These types model the Salesforce CPQ ServiceRouter request and response
//! payloads. Core fields are strongly typed; unknown or custom fields are
//! captured by a `#[serde(flatten)] extra` map on each model.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Quote Models ─────────────────────────────────────────────────────

/// A Salesforce CPQ quote model as returned by the ServiceRouter.
///
/// Core fields are typed for convenience; any additional fields (custom
/// or unmapped) are captured in [`extra`](Self::extra).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuoteModel {
    /// Salesforce record ID for the quote.
    #[serde(rename = "Id", default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// Account associated with this quote.
    #[serde(
        rename = "SBQQ__Account__c",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub account_id: Option<String>,

    /// Opportunity this quote is linked to.
    #[serde(
        rename = "SBQQ__Opportunity2__c",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub opportunity_id: Option<String>,

    /// Whether this is the primary quote on the opportunity.
    #[serde(
        rename = "SBQQ__Primary__c",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub primary: Option<bool>,

    /// Quote status (e.g., "Draft", "Approved", "Presented").
    #[serde(
        rename = "SBQQ__Status__c",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub status: Option<String>,

    /// Net amount after all adjustments.
    #[serde(
        rename = "SBQQ__NetAmount__c",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub net_amount: Option<f64>,

    /// List (regular) amount before adjustments.
    #[serde(
        rename = "SBQQ__ListAmount__c",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub list_amount: Option<f64>,

    /// Customer-facing discount percentage.
    #[serde(
        rename = "SBQQ__CustomerDiscount__c",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub customer_discount: Option<f64>,

    /// Pricebook ID used for this quote.
    #[serde(
        rename = "SBQQ__PricebookId__c",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub pricebook_id: Option<String>,

    /// Start date for the subscription/quote.
    #[serde(
        rename = "SBQQ__StartDate__c",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub start_date: Option<String>,

    /// End date for the subscription/quote.
    #[serde(
        rename = "SBQQ__EndDate__c",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub end_date: Option<String>,

    /// Subscription term in months.
    #[serde(
        rename = "SBQQ__SubscriptionTerm__c",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub subscription_term: Option<f64>,

    /// Line items on this quote.
    #[serde(rename = "lineItems", default)]
    pub line_items: Vec<QuoteLineModel>,

    /// Catch-all for custom or unmapped fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// A single CPQ quote line item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuoteLineModel {
    /// Salesforce record ID for this line item.
    #[serde(rename = "Id", default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// Product linked to this line.
    #[serde(
        rename = "SBQQ__Product__c",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub product_id: Option<String>,

    /// Quantity of the product.
    #[serde(
        rename = "SBQQ__Quantity__c",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub quantity: Option<f64>,

    /// List (catalog) price per unit.
    #[serde(
        rename = "SBQQ__ListPrice__c",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub list_price: Option<f64>,

    /// Net price per unit after adjustments.
    #[serde(
        rename = "SBQQ__NetPrice__c",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub net_price: Option<f64>,

    /// Net total (quantity * net price).
    #[serde(
        rename = "SBQQ__NetTotal__c",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub net_total: Option<f64>,

    /// Discount percentage applied to this line.
    #[serde(
        rename = "SBQQ__Discount__c",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub discount: Option<f64>,

    /// Whether this is an optional line item.
    #[serde(
        rename = "SBQQ__Optional__c",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub optional: Option<bool>,

    /// The quote this line belongs to.
    #[serde(
        rename = "SBQQ__Quote__c",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub quote_id: Option<String>,

    /// Catch-all for custom or unmapped fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

// ── Product Model ────────────────────────────────────────────────────

/// A CPQ product model as returned by the Product API loader.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProductModel {
    /// The underlying Product2 record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<serde_json::Value>,

    /// Feature categories for this product.
    #[serde(rename = "featureCategories", default)]
    pub feature_categories: Vec<serde_json::Value>,

    /// Product features.
    #[serde(default)]
    pub features: Vec<serde_json::Value>,

    /// Available product options.
    #[serde(default)]
    pub options: Vec<serde_json::Value>,

    /// Configuration attributes.
    #[serde(default)]
    pub configuration: Option<serde_json::Value>,

    /// Catch-all for custom or unmapped fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

// ── Configuration Model ──────────────────────────────────────────────

/// A CPQ configuration model for product configuration operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigurationModel {
    /// The configured product's Salesforce ID.
    #[serde(
        rename = "configuredProductId",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub configured_product_id: Option<String>,

    /// Option configurations within this product.
    #[serde(rename = "optionConfigurations", default)]
    pub option_configurations: Vec<serde_json::Value>,

    /// Whether the current configuration is valid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid: Option<bool>,

    /// Validation messages, if any.
    #[serde(rename = "validationMessages", default)]
    pub validation_messages: Vec<String>,

    /// Catch-all for custom or unmapped fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

// ── ServiceRouter Request Envelope ───────────────────────────────────

/// The request envelope sent to the CPQ ServiceRouter.
///
/// The ServiceRouter expects `model` to be a **JSON string** (stringified JSON),
/// not a JSON object. The CPQ handler serializes the inner model to a string
/// before constructing this envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceRouterRequest {
    /// The saver/loader class name (e.g., `"SBQQ.QuoteAPI.QuoteSaver"`).
    pub saver: String,
    /// The model payload as a stringified JSON string.
    pub model: String,
}

impl ServiceRouterRequest {
    /// Creates a new `ServiceRouterRequest` from a saver name and a
    /// serializable model.
    ///
    /// The model is automatically serialized to a JSON string.
    ///
    /// # Errors
    ///
    /// Returns a serialization error if the model cannot be converted to JSON.
    pub fn new(
        saver: &str,
        model: &impl Serialize,
    ) -> std::result::Result<Self, serde_json::Error> {
        Ok(Self {
            saver: saver.to_string(),
            model: serde_json::to_string(model)?,
        })
    }

    /// Creates a `ServiceRouterRequest` from a saver name and a pre-serialized
    /// model string.
    #[must_use]
    pub fn from_raw(saver: &str, model: String) -> Self {
        Self {
            saver: saver.to_string(),
            model,
        }
    }
}

// ── Product Addition Types ───────────────────────────────────────────

/// Options for adding products to a quote.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddProductsRequest {
    /// Whether to ignore the calculate step after adding products.
    #[serde(rename = "ignoreCalculate", default)]
    pub ignore_calculate: bool,

    /// The IDs of products to add.
    #[serde(rename = "productIds")]
    pub product_ids: Vec<String>,

    /// Optional pricing method override.
    #[serde(
        rename = "pricingMethod",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub pricing_method: Option<String>,

    /// Catch-all for additional options.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl AddProductsRequest {
    /// Creates a new `AddProductsRequest` with the given product IDs.
    #[must_use]
    pub fn new(product_ids: Vec<String>) -> Self {
        Self {
            ignore_calculate: false,
            product_ids,
            pricing_method: None,
            extra: HashMap::new(),
        }
    }
}

// ── Document Generation Types ────────────────────────────────────────

/// Request for generating a quote document/proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateDocumentRequest {
    /// The quote ID to generate a document for.
    #[serde(rename = "quoteId")]
    pub quote_id: String,

    /// The template ID to use for document generation.
    #[serde(
        rename = "templateId",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub template_id: Option<String>,

    /// Output format (e.g., "pdf", "word").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,

    /// Catch-all for additional options.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl GenerateDocumentRequest {
    /// Creates a new document generation request for the given quote.
    #[must_use]
    pub fn new(quote_id: impl Into<String>) -> Self {
        Self {
            quote_id: quote_id.into(),
            template_id: None,
            format: None,
            extra: HashMap::new(),
        }
    }

    /// Sets the template ID.
    #[must_use]
    pub fn with_template(mut self, template_id: impl Into<String>) -> Self {
        self.template_id = Some(template_id.into());
        self
    }

    /// Sets the output format.
    #[must_use]
    pub fn with_format(mut self, format: impl Into<String>) -> Self {
        self.format = Some(format.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Must;

    // ── QuoteModel tests ─────────────────────────────────────────────

    #[test]
    fn test_quote_model_deserialize_core_fields() {
        let json = serde_json::json!({
            "Id": "a0x000000000001AAA",
            "SBQQ__Status__c": "Draft",
            "SBQQ__NetAmount__c": 1500.0,
            "SBQQ__Primary__c": true,
            "SBQQ__Account__c": "001000000000001AAA",
            "SBQQ__Opportunity2__c": "006000000000001AAA",
            "lineItems": []
        });

        let quote: QuoteModel = serde_json::from_value(json).must();
        assert_eq!(quote.id.as_deref(), Some("a0x000000000001AAA"));
        assert_eq!(quote.status.as_deref(), Some("Draft"));
        assert_eq!(quote.net_amount, Some(1500.0));
        assert_eq!(quote.primary, Some(true));
        assert_eq!(quote.account_id.as_deref(), Some("001000000000001AAA"));
        assert_eq!(quote.opportunity_id.as_deref(), Some("006000000000001AAA"));
        assert!(quote.line_items.is_empty());
    }

    #[test]
    fn test_quote_model_captures_extra_fields() {
        let json = serde_json::json!({
            "Id": "a0x000000000001AAA",
            "SBQQ__Status__c": "Draft",
            "lineItems": [],
            "CustomField__c": "custom_value",
            "AnotherCustom__c": 42
        });

        let quote: QuoteModel = serde_json::from_value(json).must();
        assert_eq!(quote.extra["CustomField__c"], "custom_value");
        assert_eq!(quote.extra["AnotherCustom__c"], 42);
    }

    #[test]
    fn test_quote_model_with_line_items() {
        let json = serde_json::json!({
            "Id": "a0x000000000001AAA",
            "lineItems": [
                {
                    "Id": "a0y000000000001AAA",
                    "SBQQ__Product__c": "01t000000000001AAA",
                    "SBQQ__Quantity__c": 2.0,
                    "SBQQ__ListPrice__c": 100.0,
                    "SBQQ__NetPrice__c": 90.0,
                    "SBQQ__NetTotal__c": 180.0
                }
            ]
        });

        let quote: QuoteModel = serde_json::from_value(json).must();
        assert_eq!(quote.line_items.len(), 1);
        let line = &quote.line_items[0];
        assert_eq!(line.id.as_deref(), Some("a0y000000000001AAA"));
        assert_eq!(line.product_id.as_deref(), Some("01t000000000001AAA"));
        assert_eq!(line.quantity, Some(2.0));
        assert_eq!(line.net_total, Some(180.0));
    }

    #[test]
    fn test_quote_model_roundtrip() {
        let original = QuoteModel {
            id: Some("a0x000000000001AAA".to_string()),
            account_id: None,
            opportunity_id: None,
            primary: Some(true),
            status: Some("Draft".to_string()),
            net_amount: Some(1000.0),
            list_amount: None,
            customer_discount: None,
            pricebook_id: None,
            start_date: None,
            end_date: None,
            subscription_term: None,
            line_items: vec![],
            extra: HashMap::new(),
        };

        let json = serde_json::to_string(&original).must();
        let deserialized: QuoteModel = serde_json::from_str(&json).must();
        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_quote_model_default_line_items() {
        let json = serde_json::json!({"Id": "a0x000000000001AAA"});
        let quote: QuoteModel = serde_json::from_value(json).must();
        assert!(quote.line_items.is_empty());
    }

    // ── QuoteLineModel tests ─────────────────────────────────────────

    #[test]
    fn test_quote_line_model_deserialize() {
        let json = serde_json::json!({
            "Id": "a0y000000000001AAA",
            "SBQQ__Product__c": "01t000000000001AAA",
            "SBQQ__Quantity__c": 5.0,
            "SBQQ__ListPrice__c": 200.0,
            "SBQQ__NetPrice__c": 180.0,
            "SBQQ__NetTotal__c": 900.0,
            "SBQQ__Discount__c": 10.0,
            "SBQQ__Optional__c": false,
            "SBQQ__Quote__c": "a0x000000000001AAA"
        });

        let line: QuoteLineModel = serde_json::from_value(json).must();
        assert_eq!(line.quantity, Some(5.0));
        assert_eq!(line.discount, Some(10.0));
        assert_eq!(line.optional, Some(false));
        assert_eq!(line.quote_id.as_deref(), Some("a0x000000000001AAA"));
    }

    #[test]
    fn test_quote_line_captures_extras() {
        let json = serde_json::json!({
            "Id": "a0y000000000001AAA",
            "CustomLineField__c": "extra_data"
        });

        let line: QuoteLineModel = serde_json::from_value(json).must();
        assert_eq!(line.extra["CustomLineField__c"], "extra_data");
    }

    // ── ProductModel tests ───────────────────────────────────────────

    #[test]
    fn test_product_model_deserialize() {
        let json = serde_json::json!({
            "record": {"Id": "01t000000000001AAA", "Name": "Widget"},
            "featureCategories": [{"name": "Base"}],
            "features": [],
            "options": [{"id": "opt1"}]
        });

        let product: ProductModel = serde_json::from_value(json).must();
        assert!(product.record.is_some());
        assert_eq!(product.feature_categories.len(), 1);
        assert!(product.features.is_empty());
        assert_eq!(product.options.len(), 1);
    }

    // ── ConfigurationModel tests ─────────────────────────────────────

    #[test]
    fn test_configuration_model_deserialize() {
        let json = serde_json::json!({
            "configuredProductId": "01t000000000001AAA",
            "optionConfigurations": [{"optionId": "opt1", "selected": true}],
            "valid": true,
            "validationMessages": []
        });

        let config: ConfigurationModel = serde_json::from_value(json).must();
        assert_eq!(
            config.configured_product_id.as_deref(),
            Some("01t000000000001AAA")
        );
        assert_eq!(config.option_configurations.len(), 1);
        assert_eq!(config.valid, Some(true));
        assert!(config.validation_messages.is_empty());
    }

    #[test]
    fn test_configuration_model_invalid_with_messages() {
        let json = serde_json::json!({
            "configuredProductId": "01t000000000001AAA",
            "optionConfigurations": [],
            "valid": false,
            "validationMessages": ["Required option missing", "Minimum quantity not met"]
        });

        let config: ConfigurationModel = serde_json::from_value(json).must();
        assert_eq!(config.valid, Some(false));
        assert_eq!(config.validation_messages.len(), 2);
    }

    // ── ServiceRouterRequest tests ───────────────────────────────────

    #[test]
    fn test_service_router_request_new() {
        let model = serde_json::json!({"quoteId": "a0x000000000001AAA"});
        let req = ServiceRouterRequest::new("SBQQ.QuoteAPI.QuoteReader", &model).must();

        assert_eq!(req.saver, "SBQQ.QuoteAPI.QuoteReader");
        // model should be a JSON string, not an object
        let parsed: serde_json::Value = serde_json::from_str(&req.model).must();
        assert_eq!(parsed["quoteId"], "a0x000000000001AAA");
    }

    #[test]
    fn test_service_router_request_serializes_double_json() {
        let model = serde_json::json!({"quoteId": "a0x000000000001AAA"});
        let req = ServiceRouterRequest::new("SBQQ.QuoteAPI.QuoteReader", &model).must();

        let serialized = serde_json::to_value(&req).must();
        // The outer "model" value must be a string, not an object
        assert!(serialized["model"].is_string());
    }

    #[test]
    fn test_service_router_request_from_raw() {
        let raw = r#"{"quoteId":"a0x000000000001AAA"}"#.to_string();
        let req = ServiceRouterRequest::from_raw("SBQQ.QuoteAPI.QuoteReader", raw);
        assert_eq!(req.saver, "SBQQ.QuoteAPI.QuoteReader");
        assert!(req.model.contains("quoteId"));
    }

    // ── AddProductsRequest tests ─────────────────────────────────────

    #[test]
    fn test_add_products_request_new() {
        let req = AddProductsRequest::new(vec![
            "01t000000000001AAA".to_string(),
            "01t000000000002AAA".to_string(),
        ]);

        assert_eq!(req.product_ids.len(), 2);
        assert!(!req.ignore_calculate);
        assert!(req.pricing_method.is_none());
    }

    // ── GenerateDocumentRequest tests ────────────────────────────────

    #[test]
    fn test_generate_document_request_builder() {
        let req = GenerateDocumentRequest::new("a0x000000000001AAA")
            .with_template("a0z000000000001AAA")
            .with_format("pdf");

        assert_eq!(req.quote_id, "a0x000000000001AAA");
        assert_eq!(req.template_id.as_deref(), Some("a0z000000000001AAA"));
        assert_eq!(req.format.as_deref(), Some("pdf"));
    }
}
