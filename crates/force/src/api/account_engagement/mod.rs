//! Salesforce Account Engagement (Pardot) API v5 handler.
//!
//! Account Engagement (formerly Pardot) exposes a v5 object model at a host that
//! is **separate** from the Salesforce org instance URL:
//!
//! | Environment | Host |
//! |---|---|
//! | Production / training | `https://pi.pardot.com` |
//! | Sandbox / demo / developer | `https://pi.demo.pardot.com` |
//!
//! Requests ride the standard Salesforce OAuth access token (`Authorization:
//! Bearer …`, injected centrally) but additionally require a
//! `Pardot-Business-Unit-Id` header selecting which business unit's data to
//! access, and the connected app must include the `pardot_api` OAuth scope.
//!
//! # Feature Flag
//!
//! This module requires the `account_engagement` feature flag:
//! ```toml
//! [dependencies]
//! force = { version = "...", features = ["account_engagement"] }
//! ```
//!
//! # Usage
//!
//! ```ignore
//! let client = builder().authenticate(auth).build().await?;
//! let ae = client.account_engagement("0Uv000000000001AAA");
//!
//! // Query prospects (the `fields` list is REQUIRED to return any data)
//! let page = ae
//!     .query_prospects("id,email,firstName,lastName", &[("limit", "50")])
//!     .await?;
//! for prospect in page.values {
//!     println!("{:?}", prospect.email);
//! }
//! ```
//!
//! See [ADR-029](../../../docs/adr/029-account-engagement-api-design.md).
#![allow(clippy::doc_markdown)]

pub(crate) mod campaigns;
pub(crate) mod custom_fields;
pub(crate) mod emails;
pub(crate) mod forms;
pub(crate) mod lists;
pub(crate) mod prospects;
pub(crate) mod types;

pub use campaigns::Campaign;
pub use custom_fields::CustomField;
pub use emails::Email;
pub use forms::Form;
pub use lists::{List, ListMembership};
pub use prospects::Prospect;
pub use types::QueryResponse;

use crate::config::Environment;
use crate::error::{HttpError, Result};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::sync::Arc;

/// Production / training Account Engagement host.
pub const PROD_HOST: &str = "https://pi.pardot.com";
/// Sandbox / demo / developer Account Engagement host.
pub const DEMO_HOST: &str = "https://pi.demo.pardot.com";

/// Required per-request header selecting the Account Engagement business unit.
const BU_HEADER: &str = "Pardot-Business-Unit-Id";

/// Selects the Account Engagement host for a given login environment.
///
/// `Environment::Sandbox` maps to the demo host; everything else
/// (`Production`, `Custom`) maps to the production host.
const fn host_for_environment(env: &Environment) -> &'static str {
    match env {
        Environment::Sandbox => DEMO_HOST,
        _ => PROD_HOST,
    }
}

/// Account Engagement (Pardot) API v5 handler.
///
/// Provides typed access to the core v5 object model (prospects, lists, list
/// memberships, campaigns, custom fields, forms, and emails) plus a generic
/// escape hatch (`get_raw`/`post_raw`/`patch_raw`/`delete_raw`) for any object
/// not yet modeled.
///
/// Obtain a handler from
/// [`ForceClient::account_engagement`](crate::client::ForceClient::account_engagement).
#[derive(Debug)]
pub struct AccountEngagementHandler<A: crate::auth::Authenticator> {
    /// Shared session state (carries the OAuth token + HTTP client).
    inner: Arc<crate::session::Session<A>>,
    /// The 18-char `0Uv…` business unit id sent as the `Pardot-Business-Unit-Id` header.
    business_unit_id: String,
    /// Full scheme + host the v5 API is served from (e.g. `https://pi.pardot.com`).
    base_url: String,
}

impl<A: crate::auth::Authenticator> Clone for AccountEngagementHandler<A> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            business_unit_id: self.business_unit_id.clone(),
            base_url: self.base_url.clone(),
        }
    }
}

impl<A: crate::auth::Authenticator> AccountEngagementHandler<A> {
    /// Creates a new handler, deriving the host from the client environment.
    #[must_use]
    pub(crate) fn new(inner: Arc<crate::session::Session<A>>, business_unit_id: String) -> Self {
        let base_url = host_for_environment(&inner.config.environment).to_string();
        Self {
            inner,
            business_unit_id,
            base_url,
        }
    }

