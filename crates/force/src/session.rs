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
}
