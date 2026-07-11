//! CPQ Document generation operations.
//!
//! Provides quote document/proposal generation via the ServiceRouter.

use super::CpqHandler;
use super::types::{GenerateDocumentRequest, ServiceRouterRequest};
use crate::error::Result;

/// CPQ Document API loader class name.
const GENERATE_PROPOSAL: &str = "SBQQ.QuoteAPI.GenerateProposal";

impl<A: crate::auth::Authenticator> CpqHandler<A> {
    /// Generates a quote document/proposal.
    ///
    /// Creates a document for the specified quote using the CPQ document
    /// generation engine. Returns raw JSON since the response shape varies
    /// by template and configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if document generation fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let request = GenerateDocumentRequest::new("a0x000000000001AAA")
    ///     .with_template("a0z000000000001AAA")
    ///     .with_format("pdf");
    ///
    /// let result = client.cpq().generate_document(&request).await?;
    /// ```
    pub async fn generate_document(
        &self,
        request: &GenerateDocumentRequest,
    ) -> Result<serde_json::Value> {
        let envelope = ServiceRouterRequest::new(GENERATE_PROPOSAL, request).map_err(|e| {
            crate::error::ForceError::Serialization(crate::error::SerializationError::from(e))
        })?;

        self.service_router_post(GENERATE_PROPOSAL, &envelope).await
    }
}

#[cfg(test)]
mod tests {
    use super::GENERATE_PROPOSAL;
    use crate::api::cpq::GenerateDocumentRequest;
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
    async fn test_generate_document_success() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/apexrest/SBQQ/ServiceRouter"))
            .and(query_param("loader", GENERATE_PROPOSAL))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "documentId": "069000000000001AAA",
                "status": "Generated",
                "contentVersionId": "068000000000001AAA"
            })))
            .mount(&server)
            .await;

        let request = GenerateDocumentRequest::new("a0x000000000001AAA")
            .with_template("a0z000000000001AAA")
            .with_format("pdf");

        let result = client.cpq().generate_document(&request).await.must();
        assert_eq!(result["documentId"], "069000000000001AAA");
        assert_eq!(result["status"], "Generated");
    }

    #[tokio::test]
    async fn test_generate_document_minimal_request() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/apexrest/SBQQ/ServiceRouter"))
            .and(query_param("loader", GENERATE_PROPOSAL))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "documentId": "069000000000001AAA",
                "status": "Generated"
            })))
            .mount(&server)
            .await;

        let request = GenerateDocumentRequest::new("a0x000000000001AAA");
        let result = client.cpq().generate_document(&request).await.must();
        assert_eq!(result["status"], "Generated");
    }

    #[tokio::test]
    async fn test_generate_document_error() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/apexrest/SBQQ/ServiceRouter"))
            .and(query_param("loader", GENERATE_PROPOSAL))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "message": "Template not found",
                "errorCode": "INVALID_TEMPLATE"
            })))
            .mount(&server)
            .await;

        let request =
            GenerateDocumentRequest::new("a0x000000000001AAA").with_template("invalid_template");

        let result = client.cpq().generate_document(&request).await;
        assert!(result.is_err());
    }
}
