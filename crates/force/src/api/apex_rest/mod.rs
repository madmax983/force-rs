//! Generic Apex REST API handler.
//!
//! The Apex REST handler provides generic HTTP access to any custom Apex REST
//! endpoint at `/services/apexrest/{path}`. This is the foundation for both
//! custom Apex REST classes and the Salesforce CPQ API.
//!
//! # Feature Flag
//!
//! This module requires the `apex_rest` feature flag:
//! ```toml
//! [dependencies]
//! force = { version = "...", features = ["apex_rest"] }
//! ```
//!
//! # Usage
//!
//! ```ignore
//! let client = builder().authenticate(auth).build().await?;
//! let apex = client.apex_rest();
//!
//! // Call a custom Apex REST endpoint
//! let result: MyResponse = apex.post("MyNamespace/MyEndpoint", &request_body).await?;
//!
//! // GET with query parameters
//! let data: Vec<Record> = apex.get_with_params("MyService/records", &[("limit", "10")]).await?;
//! ```

use crate::error::{HttpError, Result};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::sync::Arc;

/// Generic Apex REST API handler.
///
/// Provides typed HTTP access to any `/services/apexrest/{path}` endpoint.
/// This handler is the building block for calling custom Apex REST classes
/// deployed in a Salesforce org.
///
/// Obtain a handler from [`ForceClient::apex_rest`](crate::client::ForceClient::apex_rest).
#[derive(Debug)]
pub struct ApexRestHandler<A: crate::auth::Authenticator> {
    inner: Arc<crate::session::Session<A>>,
}

impl<A: crate::auth::Authenticator> Clone for ApexRestHandler<A> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<A: crate::auth::Authenticator> ApexRestHandler<A> {
    /// Creates a new Apex REST handler wrapping the given session.
    #[must_use]
    pub(crate) fn new(inner: Arc<crate::session::Session<A>>) -> Self {
        Self { inner }
    }

