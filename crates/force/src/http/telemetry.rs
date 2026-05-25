//! Telemetry and observability types for HTTP requests.

use super::retry::RequestRetryClass;
use std::sync::Arc;
use std::time::Instant;

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
///
/// ⚡ Bolt: Uses `&'a str` instead of `String` for `method` and `path` to eliminate
/// heap allocations during high-frequency HTTP request retries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryEvent<'a> {
    /// HTTP method.
    pub method: &'a str,
    /// URL path only (query excluded).
    pub path: &'a str,
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
///
/// ⚡ Bolt: Uses `&'a str` instead of `String` for `method` and `path` to eliminate
/// heap allocations during high-frequency HTTP request completions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestCompletion<'a> {
    /// HTTP method.
    pub method: &'a str,
    /// URL path only (query excluded).
    pub path: &'a str,
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
type RetryHook = Arc<dyn for<'a> Fn(&RetryEvent<'a>) + Send + Sync>;
type CompletionHook = Arc<dyn for<'a> Fn(&RequestCompletion<'a>) + Send + Sync>;

/// Optional telemetry hooks for retry and completion events.
#[derive(Clone, Default)]
pub struct TelemetryHooks {
    pub(crate) on_retry: Option<RetryHook>,
    pub(crate) on_complete: Option<CompletionHook>,
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
        F: for<'a> Fn(&RetryEvent<'a>) + Send + Sync + 'static,
    {
        self.on_retry = Some(Arc::new(hook));
        self
    }

    /// Registers a completion callback.
    #[must_use]
    pub fn on_complete<F>(mut self, hook: F) -> Self
    where
        F: for<'a> Fn(&RequestCompletion<'a>) + Send + Sync + 'static,
    {
        self.on_complete = Some(Arc::new(hook));
        self
    }

    /// Checks if any telemetry hooks are registered.
    pub fn has_hooks(&self) -> bool {
        self.on_retry.is_some() || self.on_complete.is_some()
    }
}

pub struct TelemetryContext<'a> {
    method: Option<&'a str>,
    path: Option<&'a str>,
    pub(crate) request_class: &'static str,
    pub(crate) start_time: Instant,
}

impl<'a> TelemetryContext<'a> {
    pub(crate) fn new(
        method: &'a str,
        path: &'a str,
        request_class: RequestRetryClass,
        capture: bool,
    ) -> Self {
        Self {
            method: if capture {
                Some(method)
            } else {
                None
            },
            path: if capture {
                Some(path)
            } else {
                None
            },
            request_class: request_class.as_str(),
            start_time: Instant::now(),
        }
    }

    pub(crate) fn create_completion(
        &self,
        status_code: Option<u16>,
        error_kind: Option<RequestErrorKind>,
        retries: u32,
    ) -> RequestCompletion<'_> {
        RequestCompletion {
            method: self.method.unwrap_or_default(),
            path: self.path.unwrap_or_default(),
            request_class: self.request_class,
            status_code,
            error_kind,
            retries,
            elapsed_ms: self.start_time.elapsed().as_millis(),
        }
    }

    pub(crate) fn create_retry_event(
        &self,
        attempt: u32,
        status_code: u16,
        backoff_ms: u128,
    ) -> RetryEvent<'_> {
        RetryEvent {
            method: self.method.unwrap_or_default(),
            path: self.path.unwrap_or_default(),
            request_class: self.request_class,
            attempt,
            status_code,
            backoff_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_hooks_debug() {
        let hooks = TelemetryHooks::new();
        let debug_str = format!("{:?}", hooks);
        assert!(debug_str.contains("TelemetryHooks"));
        assert!(debug_str.contains("has_on_retry: false"));
        assert!(debug_str.contains("has_on_complete: false"));

        let hooks = hooks.on_retry(|_| {});
        let debug_str = format!("{:?}", hooks);
        assert!(debug_str.contains("has_on_retry: true"));
        assert!(debug_str.contains("has_on_complete: false"));

        let hooks = hooks.on_complete(|_| {});
        let debug_str = format!("{:?}", hooks);
        assert!(debug_str.contains("has_on_retry: true"));
        assert!(debug_str.contains("has_on_complete: true"));
    }

    #[test]
    fn test_telemetry_hooks_has_hooks() {
        let mut hooks = TelemetryHooks::new();
        assert!(!hooks.has_hooks());

        hooks = hooks.on_retry(|_| {});
        assert!(hooks.has_hooks());

        let hooks2 = TelemetryHooks::new().on_complete(|_| {});
        assert!(hooks2.has_hooks());
    }

    #[test]
    fn test_telemetry_context_creation() {
        let ctx = TelemetryContext::new("GET", "/test", RequestRetryClass::Read, true);
        assert_eq!(ctx.method.as_deref(), Some("GET"));
        assert_eq!(ctx.path.as_deref(), Some("/test"));
        assert_eq!(ctx.request_class, "read");

        let ctx_no_capture =
            TelemetryContext::new("POST", "/test2", RequestRetryClass::Mutation, false);
        assert_eq!(ctx_no_capture.method, None);
        assert_eq!(ctx_no_capture.path, None);
        assert_eq!(ctx_no_capture.request_class, "mutation");
    }

    #[test]
    fn test_create_completion() {
        let ctx = TelemetryContext::new("GET", "/test", RequestRetryClass::Read, true);
        let completion = ctx.create_completion(Some(200), None, 1);

        assert_eq!(completion.method, "GET");
        assert_eq!(completion.path, "/test");
        assert_eq!(completion.request_class, "read");
        assert_eq!(completion.status_code, Some(200));
        assert_eq!(completion.error_kind, None);
        assert_eq!(completion.retries, 1);

        let ctx_no_capture = TelemetryContext::new("GET", "/test", RequestRetryClass::Read, false);
        let completion2 =
            ctx_no_capture.create_completion(None, Some(RequestErrorKind::Timeout), 0);

        assert_eq!(completion2.method, "");
        assert_eq!(completion2.path, "");
        assert_eq!(completion2.error_kind, Some(RequestErrorKind::Timeout));
    }

    #[test]
    fn test_create_retry_event() {
        let ctx = TelemetryContext::new("GET", "/test", RequestRetryClass::Read, true);
        let retry = ctx.create_retry_event(2, 503, 1000);

        assert_eq!(retry.method, "GET");
        assert_eq!(retry.path, "/test");
        assert_eq!(retry.request_class, "read");
        assert_eq!(retry.attempt, 2);
        assert_eq!(retry.status_code, 503);
        assert_eq!(retry.backoff_ms, 1000);

        let ctx_no_capture = TelemetryContext::new("GET", "/test", RequestRetryClass::Read, false);
        let retry2 = ctx_no_capture.create_retry_event(1, 429, 500);

        assert_eq!(retry2.method, "");
        assert_eq!(retry2.path, "");
        assert_eq!(retry2.status_code, 429);
    }
}