    /// Overrides the Account Engagement host.
    ///
    /// Use this to point at `https://pi.demo.pardot.com`, a custom domain, or a
    /// mock server in tests. Pass a full scheme + host with no trailing slash,
    /// e.g. `"https://pi.demo.pardot.com"`.
    #[must_use]
    pub fn with_host(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Builds an absolute v5 URL: `{base_url}/api/v5/{path}`.
    fn ae_url(&self, path: &str) -> String {
        format!("{}/api/v5/{}", self.base_url, path.trim_start_matches('/'))
    }

    // ── Low-level HTTP helpers (absolute URL + BU header) ─────────────

    async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: Option<&[(&str, &str)]>,
        error_msg: &str,
    ) -> Result<T> {
        let url = self.ae_url(path);
        let mut req = self
            .inner
            .get(&url)
            .header(BU_HEADER, self.business_unit_id.as_str());
        if let Some(params) = query {
            req = req.query(params);
        }
        let request = req.build().map_err(HttpError::from)?;
        self.inner.send_request_and_decode(request, error_msg).await
    }

    async fn post<T: DeserializeOwned, B: Serialize + Sync>(
        &self,
        path: &str,
        body: &B,
        error_msg: &str,
    ) -> Result<T> {
        let url = self.ae_url(path);
        let request = self
            .inner
            .post(&url)
            .header(BU_HEADER, self.business_unit_id.as_str())
            .json(body)
            .build()
            .map_err(HttpError::from)?;
        self.inner.send_request_and_decode(request, error_msg).await
    }

    async fn patch<T: DeserializeOwned, B: Serialize + Sync>(
        &self,
        path: &str,
        body: &B,
        error_msg: &str,
    ) -> Result<T> {
        let url = self.ae_url(path);
        let request = self
            .inner
            .patch(&url)
            .header(BU_HEADER, self.business_unit_id.as_str())
            .json(body)
            .build()
            .map_err(HttpError::from)?;
        self.inner.send_request_and_decode(request, error_msg).await
    }

    /// DELETE; expects a 2xx (typically 204 No Content) with no body decode.
    async fn delete_empty(&self, path: &str, error_msg: &str) -> Result<()> {
        let url = self.ae_url(path);
        let request = self
            .inner
            .delete(&url)
            .header(BU_HEADER, self.business_unit_id.as_str())
            .build()
            .map_err(HttpError::from)?;
        self.inner
            .execute_and_check_success(request, error_msg)
            .await?;
        Ok(())
    }

    // ── Generic object operations (used by the typed modules) ─────────

    /// Query an object collection. Always pass `fields` among `params`.
    async fn query_objects<T: DeserializeOwned>(
        &self,
        object: &str,
        params: &[(&str, &str)],
    ) -> Result<QueryResponse<T>> {
        self.get(object, Some(params), "Account Engagement query failed")
            .await
    }

    /// Read a single object by id, requesting `fields`.
    async fn read_object<T: DeserializeOwned>(
        &self,
        object: &str,
        id: &str,
        fields: &str,
    ) -> Result<T> {
        let path = format!("{object}/{id}");
        self.get(
            &path,
            Some(&[("fields", fields)]),
            "Account Engagement read failed",
        )
        .await
    }

    /// Create an object.
    async fn create_object<T: DeserializeOwned, B: Serialize + Sync>(
        &self,
        object: &str,
        body: &B,
    ) -> Result<T> {
        self.post(object, body, "Account Engagement create failed")
            .await
    }

    /// Update an object by id.
    async fn update_object<T: DeserializeOwned, B: Serialize + Sync>(
        &self,
        object: &str,
        id: &str,
        body: &B,
    ) -> Result<T> {
        let path = format!("{object}/{id}");
        self.patch(&path, body, "Account Engagement update failed")
            .await
    }

    /// Delete an object by id.
    async fn delete_object(&self, object: &str, id: &str) -> Result<()> {
        let path = format!("{object}/{id}");
        self.delete_empty(&path, "Account Engagement delete failed")
            .await
    }

    // ── Generic escape hatch (public) ─────────────────────────────────

    /// GET any v5 endpoint, returning the raw JSON body.
    ///
    /// `path` is relative to `/api/v5/`, e.g. `"objects/visits"`. Remember that
    /// the API requires a `fields` query parameter to return object data.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response is not valid JSON.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let visits = client
    ///     .account_engagement("0Uv…")
    ///     .get_raw("objects/visits", Some(&[("fields", "id,prospectId")]))
    ///     .await?;
    /// ```
    pub async fn get_raw(
        &self,
        path: &str,
        query: Option<&[(&str, &str)]>,
    ) -> Result<serde_json::Value> {
        self.get(path, query, "Account Engagement GET failed").await
    }

    /// POST to any v5 endpoint, returning the raw JSON body.
    ///
    /// `path` is relative to `/api/v5/`, e.g. `"objects/prospects"`.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response is not valid JSON.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let created = client
    ///     .account_engagement("0Uv…")
    ///     .post_raw("objects/prospects", &serde_json::json!({"email": "a@b.com"}))
    ///     .await?;
    /// ```
    pub async fn post_raw(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.post(path, body, "Account Engagement POST failed")
            .await
    }

