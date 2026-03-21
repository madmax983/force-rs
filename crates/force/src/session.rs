//! Shared session state for Salesforce API clients.
//!
//! This module contains the `Session` struct which holds the core state:
//! - Client configuration
//! - HTTP client
//! - Authentication token manager
//!
//! It acts as the "Context" for all API operations, ensuring they share
//! the same authentication and configuration.

use crate::auth::token_manager::TokenManager;
use crate::config::ClientConfig;
use crate::http::HttpExecutor;
use crate::http::RequestRetryClass;
use serde::de::DeserializeOwned;
use std::sync::Arc;

/// Shared session state across cloned clients.
///
/// This is generic over the authenticator type to avoid trait object overhead.
#[derive(Debug, Clone)]
pub struct Session<A: crate::auth::authenticator::Authenticator> {
    /// Client configuration.
    pub(crate) config: ClientConfig,
    /// HTTP client for making requests.
    pub(crate) http_client: reqwest::Client,
    /// Shared HTTP executor for auth/retry/timeout middleware.
    pub(crate) http_executor: HttpExecutor,
    /// Token manager for automatic token refresh (wrapped in Arc for cloning).
    pub(crate) token_manager: Arc<TokenManager<A>>,
}

impl<A: crate::auth::authenticator::Authenticator> Session<A> {
    /// Returns the token manager for this session.
    ///
    /// Extension crates (e.g. `force-pubsub`) use this to obtain fresh access
    /// tokens without duplicating authentication logic.
    #[must_use]
    pub fn token_manager(&self) -> &Arc<TokenManager<A>> {
        &self.token_manager
    }

    /// Executes a request through the shared middleware pipeline.
    pub(crate) async fn execute_request(
        &self,
        request: reqwest::Request,
    ) -> crate::error::Result<reqwest::Response> {
        let token = self.token_manager.get_token_arc().await?;
        let token_manager = Arc::clone(&self.token_manager);
        self.http_executor
            .execute_response(request, &token, move || {
                let token_manager = Arc::clone(&token_manager);
                async move { token_manager.force_refresh().await }
            })
            .await
    }
    /// Returns the instance URL from the current authentication token.
    pub(crate) async fn instance_url(&self) -> crate::error::Result<String> {
        let token = self.token_manager.get_token_arc().await?;
        Ok(token.instance_url().to_string())
    }

    /// Executes a request with an explicit retry class override.
    pub(crate) async fn execute_request_with_retry_class(
        &self,
        request: reqwest::Request,
        retry_class: RequestRetryClass,
    ) -> crate::error::Result<reqwest::Response> {
        let token = self.token_manager.get_token_arc().await?;
        let token_manager = Arc::clone(&self.token_manager);
        self.http_executor
            .execute_response_with_retry_class(
                request,
                &token,
                move || {
                    let token_manager = Arc::clone(&token_manager);
                    async move { token_manager.force_refresh().await }
                },
                retry_class,
            )
            .await
    }

    /// Executes a request, checks for success, and deserializes the JSON response.
    ///
    /// This helper standardizes the pattern of:
    /// 1. Executing the request via `execute_request`
    /// 2. Checking `response.status().is_success()`
    /// 3. Converting non-success responses to `ForceError`
    /// 4. Deserializing success responses to `T`
    pub(crate) async fn send_request_and_decode<T: DeserializeOwned>(
        &self,
        request: reqwest::Request,
        fallback_error_message: &str,
    ) -> crate::error::Result<T> {
        let response = self.execute_request(request).await?;

        if !response.status().is_success() {
            return Err(
                crate::http::response_to_force_error(response, fallback_error_message).await,
            );
        }

        response
            .json::<T>()
            .await
            .map_err(crate::error::HttpError::from)
            .map_err(Into::into)
    }

    /// Resolves a path to a full Apex REST URL.
    ///
    /// Constructs: `{instance_url}/services/apexrest/{path}`
    ///
    /// Unlike [`resolve_url`](Self::resolve_url), this does **not** include the
    /// API version in the path, because Apex REST endpoints are version-less.
    #[cfg(feature = "apex_rest")]
    pub(crate) async fn resolve_apex_rest_url(&self, path: &str) -> crate::error::Result<String> {
        let token = self.token_manager.get_token_arc().await?;
        let clean_path = path.trim_start_matches('/');
        if clean_path.is_empty() {
            Ok(format!("{}/services/apexrest", token.instance_url()))
        } else {
            Ok(format!(
                "{}/services/apexrest/{}",
                token.instance_url(),
                clean_path
            ))
        }
    }

    /// Resolves a path to a full Salesforce API URL.
    ///
    /// Constructs: `{instance_url}/services/data/{api_version}/{path}`
    pub(crate) async fn resolve_url(&self, path: &str) -> crate::error::Result<String> {
        let token = self.token_manager.get_token_arc().await?;
        // Handle empty path or path starting with slash
        let clean_path = path.trim_start_matches('/');
        if clean_path.is_empty() {
            Ok(format!(
                "{}/services/data/{}",
                token.instance_url(),
                self.config.api_version
            ))
        } else {
            Ok(format!(
                "{}/services/data/{}/{}",
                token.instance_url(),
                self.config.api_version,
                clean_path
            ))
        }
    }

    /// Creates a GET request builder for the given URL.
    pub(crate) fn get(&self, url: &str) -> reqwest::RequestBuilder {
        self.http_client.get(url)
    }

    /// Creates a POST request builder for the given URL.
    pub(crate) fn post(&self, url: &str) -> reqwest::RequestBuilder {
        self.http_client.post(url)
    }

    /// Creates a PATCH request builder for the given URL.
    pub(crate) fn patch(&self, url: &str) -> reqwest::RequestBuilder {
        self.http_client.patch(url)
    }

    /// Creates a DELETE request builder for the given URL.
    pub(crate) fn delete(&self, url: &str) -> reqwest::RequestBuilder {
        self.http_client.delete(url)
    }

    /// Creates a PUT request builder for the given URL.
    pub(crate) fn put(&self, url: &str) -> reqwest::RequestBuilder {
        self.http_client.put(url)
    }

    /// Creates a request builder for the given HTTP method and URL.
    pub(crate) fn request(&self, method: reqwest::Method, url: &str) -> reqwest::RequestBuilder {
        self.http_client.request(method, url)
    }
}
