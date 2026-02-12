use crate::auth::TokenManager;
use crate::config::ClientConfig;
use crate::http::HttpExecutor;
use crate::http::RequestRetryClass;
use std::sync::Arc;

/// Inner state shared across cloned clients.
///
/// This is generic over the authenticator type to avoid trait object overhead.
#[derive(Debug, Clone)]
pub(crate) struct Inner<A: crate::auth::Authenticator> {
    /// Client configuration.
    pub(crate) config: ClientConfig,
    /// HTTP client for making requests.
    pub(crate) http_client: reqwest::Client,
    /// Shared HTTP executor for auth/retry/timeout middleware.
    pub(crate) http_executor: HttpExecutor,
    /// Token manager for automatic token refresh (wrapped in Arc for cloning).
    pub(crate) token_manager: Arc<TokenManager<A>>,
}

impl<A: crate::auth::Authenticator> Inner<A> {
    /// Executes a request through the shared middleware pipeline.
    pub(crate) async fn execute_request(
        &self,
        request: reqwest::Request,
    ) -> crate::error::Result<reqwest::Response> {
        let token = self.token_manager.token().await?;
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
        let token = self.token_manager.token().await?;
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
}