    /// PATCH any v5 endpoint, returning the raw JSON body.
    ///
    /// `path` is relative to `/api/v5/`, e.g. `"objects/prospects/42"`.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response is not valid JSON.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let updated = client
    ///     .account_engagement("0Uv…")
    ///     .patch_raw("objects/prospects/42", &serde_json::json!({"score": 10}))
    ///     .await?;
    /// ```
    pub async fn patch_raw(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.patch(path, body, "Account Engagement PATCH failed")
            .await
    }

    /// DELETE any v5 endpoint.
    ///
    /// `path` is relative to `/api/v5/`, e.g. `"objects/prospects/42"`. Expects a
    /// 2xx response (typically 204 No Content).
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or returns a non-success status.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// client
    ///     .account_engagement("0Uv…")
    ///     .delete_raw("objects/prospects/42")
    ///     .await?;
    /// ```
    pub async fn delete_raw(&self, path: &str) -> Result<()> {
        self.delete_empty(path, "Account Engagement DELETE failed")
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Environment;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const TEST_BU: &str = "0Uv000000000001AAA";

    /// Builds a handler pointed at the mock server via `with_host`.
    async fn handler_for(server: &MockServer) -> AccountEngagementHandler<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", &server.uri());
        let client = crate::client::builder()
            .authenticate(auth)
            .build()
            .await
            .must();
        client.account_engagement(TEST_BU).with_host(server.uri())
    }

    // ── Host derivation ───────────────────────────────────────────────

    #[test]
    fn test_host_for_environment_production() {
        assert_eq!(host_for_environment(&Environment::Production), PROD_HOST);
    }

    #[test]
    fn test_host_for_environment_sandbox() {
        assert_eq!(host_for_environment(&Environment::Sandbox), DEMO_HOST);
    }

    #[test]
    fn test_host_for_environment_custom_uses_prod() {
        let env = Environment::Custom("https://my.example.com".to_string());
        assert_eq!(host_for_environment(&env), PROD_HOST);
    }

    #[tokio::test]
    async fn test_new_derives_prod_host_by_default() {
        let auth = MockAuthenticator::new("test_token", "https://na1.salesforce.com");
        let client = crate::client::builder()
            .authenticate(auth)
            .build()
            .await
            .must();
        // Default environment is Production.
        let handler = client.account_engagement(TEST_BU);
        assert_eq!(handler.base_url, PROD_HOST);
    }

    #[tokio::test]
    async fn test_with_host_overrides_base_url() {
        let auth = MockAuthenticator::new("test_token", "https://na1.salesforce.com");
        let client = crate::client::builder()
            .authenticate(auth)
            .build()
            .await
            .must();
        let handler = client
            .account_engagement(TEST_BU)
            .with_host("https://pi.demo.pardot.com");
        assert_eq!(handler.base_url, DEMO_HOST);
        assert_eq!(
            handler.ae_url("objects/prospects"),
            "https://pi.demo.pardot.com/api/v5/objects/prospects"
        );
    }

    // ── Escape hatch ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_raw_success_sends_bu_header() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v5/objects/visits"))
            .and(header(BU_HEADER, TEST_BU))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"values": [{"id": 7}]})),
            )
            .expect(1)
            .mount(&server)
            .await;

        let handler = handler_for(&server).await;
        let value = handler
            .get_raw("objects/visits", Some(&[("fields", "id,prospectId")]))
            .await
            .must();
        assert_eq!(value["values"][0]["id"], 7);
    }

    #[tokio::test]
    async fn test_post_raw_success() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v5/objects/prospects"))
            .and(header(BU_HEADER, TEST_BU))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"id": 99})))
            .mount(&server)
            .await;

        let handler = handler_for(&server).await;
        let value = handler
            .post_raw(
                "objects/prospects",
                &serde_json::json!({"email": "a@b.com"}),
            )
            .await
            .must();
        assert_eq!(value["id"], 99);
    }

    #[tokio::test]
    async fn test_delete_raw_204() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/api/v5/objects/prospects/42"))
            .and(header(BU_HEADER, TEST_BU))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let handler = handler_for(&server).await;
        handler.delete_raw("objects/prospects/42").await.must();
    }

    // ── Error path ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_error_body_returns_err() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v5/objects/visits"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(serde_json::json!({"code": 60, "message": "Invalid request"})),
            )
            .mount(&server)
            .await;

        let handler = handler_for(&server).await;
        let result = handler.get_raw("objects/visits", None).await;
        assert!(result.is_err());
    }
}
