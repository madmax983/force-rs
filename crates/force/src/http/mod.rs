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
use reqwest::{Request, Response, StatusCode};
use serde::de::DeserializeOwned;
use std::time::Duration;

/// HTTP executor that handles middleware concerns.
///
/// The executor manages all HTTP communication with Salesforce, applying
/// middleware for authentication, retries, and error handling.
#[derive(Debug, Clone)]
pub struct HttpExecutor {
    /// The underlying HTTP client.
    client: reqwest::Client,
    /// Maximum number of retry attempts.
    max_retries: u32,
    /// Base timeout for requests.
    timeout: Duration,
}

impl HttpExecutor {
    /// Creates a new HTTP executor with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            max_retries: 3,
            timeout: Duration::from_secs(30),
        }
    }

    /// Creates a new HTTP executor with custom configuration.
    #[must_use]
    pub fn with_config(max_retries: u32, timeout: Duration) -> Self {
        Self {
            client: reqwest::Client::new(),
            max_retries,
            timeout,
        }
    }

    /// Creates a new HTTP executor with a preconfigured reqwest client.
    #[must_use]
    pub fn with_client(client: reqwest::Client, max_retries: u32, timeout: Duration) -> Self {
        Self {
            client,
            max_retries,
            timeout,
        }
    }

    /// Executes a request with auth/retry behavior and returns the raw response.
    ///
    /// This method does not parse non-success status bodies into API errors.
    /// It is intended for higher-level handlers that need status-dependent behavior.
    pub async fn execute_response<F, Fut>(
        &self,
        mut request: Request,
        token: &AccessToken,
        refresh_token: F,
    ) -> Result<Response>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<AccessToken>>,
    {
        // Inject Bearer token
        let auth_header = format!("Bearer {}", token.as_str());
        let header_value = auth_header.parse().map_err(|_| {
            HttpError::InvalidUrl(format!("invalid authorization header: {}", auth_header))
        })?;
        request.headers_mut().insert("Authorization", header_value);

        // Execute with retry logic
        let mut attempt = 0;
        loop {
            let req_clone = request.try_clone().ok_or_else(|| {
                HttpError::InvalidUrl("cannot clone request for retry".to_string())
            })?;

            let response = tokio::time::timeout(self.timeout, self.client.execute(req_clone))
                .await
                .map_err(|_| HttpError::Timeout {
                    timeout_seconds: self.timeout.as_secs(),
                })?
                .map_err(HttpError::from)?;

            match response.status() {
                StatusCode::UNAUTHORIZED => {
                    // 401: Refresh token and retry once
                    if attempt == 0 {
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
                        attempt += 1;
                        continue;
                    }
                    return Ok(response);
                }
                StatusCode::TOO_MANY_REQUESTS => {
                    // 429: Rate limit - respect Retry-After header
                    let retry_after = parse_retry_after(&response).unwrap_or(60);
                    return Err(HttpError::RateLimitExceeded {
                        retry_after_seconds: retry_after,
                    }
                    .into());
                }
                StatusCode::SERVICE_UNAVAILABLE if attempt < self.max_retries => {
                    // 503: Retry with exponential backoff
                    let backoff = exponential_backoff(attempt);
                    tokio::time::sleep(backoff).await;
                    attempt += 1;
                    continue;
                }
                _ => return Ok(response),
            }
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

