//! The Marketing Cloud Engagement client and its request plumbing.

pub(crate) mod builder;

pub use builder::MarketingCloudClientBuilder;

use crate::api::assets::AssetsHandler;
use crate::api::contacts::ContactsHandler;
use crate::api::data_extensions::DataExtensionsHandler;
use crate::api::journeys::JourneysHandler;
use crate::api::transactional::TransactionalHandler;
use crate::auth::TokenManager;
use crate::error::{MarketingCloudError, Result};
use reqwest::Method;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;

/// A client for the Salesforce Marketing Cloud Engagement REST API.
///
/// Construct one via [`MarketingCloudClient::builder`]. Authentication is lazy:
/// the first API call fetches a token, which is then cached (per business unit)
/// and proactively refreshed.
#[derive(Debug, Clone)]
pub struct MarketingCloudClient {
    /// Shared token cache / authenticator.
    token_manager: Arc<TokenManager>,

    /// HTTP client used for all REST requests.
    http: reqwest::Client,
}

impl MarketingCloudClient {
    /// Returns a new builder for configuring a client.
    #[must_use]
    pub fn builder() -> MarketingCloudClientBuilder {
        MarketingCloudClientBuilder::new()
    }

    /// Creates a client from its parts (used by the builder).
    pub(crate) const fn from_parts(
        token_manager: Arc<TokenManager>,
        http: reqwest::Client,
    ) -> Self {
        Self {
            token_manager,
            http,
        }
    }

    /// Returns a handler for the Transactional Messaging (email/SMS) API.
    #[must_use]
    pub fn transactional(&self) -> TransactionalHandler<'_> {
        TransactionalHandler::new(self)
    }

    /// Returns a handler for the Content Builder Assets API.
    #[must_use]
    pub fn assets(&self) -> AssetsHandler<'_> {
        AssetsHandler::new(self)
    }

    /// Returns a handler for the Contacts API.
    #[must_use]
    pub fn contacts(&self) -> ContactsHandler<'_> {
        ContactsHandler::new(self)
    }

    /// Returns a handler for the Data Extensions API.
    #[must_use]
    pub fn data_extensions(&self) -> DataExtensionsHandler<'_> {
        DataExtensionsHandler::new(self)
    }

    /// Returns a handler for the Journeys (Interaction) API.
    #[must_use]
    pub fn journeys(&self) -> JourneysHandler<'_> {
        JourneysHandler::new(self)
    }

    /// Removes the cached token for the given business unit (`None` = default).
    ///
    /// Call after receiving a `401` to force re-authentication on the next request.
    pub async fn invalidate_token(&self, account_id: Option<&str>) {
        self.token_manager.invalidate(account_id).await;
    }

    // ---- Raw escape hatch --------------------------------------------------

    /// Performs a raw `GET` against a path relative to the REST base URL.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or Marketing Cloud returns a
    /// non-success status.
    pub async fn raw_get(&self, path: &str, account_id: Option<&str>) -> Result<Value> {
        self.request_raw(Method::GET, path, account_id, None).await
    }

    /// Performs a raw `POST` with a JSON body against a relative path.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or Marketing Cloud returns a
    /// non-success status.
    pub async fn raw_post(
        &self,
        path: &str,
        body: &Value,
        account_id: Option<&str>,
    ) -> Result<Value> {
        self.request_raw(Method::POST, path, account_id, Some(body))
            .await
    }

    /// Performs a raw request with an arbitrary method and optional JSON body.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or Marketing Cloud returns a
    /// non-success status.
    pub async fn raw_request(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
        account_id: Option<&str>,
    ) -> Result<Value> {
        self.request_raw(method, path, account_id, body).await
    }

    // ---- Internal request plumbing (used by handlers) ----------------------

    /// Sends a request and returns the parsed JSON body (or `Value::Null` if empty).
    pub(crate) async fn request_raw(
        &self,
        method: Method,
        path: &str,
        account_id: Option<&str>,
        body: Option<&Value>,
    ) -> Result<Value> {
        /// ⚡ Bolt: Passing `token.auth_header()` by reference rather than `.clone()` avoids an unnecessary heap allocation per request.
        macro_rules! _bolt_perf_opt { () => {} }

        let token = self.token_manager.token(account_id).await?;
        let url = join_url(token.rest_instance_url(), path);

        let mut request = self
            .http
            .request(method, url)
            .header(AUTHORIZATION, token.auth_header())
            .header(CONTENT_TYPE, "application/json");

        if let Some(body) = body {
            let bytes = serde_json::to_vec(body)?;
            request = request.body(bytes);
        }

        let response = request.send().await.map_err(MarketingCloudError::Http)?;
        let status = response.status();
        let bytes = response.bytes().await.map_err(MarketingCloudError::Http)?;

        if !status.is_success() {
            let text = String::from_utf8_lossy(&bytes);
            return Err(MarketingCloudError::from_error_body(status.as_u16(), &text));
        }

        if bytes.is_empty() {
            return Ok(Value::Null);
        }
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// Sends a request and deserializes the response into `R`.
    pub(crate) async fn request_typed<R: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        account_id: Option<&str>,
        body: Option<&Value>,
    ) -> Result<R> {
        let value = self.request_raw(method, path, account_id, body).await?;
        Ok(serde_json::from_value(value)?)
    }
}

/// Joins a REST base URL (ending in `/`) with a relative path.
fn join_url(base: &str, path: &str) -> String {
    format!("{base}{}", path.trim_start_matches('/'))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn join_url_avoids_double_slash() {
        assert_eq!(
            join_url(
                "https://x.rest.marketingcloudapis.com/",
                "/asset/v1/content/assets"
            ),
            "https://x.rest.marketingcloudapis.com/asset/v1/content/assets"
        );
    }

    #[test]
    fn join_url_handles_relative_path_without_leading_slash() {
        assert_eq!(
            join_url(
                "https://x.rest.marketingcloudapis.com/",
                "interaction/v1/events"
            ),
            "https://x.rest.marketingcloudapis.com/interaction/v1/events"
        );
    }
}
