//! Salesforce API client.
//!
//! This module provides the core `ForceClient` and builder.

mod builder;

pub use builder::ForceClientBuilder;

use crate::auth::TokenManager;
use crate::config::ClientConfig;
use crate::http::{HttpExecutor, RequestRetryClass};
use serde::de::DeserializeOwned;
use std::sync::Arc;

/// Salesforce API client.
///
/// Clients are cheaply cloneable via `Arc` and shared internal state.
///
/// The client is generic over the authenticator type for zero-cost abstraction.
#[derive(Debug)]
pub struct ForceClient<A: crate::auth::Authenticator> {
    /// Client configuration.
    pub(crate) config: ClientConfig,
    /// HTTP client for making requests.
    pub(crate) http_client: reqwest::Client,
    /// Shared HTTP executor for auth/retry/timeout middleware.
    pub(crate) http_executor: HttpExecutor,
    /// Token manager for automatic token refresh (wrapped in Arc for cloning).
    pub(crate) token_manager: Arc<TokenManager<A>>,
}

/// Public builder constructor.
#[must_use]
pub fn builder() -> ForceClientBuilder {
    ForceClientBuilder::new()
}

impl<A: crate::auth::Authenticator> Clone for ForceClient<A> {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            http_client: self.http_client.clone(),
            http_executor: self.http_executor.clone(),
            token_manager: self.token_manager.clone(),
        }
    }
}

impl<A: crate::auth::Authenticator> ForceClient<A> {
    /// Returns the client configuration.
    #[must_use]
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    /// Returns the current access token, refreshing if necessary.
    ///
    /// # Errors
    ///
    /// Returns an error if authentication or token refresh fails.
    pub async fn token(&self) -> crate::error::Result<crate::auth::AccessToken> {
        self.token_manager.token().await
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

    /// Creates a REST API handler for this client.
    ///
    /// The REST handler provides access to CRUD operations, queries, and metadata.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let client = builder().build(auth).await?;
    /// let rest = client.rest();
    /// ```
    #[cfg(feature = "rest")]
    #[must_use]
    pub fn rest(&self) -> crate::api::rest::RestHandler<A> {
        crate::api::rest::RestHandler::new(self.clone())
    }

    /// Creates a Bulk API 2.0 handler for this client.
    ///
    /// The Bulk handler provides access to high-volume data operations and bulk queries.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let client = builder().build(auth).await?;
    /// let bulk = client.bulk();
    /// ```
    #[cfg(feature = "bulk")]
    #[must_use]
    pub fn bulk(&self) -> crate::api::bulk::BulkHandler<A> {
        crate::api::bulk::BulkHandler::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_creation() {
        let _builder = builder();
    }
}
