//! Portability API endpoints.
//!
//! Provides GDPR/CCPA data subject access request (DSAR) portability
//! operations for compiling and downloading personal data exports.

use super::ConsentHandler;
use super::types::{PortabilityRequest, PortabilityResponse};
use crate::error::{HttpError, Result};

impl<A: crate::auth::Authenticator> ConsentHandler<A> {
    /// Requests compilation of data for a portability export.
    ///
    /// Initiates an asynchronous data compilation for the specified records.
    /// The response contains a `request_id` that can be polled with
    /// [`check_portability_status`](Self::check_portability_status).
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let req = PortabilityRequest::new("Contact", vec!["003xx000003GYk1".into()]);
    /// let resp = client.consent().request_portability(&req).await?;
    /// println!("Request ID: {}", resp.request_id);
    /// ```
    pub async fn request_portability(
        &self,
        request: &PortabilityRequest,
    ) -> Result<PortabilityResponse> {
        let url = self.resolve_portability_url("").await?;
        let http_request = self
            .inner
            .post(&url)
            .json(request)
            .build()
            .map_err(HttpError::from)?;
        self.inner
            .send_request_and_decode(http_request, "Portability request failed")
            .await
    }

    /// Checks the status of a portability compilation request.
    ///
    /// Poll this endpoint to determine when the data export is ready
    /// for download. When `status` is [`Complete`](super::PortabilityStatus::Complete),
    /// the `download_url` field will be populated.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the request ID is not found.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let status = client.consent()
    ///     .check_portability_status("req-001")
    ///     .await?;
    ///
    /// match status.status {
    ///     PortabilityStatus::Complete => println!("Download: {}", status.download_url.unwrap()),
    ///     PortabilityStatus::Pending => println!("Still compiling..."),
    ///     PortabilityStatus::Failed => println!("Export failed"),
    /// }
    /// ```
    pub async fn check_portability_status(&self, request_id: &str) -> Result<PortabilityResponse> {
        let url = self.resolve_portability_url(request_id).await?;
        let request = self.inner.get(&url).build().map_err(HttpError::from)?;
        self.inner
            .send_request_and_decode(request, "Portability status check failed")
            .await
    }
}

#[cfg(test)]
mod tests {
    use crate::api::consent::{PortabilityRequest, PortabilityStatus};
    use crate::test_support::{MockAuthenticator, Must};
    use wiremock::matchers::{method, path};
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

    // ── request_portability tests ────────────────────────────────────

    #[tokio::test]
    async fn test_request_portability_success() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/portability"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "requestId": "req-001",
                "status": "Pending"
            })))
            .mount(&server)
            .await;

        let req = PortabilityRequest::new("Contact", vec!["003xx000003GYk1".to_string()]);
        let resp = client.consent().request_portability(&req).await.must();

        assert_eq!(resp.request_id, "req-001");
        assert_eq!(resp.status, PortabilityStatus::Pending);
        assert!(resp.download_url.is_none());
    }

    #[tokio::test]
    async fn test_request_portability_error() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/portability"))
            .respond_with(
                ResponseTemplate::new(400).set_body_json(serde_json::json!([{
                    "message": "Invalid object type",
                    "errorCode": "INVALID_TYPE"
                }])),
            )
            .mount(&server)
            .await;

        let req = PortabilityRequest::new("InvalidObject", vec!["003xx000003GYk1".to_string()]);
        let result = client.consent().request_portability(&req).await;
        assert!(result.is_err());
    }

    // ── check_portability_status tests ───────────────────────────────

    #[tokio::test]
    async fn test_check_portability_status_pending() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/portability/req-001"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "requestId": "req-001",
                "status": "Pending"
            })))
            .mount(&server)
            .await;

        let resp = client
            .consent()
            .check_portability_status("req-001")
            .await
            .must();

        assert_eq!(resp.status, PortabilityStatus::Pending);
        assert!(resp.download_url.is_none());
    }

    #[tokio::test]
    async fn test_check_portability_status_complete() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/portability/req-001"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "requestId": "req-001",
                "status": "Complete",
                "downloadUrl": "https://instance.salesforce.com/download/req-001"
            })))
            .mount(&server)
            .await;

        let resp = client
            .consent()
            .check_portability_status("req-001")
            .await
            .must();

        assert_eq!(resp.status, PortabilityStatus::Complete);
        assert_eq!(
            resp.download_url.as_deref(),
            Some("https://instance.salesforce.com/download/req-001")
        );
    }

    #[tokio::test]
    async fn test_check_portability_status_failed() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/portability/req-002"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "requestId": "req-002",
                "status": "Failed"
            })))
            .mount(&server)
            .await;

        let resp = client
            .consent()
            .check_portability_status("req-002")
            .await
            .must();

        assert_eq!(resp.status, PortabilityStatus::Failed);
        assert!(resp.download_url.is_none());
    }

    #[tokio::test]
    async fn test_check_portability_status_not_found() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/portability/nonexistent"))
            .respond_with(
                ResponseTemplate::new(404).set_body_json(serde_json::json!([{
                    "message": "Request not found",
                    "errorCode": "NOT_FOUND"
                }])),
            )
            .mount(&server)
            .await;

        let result = client
            .consent()
            .check_portability_status("nonexistent")
            .await;
        assert!(result.is_err());
    }
}
