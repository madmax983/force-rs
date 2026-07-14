//! OAuth 2.0 SAML Bearer Assertion Flow.
//!
//! This module implements the OAuth 2.0 SAML Bearer Assertion Flow for server-to-server
//! authentication with Salesforce. This allows enterprise integrations to reuse existing
//! SAML 2.0 Identity Provider assertions to negotiate an access token.
//!
//! # Example
//!
//! ```ignore
//! use force::auth::SamlBearerFlow;
//!
//! let flow = SamlBearerFlow::builder()
//!     .assertion("your_base64_encoded_saml_assertion")
//!     .production() // or .sandbox()
//!     .build()?;
//!
//! let token = flow.authenticate().await?;
//! ```

use crate::auth::authenticator::Authenticator;
use crate::auth::token::{AccessToken, TokenResponse};
use crate::error::{AuthenticationError, ForceError, HttpError, Result};
use async_trait::async_trait;

/// Configuration builder for the SAML Bearer flow.
#[derive(Debug, Default)]
pub struct SamlBearerFlowBuilder {
    assertion: Option<String>,
    token_url: Option<String>,
}

impl SamlBearerFlowBuilder {
    /// Sets the base64-encoded signed SAML 2.0 assertion.
    #[must_use]
    pub fn assertion(mut self, assertion: impl Into<String>) -> Self {
        self.assertion = Some(assertion.into());
        self
    }

    /// Sets the token URL.
    #[must_use]
    pub fn token_url(mut self, url: impl Into<String>) -> Self {
        self.token_url = Some(url.into());
        self
    }

    /// Configures the flow to use the standard production login URL.
    #[must_use]
    pub fn production(mut self) -> Self {
        self.token_url = Some(crate::auth::PRODUCTION_TOKEN_URL.to_string());
        self
    }

    /// Configures the flow to use the standard sandbox login URL.
    #[must_use]
    pub fn sandbox(mut self) -> Self {
        self.token_url = Some(crate::auth::SANDBOX_TOKEN_URL.to_string());
        self
    }

    /// Builds the `SamlBearerFlow` authenticator.
    ///
    /// # Errors
    /// Returns an error if required fields (assertion, token_url) are missing.
    pub fn build(self) -> Result<SamlBearerFlow> {
        let assertion = self.assertion.ok_or_else(|| {
            ForceError::Authentication(AuthenticationError::TokenRequestFailed(
                "missing assertion".into(),
            ))
        })?;

        let token_url = self.token_url.ok_or_else(|| {
            ForceError::Authentication(AuthenticationError::TokenRequestFailed(
                "missing token_url".into(),
            ))
        })?;

        let http_client = reqwest::Client::builder()
            .build()
            .map_err(|e| ForceError::Http(HttpError::RequestFailed(e)))?;

        Ok(SamlBearerFlow {
            assertion,
            token_url,
            http_client,
        })
    }
}

/// Authenticator implementing the SAML Bearer flow.
#[derive(Debug)]
pub struct SamlBearerFlow {
    assertion: String,
    token_url: String,
    http_client: reqwest::Client,
}

impl SamlBearerFlow {
    /// Creates a new builder for the SAML Bearer flow.
    #[must_use]
    pub fn builder() -> SamlBearerFlowBuilder {
        SamlBearerFlowBuilder::default()
    }
}

#[async_trait]
impl Authenticator for SamlBearerFlow {
    async fn authenticate(&self) -> Result<AccessToken> {
        let params = [
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:saml2-bearer",
            ),
            ("assertion", &self.assertion),
        ];

        let response = self
            .http_client
            .post(&self.token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| ForceError::Http(HttpError::RequestFailed(e)))?;

        if !response.status().is_success() {
            return Err(crate::auth::handle_oauth_error(response, None).await);
        }

        let bytes = crate::http::error::read_capped_body_bytes(response, 1024 * 1024).await?;
        let token_res = serde_json::from_slice::<TokenResponse>(&bytes)
            .map_err(crate::error::SerializationError::from)?;

        Ok(AccessToken::from_response(token_res))
    }

    async fn refresh(&self) -> Result<AccessToken> {
        self.authenticate().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::must::MustMsg;
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn test_builder_missing_assertion() {
        let Err(err) = SamlBearerFlow::builder().production().build() else {
            panic!("Expected an error when assertion is missing");
        };
        assert!(err.to_string().contains("missing assertion"));
    }

    #[test]
    fn test_builder_missing_token_url() {
        let Err(err) = SamlBearerFlow::builder()
            .assertion("test_assertion")
            .build()
        else {
            panic!("Expected an error when token_url is missing");
        };
        assert!(err.to_string().contains("missing token_url"));
    }

    #[test]
    fn test_builder_production_url() {
        let flow = SamlBearerFlow::builder()
            .assertion("test_assertion")
            .production()
            .build()
            .must_msg("Failed to build flow");

        assert_eq!(flow.token_url, crate::auth::PRODUCTION_TOKEN_URL);
    }

    #[test]
    fn test_builder_sandbox_url() {
        let flow = SamlBearerFlow::builder()
            .assertion("test_assertion")
            .sandbox()
            .build()
            .must_msg("Failed to build flow");

        assert_eq!(flow.token_url, crate::auth::SANDBOX_TOKEN_URL);
    }

    #[tokio::test]
    async fn test_authenticate_success() {
        let mock_server = MockServer::start().await;

        let success_response = serde_json::json!({
            "access_token": "mock_access_token",
            "instance_url": "https://test.salesforce.com",
            "token_type": "Bearer"
        });

        Mock::given(method("POST"))
            .and(path("/services/oauth2/token"))
            .and(body_string_contains(
                "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Asaml2-bearer",
            ))
            .and(body_string_contains("assertion=mock_assertion"))
            .respond_with(ResponseTemplate::new(200).set_body_json(success_response))
            .mount(&mock_server)
            .await;

        let token_url = format!("{}/services/oauth2/token", mock_server.uri());
        let flow = SamlBearerFlow::builder()
            .assertion("mock_assertion")
            .token_url(&token_url)
            .build()
            .must_msg("Failed to build flow");

        let token = flow.authenticate().await.must_msg("Failed to authenticate");
        assert_eq!(token.as_str(), "mock_access_token");
        assert_eq!(token.instance_url(), "https://test.salesforce.com");
    }

    #[tokio::test]
    async fn test_authenticate_failure() {
        let mock_server = MockServer::start().await;

        let error_response = serde_json::json!({
            "error": "invalid_grant",
            "error_description": "invalid assertion"
        });

        Mock::given(method("POST"))
            .and(path("/services/oauth2/token"))
            .respond_with(ResponseTemplate::new(400).set_body_json(error_response))
            .mount(&mock_server)
            .await;

        let token_url = format!("{}/services/oauth2/token", mock_server.uri());
        let flow = SamlBearerFlow::builder()
            .assertion("bad_assertion")
            .token_url(&token_url)
            .build()
            .must_msg("Failed to build flow");

        let Err(err) = flow.authenticate().await else {
            panic!("Expected an error on invalid grant");
        };
        assert!(err.to_string().contains("invalid_grant"));
    }
}