    /// GET an Apex REST endpoint and deserialize the JSON response.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be deserialized.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let records: Vec<MyRecord> = client.apex_rest()
    ///     .get("MyNamespace/records")
    ///     .await?;
    /// ```
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = self.inner.resolve_apex_rest_url(path).await?;
        let request = self.inner.get(&url).build().map_err(HttpError::from)?;
        self.inner
            .send_request_and_decode(request, "Apex REST GET failed")
            .await
    }

    /// GET an Apex REST endpoint with query parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be deserialized.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let records: Vec<MyRecord> = client.apex_rest()
    ///     .get_with_params("MyService/records", &[("limit", "10")])
    ///     .await?;
    /// ```
    pub async fn get_with_params<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<T> {
        let url = self.inner.resolve_apex_rest_url(path).await?;
        let request = self
            .inner
            .get(&url)
            .query(params)
            .build()
            .map_err(HttpError::from)?;
        self.inner
            .send_request_and_decode(request, "Apex REST GET failed")
            .await
    }

    /// POST to an Apex REST endpoint with a JSON body.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be deserialized.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let result: MyResponse = client.apex_rest()
    ///     .post("MyNamespace/MyEndpoint", &request_body)
    ///     .await?;
    /// ```
    pub async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &(impl Serialize + Sync),
    ) -> Result<T> {
        let url = self.inner.resolve_apex_rest_url(path).await?;
        let request = self
            .inner
            .post(&url)
            .json(body)
            .build()
            .map_err(HttpError::from)?;
        self.inner
            .send_request_and_decode(request, "Apex REST POST failed")
            .await
    }

    /// POST to an Apex REST endpoint, returning raw `serde_json::Value`.
    ///
    /// Useful for endpoints with dynamic or unknown response shapes.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response is not valid JSON.
    pub async fn post_raw(
        &self,
        path: &str,
        body: &(impl Serialize + Sync),
    ) -> Result<serde_json::Value> {
        self.post(path, body).await
    }

    /// PATCH an Apex REST endpoint with a JSON body.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be deserialized.
    pub async fn patch<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &(impl Serialize + Sync),
    ) -> Result<T> {
        let url = self.inner.resolve_apex_rest_url(path).await?;
        let request = self
            .inner
            .patch(&url)
            .json(body)
            .build()
            .map_err(HttpError::from)?;
        self.inner
            .send_request_and_decode(request, "Apex REST PATCH failed")
            .await
    }

    /// PUT to an Apex REST endpoint with a JSON body.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be deserialized.
    pub async fn put<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &(impl Serialize + Sync),
    ) -> Result<T> {
        let url = self.inner.resolve_apex_rest_url(path).await?;
        let request = self
            .inner
            .put(&url)
            .json(body)
            .build()
            .map_err(HttpError::from)?;
        self.inner
            .send_request_and_decode(request, "Apex REST PUT failed")
            .await
    }

    /// DELETE an Apex REST endpoint.
    ///
    /// Expects a 2xx response with no body (typically 204 No Content).
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or returns a non-success status.
    pub async fn delete(&self, path: &str) -> Result<()> {
        let url = self.inner.resolve_apex_rest_url(path).await?;
        let request = self.inner.delete(&url).build().map_err(HttpError::from)?;
        let response = self.inner.execute_request(request).await?;
        if !response.status().is_success() {
            return Err(
                crate::http::response_to_force_error(response, "Apex REST DELETE failed").await,
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{MockAuthenticator, Must};
    use serde::Deserialize;
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

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestResponse {
        message: String,
        count: i32,
    }

    // ── URL resolution tests ─────────────────────────────────────────

    #[tokio::test]
    async fn test_resolve_apex_rest_url_simple_path() {
        let (server, client) = setup().await;
        let handler = client.apex_rest();
        let url = handler
            .inner
            .resolve_apex_rest_url("MyService")
            .await
            .must();
        assert_eq!(url, format!("{}/services/apexrest/MyService", server.uri()));
    }

    #[tokio::test]
    async fn test_resolve_apex_rest_url_strips_leading_slash() {
        let (server, client) = setup().await;
        let handler = client.apex_rest();
        let url = handler
            .inner
            .resolve_apex_rest_url("/MyService")
            .await
            .must();
        assert_eq!(url, format!("{}/services/apexrest/MyService", server.uri()));
    }

    #[tokio::test]
    async fn test_resolve_apex_rest_url_empty_path() {
        let (server, client) = setup().await;
        let handler = client.apex_rest();
        let url = handler.inner.resolve_apex_rest_url("").await.must();
        assert_eq!(url, format!("{}/services/apexrest", server.uri()));
    }

    #[tokio::test]
    async fn test_resolve_apex_rest_url_nested_path() {
        let (server, client) = setup().await;
        let handler = client.apex_rest();
        let url = handler
            .inner
            .resolve_apex_rest_url("SBQQ/ServiceRouter")
            .await
            .must();
        assert_eq!(
            url,
            format!("{}/services/apexrest/SBQQ/ServiceRouter", server.uri())
        );
    }

    #[tokio::test]
    async fn test_resolve_apex_rest_url_has_no_api_version() {
        let (_server, client) = setup().await;
        let handler = client.apex_rest();
        let url = handler
            .inner
            .resolve_apex_rest_url("MyService")
            .await
            .must();
        // Apex REST URLs must NOT contain /services/data/vXX.0/
        assert!(!url.contains("/services/data/"));
    }

    // ── Handler construction tests ───────────────────────────────────

    #[tokio::test]
    async fn test_handler_is_cloneable() {
        let (_server, client) = setup().await;
        let handler = client.apex_rest();
        let cloned = handler.clone();
        // Verify both handlers share the same session via Arc
        let debug_original = format!("{handler:?}");
        let debug_cloned = format!("{cloned:?}");
        assert_eq!(debug_original, debug_cloned);
    }

    #[tokio::test]
    async fn test_handler_is_debug() {
        let (_server, client) = setup().await;
        let handler = client.apex_rest();
        let debug = format!("{handler:?}");
        assert!(debug.contains("ApexRestHandler"));
    }

    // ── GET tests ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_success() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path("/services/apexrest/MyService"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"message": "hello", "count": 42})),
            )
            .mount(&server)
            .await;

        let result: TestResponse = client.apex_rest().get("MyService").await.must();
        assert_eq!(result.message, "hello");
        assert_eq!(result.count, 42);
    }

    #[tokio::test]
    async fn test_get_with_params_success() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path("/services/apexrest/MyService/records"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"message": "filtered", "count": 5})),
            )
            .mount(&server)
            .await;

        let result: TestResponse = client
            .apex_rest()
            .get_with_params("MyService/records", &[("limit", "10")])
            .await
            .must();
        assert_eq!(result.message, "filtered");
        assert_eq!(result.count, 5);
    }

    // ── POST tests ───────────────────────────────────────────────────

    #[tokio::test]
    async fn test_post_success() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/apexrest/MyService/create"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"message": "created", "count": 1})),
            )
            .mount(&server)
            .await;

        let body = serde_json::json!({"name": "Test"});
        let result: TestResponse = client
            .apex_rest()
            .post("MyService/create", &body)
            .await
            .must();
        assert_eq!(result.message, "created");
        assert_eq!(result.count, 1);
    }

    #[tokio::test]
    async fn test_post_raw_success() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/apexrest/MyService/dynamic"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"arbitrary": "data", "nested": {"key": 1}})),
            )
            .mount(&server)
            .await;

        let body = serde_json::json!({"input": "test"});
        let result = client
            .apex_rest()
            .post_raw("MyService/dynamic", &body)
            .await
            .must();
        assert_eq!(result["arbitrary"], "data");
        assert_eq!(result["nested"]["key"], 1);
    }

    // ── PATCH tests ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_patch_success() {
        let (server, client) = setup().await;

        Mock::given(method("PATCH"))
            .and(path("/services/apexrest/MyService/update"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"message": "updated", "count": 1})),
            )
            .mount(&server)
            .await;

        let body = serde_json::json!({"name": "Updated"});
        let result: TestResponse = client
            .apex_rest()
            .patch("MyService/update", &body)
            .await
            .must();
        assert_eq!(result.message, "updated");
    }

    // ── PUT tests ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_put_success() {
        let (server, client) = setup().await;

        Mock::given(method("PUT"))
            .and(path("/services/apexrest/MyService/replace"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"message": "replaced", "count": 1})),
            )
            .mount(&server)
            .await;

        let body = serde_json::json!({"name": "Replaced"});
        let result: TestResponse = client
            .apex_rest()
            .put("MyService/replace", &body)
            .await
            .must();
        assert_eq!(result.message, "replaced");
    }

    // ── DELETE tests ─────────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_success() {
        let (server, client) = setup().await;

        Mock::given(method("DELETE"))
            .and(path("/services/apexrest/MyService/item/123"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        client.apex_rest().delete("MyService/item/123").await.must();
    }

    // ── Error handling tests ─────────────────────────────────────────

    #[tokio::test]
    async fn test_get_400_returns_error() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path("/services/apexrest/MyService/bad"))
            .respond_with(ResponseTemplate::new(400).set_body_json(
                serde_json::json!([{"message": "Bad request", "errorCode": "INVALID_INPUT"}]),
            ))
            .mount(&server)
            .await;

        let result: Result<TestResponse> = client.apex_rest().get("MyService/bad").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_post_500_returns_error() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/apexrest/MyService/fail"))
            .respond_with(
                ResponseTemplate::new(500)
                    .set_body_json(serde_json::json!({"message": "Internal error"})),
            )
            .mount(&server)
            .await;

        let body = serde_json::json!({"data": "test"});
        let result: Result<TestResponse> = client.apex_rest().post("MyService/fail", &body).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_404_returns_error() {
        let (server, client) = setup().await;

        Mock::given(method("DELETE"))
            .and(path("/services/apexrest/MyService/notfound"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let result = client.apex_rest().delete("MyService/notfound").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_namespaced_path() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path("/services/apexrest/MyNS/SubService/action"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"message": "namespaced", "count": 1})),
            )
            .mount(&server)
            .await;

        let result: TestResponse = client
            .apex_rest()
            .get("MyNS/SubService/action")
            .await
            .must();
        assert_eq!(result.message, "namespaced");
    }
}
