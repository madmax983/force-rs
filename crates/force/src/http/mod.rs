//! HTTP client layer with middleware for retry, rate limiting, and auth refresh.
//!
//! This module provides the HTTP execution layer that handles:
//! - Automatic token injection and Bearer authentication
//! - 401 response detection and automatic token refresh + retry
//! - 429 rate limit handling with Retry-After header respect
//! - Exponential backoff for retryable errors (503, network failures)
//! - Sforce-Limit-Info header tracking for API limits
//! - Detailed error parsing from Salesforce API responses

use crate::auth::AccessToken;
use crate::error::{HttpError, Result};
use reqwest::{Method, Request, Response, StatusCode};
use serde::de::DeserializeOwned;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

/// Retry behavior per request safety class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Maximum retries for read-style operations (e.g. GET query calls).
    pub read_max_retries: u32,
    /// Maximum retries for explicitly idempotent mutation operations.
    pub idempotent_mutation_max_retries: u32,
    /// Maximum retries for mutation operations (e.g. POST/PATCH/DELETE).
    pub mutation_max_retries: u32,
}

impl RetryPolicy {
    /// Creates a retry policy with explicit read/mutation retry limits.
    #[must_use]
    pub const fn new(read_max_retries: u32, mutation_max_retries: u32) -> Self {
        Self {
            read_max_retries,
            idempotent_mutation_max_retries: read_max_retries,
            mutation_max_retries,
        }
    }

    /// Overrides retries for explicitly idempotent mutations.
    #[must_use]
    pub const fn with_idempotent_mutation_retries(mut self, retries: u32) -> Self {
        self.idempotent_mutation_max_retries = retries;
        self
    }
}

/// Retry safety class for request execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestRetryClass {
    /// Read-only request.
    Read,
    /// Mutation explicitly treated as idempotent.
    IdempotentMutation,
    /// Potentially non-idempotent mutation.
    Mutation,
}

impl RequestRetryClass {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::IdempotentMutation => "idempotent_mutation",
            Self::Mutation => "mutation",
        }
    }
}

/// Error kind recorded by request completion telemetry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestErrorKind {
    /// Request timed out.
    Timeout,
    /// Transport-level request error.
    Transport,
    /// Request was rate limited.
    RateLimited,
}

/// Redaction-safe retry telemetry event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryEvent {
    /// HTTP method.
    pub method: String,
    /// URL path only (query excluded).
    pub path: String,
    /// Request safety class.
    pub request_class: &'static str,
    /// Retry attempt number (0-based).
    pub attempt: u32,
    /// Status code that triggered retry.
    pub status_code: u16,
    /// Backoff delay in milliseconds.
    pub backoff_ms: u128,
}

/// Redaction-safe request completion telemetry event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestCompletion {
    /// HTTP method.
    pub method: String,
    /// URL path only (query excluded).
    pub path: String,
    /// Request safety class.
    pub request_class: &'static str,
    /// Final status code if response was received.
    pub status_code: Option<u16>,
    /// Error kind when response is unavailable or handled as error.
    pub error_kind: Option<RequestErrorKind>,
    /// Number of retry attempts performed.
    pub retries: u32,
    /// Total elapsed milliseconds.
    pub elapsed_ms: u128,
}

/// Optional telemetry hooks for retry and completion events.
type RetryHook = Arc<dyn Fn(&RetryEvent) + Send + Sync>;
type CompletionHook = Arc<dyn Fn(&RequestCompletion) + Send + Sync>;

/// Optional telemetry hooks for retry and completion events.
#[derive(Clone, Default)]
pub struct TelemetryHooks {
    on_retry: Option<RetryHook>,
    on_complete: Option<CompletionHook>,
}

impl std::fmt::Debug for TelemetryHooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TelemetryHooks")
            .field("has_on_retry", &self.on_retry.is_some())
            .field("has_on_complete", &self.on_complete.is_some())
            .finish()
    }
}

