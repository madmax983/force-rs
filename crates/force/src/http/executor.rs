//! HTTP executor implementation.

use super::retry::{
    RequestRetryClass, RetryPolicy, classify_request, exponential_backoff, parse_retry_after,
};
use super::telemetry::{RequestErrorKind, TelemetryContext, TelemetryHooks};
use crate::auth::AccessToken;
use crate::error::{HttpError, Result};
use reqwest::{Request, Response, StatusCode};
use serde::de::DeserializeOwned;
use std::time::Duration;

const BASE_BACKOFF_MS: u64 = 500;

/// HTTP executor that handles middleware concerns.
///
/// The executor manages all HTTP communication with Salesforce, applying
/// middleware for authentication, retries, and error handling.
#[derive(Debug, Clone)]
pub struct HttpExecutor {
    /// The underlying HTTP client.
    client: reqwest::Client,
    /// Policy controlling retries based on request safety.
    retry_policy: RetryPolicy,
    /// Base timeout for requests.
    timeout: Duration,
    /// Base backoff duration (default 500ms).
    base_backoff: Duration,
    /// Optional telemetry hooks.
    telemetry_hooks: TelemetryHooks,
}

impl HttpExecutor {
    /// Creates a new HTTP executor with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            retry_policy: RetryPolicy::new(3, 0),
            timeout: Duration::from_secs(30),
            base_backoff: Duration::from_millis(BASE_BACKOFF_MS),
            telemetry_hooks: TelemetryHooks::new(),
        }
    }

    /// Creates a new HTTP executor with custom configuration.
    #[must_use]
    pub fn with_config(max_retries: u32, timeout: Duration) -> Self {
        Self {
            client: reqwest::Client::new(),
            retry_policy: RetryPolicy::new(max_retries, 0),
            timeout,
            base_backoff: Duration::from_millis(BASE_BACKOFF_MS),
            telemetry_hooks: TelemetryHooks::new(),
        }
    }

    /// Creates a new HTTP executor with a preconfigured reqwest client.
    #[must_use]
    pub fn with_client(client: reqwest::Client, max_retries: u32, timeout: Duration) -> Self {
        Self {
            client,
            retry_policy: RetryPolicy::new(max_retries, 0),
            timeout,
            base_backoff: Duration::from_millis(BASE_BACKOFF_MS),
            telemetry_hooks: TelemetryHooks::new(),
        }
    }

    /// Creates a new HTTP executor with an explicit retry policy.
    #[must_use]
    pub fn with_retry_policy(retry_policy: RetryPolicy, timeout: Duration) -> Self {
        Self {
            client: reqwest::Client::new(),
            retry_policy,
            timeout,
            base_backoff: Duration::from_millis(BASE_BACKOFF_MS),
            telemetry_hooks: TelemetryHooks::new(),
        }
    }

    /// Configures the base backoff duration for retries.
    #[must_use]
    pub fn with_base_backoff(mut self, base_backoff: Duration) -> Self {
        self.base_backoff = base_backoff;
        self
    }

    /// Attaches telemetry hooks to this executor.
    #[must_use]
    pub fn with_telemetry_hooks(mut self, telemetry_hooks: TelemetryHooks) -> Self {
        self.telemetry_hooks = telemetry_hooks;
        self
    }

    /// Executes a request with auth/retry behavior and returns the raw response.
    ///
    /// This method does not parse non-success status bodies into API errors.
    /// It is intended for higher-level handlers that need status-dependent behavior.
    pub async fn execute_response<F, Fut>(
        &self,
        request: Request,
        token: &AccessToken,
        refresh_token: F,
    ) -> Result<Response>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<AccessToken>>,
    {
        let request_class = classify_request(request.method());
        self.execute_response_with_retry_class(request, token, refresh_token, request_class)
            .await
    }

    /// Executes a request with an explicit retry class override.
    pub async fn execute_response_with_retry_class<F, Fut>(
        &self,
        request: Request,
        token: &AccessToken,
        refresh_token: F,
        request_class: RequestRetryClass,
    ) -> Result<Response>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<AccessToken>>,
    {
        self.execute_response_with_class(request, token, refresh_token, request_class)
            .await
    }

    fn inject_auth_header(request: &mut Request, token: &AccessToken) -> Result<()> {
        let header_value = token.auth_header()?;
        request
            .headers_mut()
            .insert(reqwest::header::AUTHORIZATION, header_value.clone());
        Ok(())
    }

    async fn execute_attempt(
        &self,
        request: Request,
        retry_attempt: u32,
        ctx: &TelemetryContext,
    ) -> Result<Response> {
        match tokio::time::timeout(self.timeout, self.client.execute(request)).await {
            Err(_) => {
                self.record_completion(ctx, None, Some(RequestErrorKind::Timeout), retry_attempt);
                Err(HttpError::Timeout {
                    timeout_seconds: self.timeout.as_secs(),
                }
                .into())
            }
            Ok(Err(error)) => {
                self.record_completion(ctx, None, Some(RequestErrorKind::Transport), retry_attempt);
                Err(HttpError::from(error).into())
            }
            Ok(Ok(response)) => Ok(response),
        }
    }

    fn is_retryable_error(error: &crate::error::ForceError) -> bool {
        match error {
            // HttpError wraps reqwest::Error via RequestFailed
            crate::error::ForceError::Http(http_err) => match http_err {
                HttpError::Timeout { .. } => true,
                HttpError::RequestFailed(re) => {
                    !re.is_builder() && !re.is_redirect() && !re.is_status()
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn handle_rate_limit(
        &self,
        response: &Response,
        retry_attempt: u32,
        ctx: &TelemetryContext,
    ) -> crate::error::ForceError {
        let retry_after = parse_retry_after(response.headers()).unwrap_or(60);
        self.record_completion(
            ctx,
            Some(StatusCode::TOO_MANY_REQUESTS.as_u16()),
            Some(RequestErrorKind::RateLimited),
            retry_attempt,
        );
        HttpError::RateLimitExceeded {
            retry_after_seconds: retry_after,
        }
        .into()
    }

    async fn handle_transient_failure(
        &self,
        retry_attempt: u32,
        ctx: &TelemetryContext,
        status_code: Option<u16>,
    ) {
        let backoff = exponential_backoff(retry_attempt, self.base_backoff);
        tracing::warn!(
            retry.attempt = retry_attempt,
            http.status_code = status_code.unwrap_or(0),
            retry.backoff_ms = backoff.as_millis(),
            "retrying request after transient failure"
        );
        self.record_retry(
            ctx,
            retry_attempt,
            status_code.unwrap_or(0),
            backoff.as_millis(),
        );
        tokio::time::sleep(backoff).await;
    }

    async fn execute_response_with_class<F, Fut>(
        &self,
        mut request: Request,
        token: &AccessToken,
        refresh_token: F,
        request_class: RequestRetryClass,
    ) -> Result<Response>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<AccessToken>>,
    {
        // Capture telemetry context once
        let ctx = TelemetryContext::new(
            request.method().as_str(),
            request.url().path(),
            request_class,
            self.telemetry_hooks.has_hooks(),
        );

        let request_span = tracing::info_span!(
            "force_http_request",
            http.method = request.method().as_str(),
            http.path = request.url().path(),
            request.class = ctx.request_class
        );
        let _request_span_guard = request_span.enter();

        // Inject Bearer token
        Self::inject_auth_header(&mut request, token)?;

        // Execute with retry logic
        let mut retry_attempt = 0;
        let mut refreshed = false;
        let max_retries = self.max_retries_for(request_class);
        loop {
            let req_clone = request.try_clone().ok_or_else(|| {
                HttpError::InvalidUrl("cannot clone request for retry".to_string())
            })?;

            let response = match self.execute_attempt(req_clone, retry_attempt, &ctx).await {
                Ok(resp) => resp,
                Err(e) if retry_attempt < max_retries => {
                    // Check if error is retryable (timeout or transport)
                    if Self::is_retryable_error(&e) {
                        self.handle_transient_failure(retry_attempt, &ctx, None)
                            .await;
                        retry_attempt += 1;
                        continue;
                    }
                    return Err(e);
                }
                Err(e) => return Err(e),
            };

            match response.status() {
                StatusCode::UNAUTHORIZED => {
                    // 401: Refresh token and retry once
                    if !refreshed {
                        let new_token = refresh_token().await?;
                        Self::inject_auth_header(&mut request, &new_token)?;
                        refreshed = true;
                        continue;
                    }

                    self.record_completion(
                        &ctx,
                        Some(StatusCode::UNAUTHORIZED.as_u16()),
                        None,
                        retry_attempt,
                    );
                    return Ok(response);
                }
                StatusCode::TOO_MANY_REQUESTS => {
                    // 429: Rate limit - respect Retry-After header
                    return Err(self.handle_rate_limit(&response, retry_attempt, &ctx));
                }
                StatusCode::SERVICE_UNAVAILABLE if retry_attempt < max_retries => {
                    // 503: Retry with exponential backoff
                    self.handle_transient_failure(retry_attempt, &ctx, Some(503))
                        .await;
                    retry_attempt += 1;
                    continue;
                }
                _ => {
                    self.record_completion(
                        &ctx,
                        Some(response.status().as_u16()),
                        None,
                        retry_attempt,
                    );
                    return Ok(response);
                }
            }
        }
    }

    fn max_retries_for(&self, request_class: RequestRetryClass) -> u32 {
        match request_class {
            RequestRetryClass::Read => self.retry_policy.read_max_retries,
            RequestRetryClass::IdempotentMutation => {
                self.retry_policy.idempotent_mutation_max_retries
            }
            RequestRetryClass::Mutation => self.retry_policy.mutation_max_retries,
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    fn record_retry(
        &self,
        ctx: &TelemetryContext,
        attempt: u32,
        status_code: u16,
        backoff_ms: u128,
    ) {
        if let Some(on_retry) = &self.telemetry_hooks.on_retry {
            on_retry(&ctx.create_retry_event(attempt, status_code, backoff_ms));
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    fn record_completion(
        &self,
        ctx: &TelemetryContext,
        status_code: Option<u16>,
        error_kind: Option<RequestErrorKind>,
        retries: u32,
    ) {
        tracing::info!(
            http.status_code = status_code.unwrap_or_default(),
            retries = retries,
            elapsed_ms = ctx.start_time.elapsed().as_millis(),
            error.kind = ?error_kind,
            "request completed"
        );
        if let Some(on_complete) = &self.telemetry_hooks.on_complete {
            on_complete(&ctx.create_completion(status_code, error_kind, retries));
        }
    }

    /// Executes an HTTP request with full middleware stack.
    ///
    /// This method applies all middleware layers:
    /// 1. Injects authentication token
    /// 2. Sends request with timeout
    /// 3. Handles 401 by triggering refresh callback and retrying
    /// 4. Handles 429 by respecting Retry-After header
    /// 5. Handles 503 with exponential backoff
    /// 6. Parses API errors from response
    ///
    /// # Arguments
    ///
    /// * `request` - The HTTP request to execute
    /// * `token` - The access token for authentication
    /// * `refresh_token` - Async callback to refresh the token on 401
    ///
    /// # Returns
    ///
    /// The HTTP response on success, or a detailed error.
    pub async fn execute<F, Fut>(
        &self,
        request: Request,
        token: &AccessToken,
        refresh_token: F,
    ) -> Result<Response>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<AccessToken>>,
    {
        let response = self.execute_response(request, token, refresh_token).await?;
        let status = response.status();

        if status.is_success() {
            Ok(response)
        } else if status == StatusCode::UNAUTHORIZED {
            Err(HttpError::StatusError {
                status_code: 401,
                message: "Unauthorized after token refresh".to_string(),
            }
            .into())
        } else {
            // Parse API error from response body
            Err(crate::http::error::response_to_force_error(response, "Unknown error").await)
        }
    }

    /// Executes a request and deserializes the JSON response.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The type to deserialize the response into
    pub async fn execute_json<T, F, Fut>(
        &self,
        request: Request,
        token: &AccessToken,
        refresh_token: F,
    ) -> Result<T>
    where
        T: DeserializeOwned,
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<AccessToken>>,
    {
        let response = self.execute(request, token, refresh_token).await?;
        let json = response.json::<T>().await.map_err(HttpError::from)?;
        Ok(json)
    }
}

impl Default for HttpExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AccessToken;
    use crate::error::HttpError;
    use crate::test_support::Must;
    use reqwest::Method;
    use std::time::Duration;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn create_test_token() -> AccessToken {
        AccessToken::new(
            "test_token".to_string(),
            "https://test.salesforce.com".to_string(),
            None,
        )
    }

    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_execute_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "totalSize": 1,
                "done": true,
                "records": []
            })))
            .mount(&mock_server)
            .await;

        let executor = HttpExecutor::new();
        let token = create_test_token();

        // Dummy refresh token closure that panics if called
        let refresh_token = || async {
            panic!("Should not be called");
            #[allow(unreachable_code)]
            Ok(create_test_token())
        };

        let request = executor
            .client
            .request(
                Method::GET,
                format!("{}/services/data/v60.0/query", mock_server.uri()),
            )
            .build()
            .must();

        let response = executor
            .execute_response(request, &token, refresh_token)
            .await
            .must();

        assert_eq!(response.status(), reqwest::StatusCode::OK);
    }

    #[tokio::test]
    async fn test_execute_401_retry() {
        let mock_server = MockServer::start().await;

        // First attempt fails with 401
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .and(header("Authorization", "Bearer expired_token"))
            .respond_with(
                ResponseTemplate::new(401).set_body_json(serde_json::json!([{
                    "message": "Session expired or invalid",
                    "errorCode": "INVALID_SESSION_ID"
                }])),
            )
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;

        // Second attempt succeeds with new token
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "totalSize": 1,
                "done": true,
                "records": []
            })))
            .mount(&mock_server)
            .await;

        let executor = HttpExecutor::new();

        let initial_token = AccessToken::new(
            "expired_token".to_string(),
            "https://test.salesforce.com".to_string(),
            None,
        );

        let refresh_calls = Arc::new(AtomicUsize::new(0));
        let calls_clone = Arc::clone(&refresh_calls);

        // Refresh token closure returns a new token and increments counter
        let refresh_token = || {
            let calls = Arc::clone(&calls_clone);
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(create_test_token())
            }
        };

        let request = executor
            .client
            .request(
                Method::GET,
                format!("{}/services/data/v60.0/query", mock_server.uri()),
            )
            .build()
            .must();

        let response = executor
            .execute_response(request, &initial_token, refresh_token)
            .await
            .must();

        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert_eq!(refresh_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_execute_429_rate_limit() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .respond_with(
                ResponseTemplate::new(429)
                    .insert_header("Retry-After", "60")
                    .set_body_json(serde_json::json!([{
                        "message": "Too Many Requests",
                        "errorCode": "REQUEST_LIMIT_EXCEEDED"
                    }])),
            )
            .mount(&mock_server)
            .await;

        let executor = HttpExecutor::new();
        let token = create_test_token();

        // Dummy refresh token closure that panics if called
        let refresh_token = || async {
            panic!("Should not be called");
            #[allow(unreachable_code)]
            Ok(create_test_token())
        };

        let request = executor
            .client
            .request(
                Method::GET,
                format!("{}/services/data/v60.0/query", mock_server.uri()),
            )
            .build()
            .must();

        let result = executor
            .execute_response(request, &token, refresh_token)
            .await;

        match result {
            Err(crate::error::ForceError::Http(HttpError::RateLimitExceeded {
                retry_after_seconds,
            })) => {
                assert_eq!(retry_after_seconds, 60);
            }
            _ => panic!("Expected RateLimitExceeded error, got: {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_execute_503_retry() {
        let mock_server = MockServer::start().await;

        // Mock a 503 Service Unavailable response
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .respond_with(
                ResponseTemplate::new(503).set_body_json(serde_json::json!([{
                    "message": "Service Unavailable",
                    "errorCode": "SERVICE_UNAVAILABLE"
                }])),
            )
            // It should be called 1 time originally + 2 retries = 3 times total
            .up_to_n_times(3)
            .expect(3)
            .mount(&mock_server)
            .await;

        // Configure executor to retry fast for testing
        let executor = HttpExecutor::with_config(2, Duration::from_secs(30))
            .with_base_backoff(Duration::from_millis(1));

        let token = create_test_token();

        // Dummy refresh token closure
        let refresh_token = || async {
            panic!("Should not be called");
            #[allow(unreachable_code)]
            Ok(create_test_token())
        };

        let request = executor
            .client
            .request(
                Method::GET,
                format!("{}/services/data/v60.0/query", mock_server.uri()),
            )
            .build()
            .must();

        let response = executor
            .execute_response(request, &token, refresh_token)
            .await
            .must();

        // Expect the response to be successfully returned and it to be 503
        assert_eq!(response.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn test_execute_timeout() {
        let mock_server = MockServer::start().await;

        // Mock a delayed response that exceeds our timeout
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_millis(100))
                    .set_body_json(serde_json::json!({
                        "totalSize": 1,
                        "done": true,
                        "records": []
                    })),
            )
            .mount(&mock_server)
            .await;

        // Configure executor with 0 retries and a 10ms timeout
        let executor = HttpExecutor::with_config(0, Duration::from_millis(10));
        let token = create_test_token();

        let refresh_token = || async {
            panic!("Should not be called");
            #[allow(unreachable_code)]
            Ok(create_test_token())
        };

        let request = executor
            .client
            .request(
                Method::GET,
                format!("{}/services/data/v60.0/query", mock_server.uri()),
            )
            .build()
            .must();

        let result = executor
            .execute_response(request, &token, refresh_token)
            .await;

        match result {
            Err(crate::error::ForceError::Http(HttpError::Timeout { timeout_seconds })) => {
                // Since 10ms converts to 0s in standard Duration::as_secs()
                assert_eq!(timeout_seconds, 0);
            }
            _ => panic!("Expected Timeout error, got: {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_execute_transport_error() {
        // Use an unroutable local address to force a transport/connection error
        let unroutable_url = "http://127.0.0.1:0/services/data/v60.0/query";

        // We only want 0 retries here so we can assert the final error directly
        let executor = HttpExecutor::with_config(0, Duration::from_millis(100));
        let token = create_test_token();

        let refresh_token = || async {
            panic!("Should not be called");
            #[allow(unreachable_code)]
            Ok(create_test_token())
        };

        let request = executor
            .client
            .request(Method::GET, unroutable_url)
            .build()
            .must();

        let result = executor
            .execute_response(request, &token, refresh_token)
            .await;

        match result {
            Err(crate::error::ForceError::Http(HttpError::RequestFailed(e))) => {
                assert!(
                    e.is_connect() || e.is_builder() || e.is_request(),
                    "Expected connection/transport error, got: {:?}",
                    e
                );
            }
            _ => panic!("Expected Transport error, got: {:?}", result),
        }
    }
}
