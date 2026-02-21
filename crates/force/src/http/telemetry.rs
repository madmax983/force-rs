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

    /// Checks if any telemetry hooks are registered.
    pub fn has_hooks(&self) -> bool {
        self.on_retry.is_some() || self.on_complete.is_some()
    }
}

pub(crate) struct TelemetryContext {
    method: Option<String>,
    path: Option<String>,
    pub(crate) request_class: &'static str,
    pub(crate) start_time: Instant,
}

impl TelemetryContext {
    pub(crate) fn new(
        method: &str,
        path: &str,
        request_class: RequestRetryClass,
        capture: bool,
    ) -> Self {
        Self {
            method: if capture {
                Some(method.to_string())
            } else {
                None
            },
            path: if capture {
                Some(path.to_string())
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
    ) -> RequestCompletion {
        RequestCompletion {
            method: self.method.clone().unwrap_or_default(),
            path: self.path.clone().unwrap_or_default(),
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
    ) -> RetryEvent {
        RetryEvent {
            method: self.method.clone().unwrap_or_default(),
            path: self.path.clone().unwrap_or_default(),
            request_class: self.request_class,
            attempt,
            status_code,
            backoff_ms,
        }
    }
}
