//! HTTP executor implementation.

use super::error::parse_api_error;
use super::retry::{
    RequestRetryClass, RetryPolicy, classify_request, exponential_backoff, parse_retry_after,
};
use crate::auth::AccessToken;
use crate::error::{HttpError, Result};
use reqwest::{Request, Response, StatusCode};
use serde::de::DeserializeOwned;
use std::time::{Duration, Instant};

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
        }
    }

    /// Configures the base backoff duration for retries.
    #[must_use]
    pub fn with_base_backoff(mut self, base_backoff: Duration) -> Self {
        self.base_backoff = base_backoff;
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
        start_time: Instant,
    ) -> Result<Response> {
        match tokio::time::timeout(self.timeout, self.client.execute(request)).await {
            Err(_) => {
                Self::log_completion(start_time, None, Some("Timeout"), retry_attempt);
                Err(HttpError::Timeout {
                    timeout_seconds: self.timeout.as_secs(),
                }
                .into())
            }
            Ok(Err(error)) => {
                Self::log_completion(start_time, None, Some("Transport"), retry_attempt);
                Err(HttpError::from(error).into())
            }
            Ok(Ok(response)) => Ok(response),
        }
    }

    fn handle_rate_limit(
        response: &Response,
        retry_attempt: u32,
        start_time: Instant,
    ) -> crate::error::ForceError {
        let retry_after = parse_retry_after(response.headers()).unwrap_or(60);
        Self::log_completion(
            start_time,
            Some(StatusCode::TOO_MANY_REQUESTS.as_u16()),
            Some("RateLimited"),
            retry_attempt,
        );
        HttpError::RateLimitExceeded {
            retry_after_seconds: retry_after,
        }
        .into()
    }

    async fn handle_service_unavailable(&self, retry_attempt: u32) {
        let backoff = exponential_backoff(retry_attempt, self.base_backoff);
        tracing::warn!(
            retry.attempt = retry_attempt,
            http.status_code = 503_u16,
            retry.backoff_ms = backoff.as_millis(),
            "retrying request after transient failure"
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
        let start_time = Instant::now();

        let request_span = tracing::info_span!(
            "force_http_request",
            http.method = request.method().as_str(),
            http.path = request.url().path(),
            request.class = request_class.as_str()
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

            let response = self.execute_attempt(req_clone, retry_attempt, start_time).await?;

            match response.status() {
                StatusCode::UNAUTHORIZED => {
                    // 401: Refresh token and retry once
                    if !refreshed {
                        let new_token = refresh_token().await?;
                        Self::inject_auth_header(&mut request, &new_token)?;
                        refreshed = true;
                        continue;
                    }

                    Self::log_completion(
                        start_time,
                        Some(StatusCode::UNAUTHORIZED.as_u16()),
                        None,
                        retry_attempt,
                    );
                    return Ok(response);
                }
                StatusCode::TOO_MANY_REQUESTS => {
                    // 429: Rate limit - respect Retry-After header
                    return Err(Self::handle_rate_limit(&response, retry_attempt, start_time));
                }
                StatusCode::SERVICE_UNAVAILABLE if retry_attempt < max_retries => {
                    // 503: Retry with exponential backoff
                    self.handle_service_unavailable(retry_attempt).await;
                    retry_attempt += 1;
                    continue;
                }
                _ => {
                    Self::log_completion(
                        start_time,
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

    fn log_completion(
        start_time: Instant,
        status_code: Option<u16>,
        error_kind: Option<&str>,
        retries: u32,
    ) {
        tracing::info!(
            http.status_code = status_code.unwrap_or_default(),
            retries = retries,
            elapsed_ms = start_time.elapsed().as_millis(),
            error.kind = ?error_kind,
            "request completed"
        );
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
            let error_text = response.text().await.map_err(HttpError::from)?;
            Err(parse_api_error(status.as_u16(), &error_text).into())
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