impl TelemetryHooks {
    /// Creates empty telemetry hooks.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            on_retry: None,
            on_complete: None,
        }
    }

    /// Registers a retry callback.
    #[must_use]
    pub fn on_retry<F>(mut self, hook: F) -> Self
    where
        F: Fn(&RetryEvent) + Send + Sync + 'static,
    {
        self.on_retry = Some(Arc::new(hook));
        self
    }

    /// Registers a completion callback.
    #[must_use]
    pub fn on_complete<F>(mut self, hook: F) -> Self
    where
        F: Fn(&RequestCompletion) + Send + Sync + 'static,
    {
        self.on_complete = Some(Arc::new(hook));
        self
    }
}

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
            telemetry_hooks: TelemetryHooks::new(),
        }
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

    #[allow(clippy::too_many_lines)]
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
        let method = request.method().to_string();
        let path = request.url().path().to_string();
        let request_class_str = request_class.as_str();
        let request_span = tracing::info_span!(
            "force_http_request",
            http.method = %method,
            http.path = %path,
            request.class = request_class_str
        );
        let _request_span_guard = request_span.enter();
        let start = Instant::now();

        // Inject Bearer token
        let auth_header = format!("Bearer {}", token.as_str());
        let header_value = auth_header.parse().map_err(|_| {
            HttpError::InvalidUrl(format!("invalid authorization header: {}", auth_header))
        })?;
        request.headers_mut().insert("Authorization", header_value);

        // Execute with retry logic
        let mut retry_attempt = 0;
        let mut refreshed = false;
        let max_retries = self.max_retries_for(request_class);
        loop {
            let req_clone = request.try_clone().ok_or_else(|| {
                HttpError::InvalidUrl("cannot clone request for retry".to_string())
            })?;

            let response = match tokio::time::timeout(self.timeout, self.client.execute(req_clone)).await {
                Err(_) => {
                    self.record_completion(RequestCompletion {
                        method: method.clone(),
                        path: path.clone(),
                        request_class: request_class_str,
                        status_code: None,
                        error_kind: Some(RequestErrorKind::Timeout),
                        retries: retry_attempt,
                        elapsed_ms: start.elapsed().as_millis(),
                    });
                    return Err(HttpError::Timeout {
                        timeout_seconds: self.timeout.as_secs(),
                    }
                    .into());
                }
                Ok(Err(error)) => {
                    self.record_completion(RequestCompletion {
                        method: method.clone(),
                        path: path.clone(),
                        request_class: request_class_str,
                        status_code: None,
                        error_kind: Some(RequestErrorKind::Transport),
                        retries: retry_attempt,
                        elapsed_ms: start.elapsed().as_millis(),
                    });
                    return Err(HttpError::from(error).into());
                }
                Ok(Ok(response)) => response,
            };

            match response.status() {
                StatusCode::UNAUTHORIZED => {
                    // 401: Refresh token and retry once
                    if !refreshed {
                        let new_token = refresh_token().await?;
                        let new_auth_header = format!("Bearer {}", new_token.as_str());
                        let new_header_value = new_auth_header.parse().map_err(|_| {
                            HttpError::InvalidUrl(format!(
                                "invalid authorization header: {}",
                                new_auth_header
                            ))
                        })?;
                        request
                            .headers_mut()
                            .insert("Authorization", new_header_value);
                        refreshed = true;
                        continue;
                    }
                    self.record_completion(RequestCompletion {
                        method: method.clone(),
                        path: path.clone(),
                        request_class: request_class_str,
                        status_code: Some(StatusCode::UNAUTHORIZED.as_u16()),
                        error_kind: None,
                        retries: retry_attempt,
                        elapsed_ms: start.elapsed().as_millis(),
                    });
                    return Ok(response);
                }
                StatusCode::TOO_MANY_REQUESTS => {
                    // 429: Rate limit - respect Retry-After header
                    let retry_after = parse_retry_after(&response).unwrap_or(60);
                    self.record_completion(RequestCompletion {
                        method: method.clone(),
                        path: path.clone(),
                        request_class: request_class_str,
                        status_code: Some(StatusCode::TOO_MANY_REQUESTS.as_u16()),
                        error_kind: Some(RequestErrorKind::RateLimited),
                        retries: retry_attempt,
                        elapsed_ms: start.elapsed().as_millis(),
                    });
                    return Err(HttpError::RateLimitExceeded {
                        retry_after_seconds: retry_after,
                    }
                    .into());
                }
                StatusCode::SERVICE_UNAVAILABLE if retry_attempt < max_retries => {
                    // 503: Retry with exponential backoff
                    let backoff = exponential_backoff(retry_attempt);
                    tracing::warn!(
                        retry.attempt = retry_attempt,
                        http.status_code = 503_u16,
                        retry.backoff_ms = backoff.as_millis(),
                        "retrying request after transient failure"
                    );
                    self.record_retry(RetryEvent {
                        method: method.clone(),
                        path: path.clone(),
                        request_class: request_class_str,
                        attempt: retry_attempt,
                        status_code: StatusCode::SERVICE_UNAVAILABLE.as_u16(),
                        backoff_ms: backoff.as_millis(),
                    });
                    tokio::time::sleep(backoff).await;
                    retry_attempt += 1;
                    continue;
                }
                _ => {
                    self.record_completion(RequestCompletion {
                        method: method.clone(),
                        path: path.clone(),
                        request_class: request_class_str,
                        status_code: Some(response.status().as_u16()),
                        error_kind: None,
                        retries: retry_attempt,
                        elapsed_ms: start.elapsed().as_millis(),
                    });
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
    fn record_retry(&self, retry_event: RetryEvent) {
        if let Some(on_retry) = &self.telemetry_hooks.on_retry {
            on_retry(&retry_event);
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    fn record_completion(&self, completion: RequestCompletion) {
        tracing::info!(
            http.status_code = completion.status_code.unwrap_or_default(),
            retries = completion.retries,
            elapsed_ms = completion.elapsed_ms,
            error.kind = ?completion.error_kind,
            "request completed"
        );
        if let Some(on_complete) = &self.telemetry_hooks.on_complete {
            on_complete(&completion);
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

/// Parses the Retry-After header from a 429 response.
///
/// Returns the number of seconds to wait, or None if header is missing/invalid.
fn parse_retry_after(response: &Response) -> Option<u64> {
    response
        .headers()
        .get("Retry-After")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
}

/// Calculates exponential backoff duration for retry attempt.
///
/// Uses formula: base_delay * 2^attempt, capped at 30 seconds.
fn exponential_backoff(attempt: u32) -> Duration {
    let base = Duration::from_millis(500);
    let backoff_ms = base.as_millis() * 2_u128.pow(attempt);
    Duration::from_millis(backoff_ms.min(30_000) as u64)
}

fn classify_request(method: &Method) -> RequestRetryClass {
    if *method == Method::GET || *method == Method::HEAD || *method == Method::OPTIONS {
        RequestRetryClass::Read
    } else {
        RequestRetryClass::Mutation
    }
}

/// Parses Salesforce API error from response body.
///
/// Salesforce error responses typically have this format:
/// ```json
/// [
///   {
///     "errorCode": "INVALID_FIELD",
///     "message": "Field does not exist",
///     "fields": ["InvalidField"]
///   }
/// ]
/// ```
fn parse_api_error(status_code: u16, body: &str) -> HttpError {
    #[derive(serde::Deserialize)]
    struct SalesforceError {
        #[serde(rename = "errorCode")]
        error_code: Option<String>,
        message: String,
        #[serde(default)]
        fields: Vec<String>,
    }

    // Try to parse as Salesforce error array
    if let Ok(errors) = serde_json::from_str::<Vec<SalesforceError>>(body) {
        if let Some(first_error) = errors.first() {
            return HttpError::StatusError {
                status_code,
                message: format!(
                    "[{}] {}",
                    first_error.error_code.as_deref().unwrap_or("UNKNOWN"),
                    first_error.message
                ),
            };
        }
    }

    // Fallback to generic status error
    HttpError::StatusError {
        status_code,
        message: body.to_string(),
    }
}

/// Converts an HTTP error response into a `ForceError` using Salesforce-aware parsing.
///
/// If the response body is empty or unreadable, falls back to `fallback_message`.
pub(crate) async fn response_to_force_error(
    response: Response,
    fallback_message: &str,
) -> crate::error::ForceError {
    let status_code = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();
    let payload = if body.trim().is_empty() {
        fallback_message.to_string()
    } else {
        body
    };
    parse_api_error(status_code, &payload).into()
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_exponential_backoff() {
        assert_eq!(exponential_backoff(0).as_millis(), 500);
        assert_eq!(exponential_backoff(1).as_millis(), 1000);
        assert_eq!(exponential_backoff(2).as_millis(), 2000);
        assert_eq!(exponential_backoff(3).as_millis(), 4000);
        // Cap at 30 seconds
        assert_eq!(exponential_backoff(10).as_millis(), 30_000);
    }

    #[test]
    fn test_parse_api_error_with_salesforce_format() {
        let body =
            r#"[{"errorCode":"INVALID_FIELD","message":"Field does not exist","fields":["Name"]}]"#;
        let error = parse_api_error(400, body);

        if let HttpError::StatusError {
            status_code,
            message,
        } = error
        {
            assert_eq!(status_code, 400);
            assert_eq!(message, "[INVALID_FIELD] Field does not exist");
        } else {
            panic!("Expected StatusError");
        }
    }

    #[test]
    fn test_parse_api_error_fallback() {
        let body = "Some error text";
        let error = parse_api_error(500, body);

        if let HttpError::StatusError {
            status_code,
            message,
        } = error
        {
            assert_eq!(status_code, 500);
            assert_eq!(message, "Some error text");
        } else {
            panic!("Expected StatusError");
        }
    }
}

