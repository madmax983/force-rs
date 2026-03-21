//! CPQ Contract amendment operations.
//!
//! Provides contract amendment via the ServiceRouter.

use super::CpqHandler;
use super::types::{QuoteModel, ServiceRouterRequest};
use crate::error::Result;

/// CPQ Contract API loader class name.
const CONTRACT_AMENDER: &str = "SBQQ.ContractManipulationAPI.ContractAmender";

impl<A: crate::auth::Authenticator> CpqHandler<A> {
    /// Creates an amendment quote from a contract.
    ///
    /// Generates a new [`QuoteModel`] representing an amendment to the
    /// specified contract. The amendment quote inherits line items and
    /// pricing from the original contract.
    ///
    /// # Errors
    ///
    /// Returns an error if the contract cannot be amended.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let amendment = client.cpq()
    ///     .amend_contract("800000000000001AAA")
    ///     .await?;
    /// println!("Amendment quote: {:?}", amendment.id);
    /// ```
    pub async fn amend_contract(&self, contract_id: &str) -> Result<QuoteModel> {
        let model = serde_json::json!({"contractId": contract_id});
        let envelope = ServiceRouterRequest::new(CONTRACT_AMENDER, &model).map_err(|e| {
            crate::error::ForceError::Serialization(crate::error::SerializationError::from(e))
        })?;

        self.service_router_post(CONTRACT_AMENDER, &envelope).await
    }
}

#[cfg(test)]
mod tests {
    use super::CONTRACT_AMENDER;
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
    async fn test_amend_contract_success() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/apexrest/SBQQ/ServiceRouter"))
            .and(query_param("loader", CONTRACT_AMENDER))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "Id": "a0x000000000099AAA",
                "SBQQ__Status__c": "Draft",
                "SBQQ__NetAmount__c": 5000.0,
                "lineItems": [
                    {
                        "Id": "a0y000000000099AAA",
                        "SBQQ__Product__c": "01t000000000001AAA",
                        "SBQQ__Quantity__c": 10.0,
                        "SBQQ__NetTotal__c": 5000.0
                    }
                ]
            })))
            .mount(&server)
            .await;

        let amendment = client
            .cpq()
            .amend_contract("800000000000001AAA")
            .await
            .must();
        assert_eq!(amendment.id.as_deref(), Some("a0x000000000099AAA"));
        assert_eq!(amendment.status.as_deref(), Some("Draft"));
        assert_eq!(amendment.net_amount, Some(5000.0));
        assert_eq!(amendment.line_items.len(), 1);
    }

    #[tokio::test]
    async fn test_amend_contract_not_found() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/apexrest/SBQQ/ServiceRouter"))
            .and(query_param("loader", CONTRACT_AMENDER))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "message": "Contract not found",
                "errorCode": "ENTITY_NOT_FOUND"
            })))
            .mount(&server)
            .await;

        let result = client.cpq().amend_contract("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_amend_contract_already_amended() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/apexrest/SBQQ/ServiceRouter"))
            .and(query_param("loader", CONTRACT_AMENDER))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "message": "Contract already has a pending amendment",
                "errorCode": "ALREADY_AMENDED"
            })))
            .mount(&server)
            .await;

        let result = client.cpq().amend_contract("800000000000001AAA").await;
        assert!(result.is_err());
    }
}
