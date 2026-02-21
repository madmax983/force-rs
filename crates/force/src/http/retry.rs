//! HTTP retry logic and policies.

use reqwest::Method;
use std::time::Duration;

const MAX_BACKOFF_MS: u64 = 30_000;

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
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::IdempotentMutation => "idempotent_mutation",
            Self::Mutation => "mutation",
        }
    }
}

/// Calculates exponential backoff duration for retry attempt.
///
/// Uses formula: base_delay * 2^attempt, capped at 30 seconds (or base_delay if larger).
pub(crate) fn exponential_backoff(attempt: u32, base: Duration) -> Duration {
    let base_ms = base.as_millis();
    // Cap should at least be the base duration, otherwise we retry faster than the base
    let max_cap = std::cmp::max(base_ms, u128::from(MAX_BACKOFF_MS));

    // Cap at 64 to prevent overflow in 2^attempt
    if attempt >= 64 {
        #[allow(clippy::cast_possible_truncation)]
        return Duration::from_millis(max_cap as u64);
    }

    let multiplier = 2_u128.pow(attempt);
    let backoff_ms = base_ms.saturating_mul(multiplier);

    #[allow(clippy::cast_possible_truncation)]
    Duration::from_millis(backoff_ms.min(max_cap) as u64)
}

pub(crate) fn classify_request(method: &Method) -> RequestRetryClass {
    if matches!(
        *method,
        Method::GET | Method::HEAD | Method::OPTIONS | Method::TRACE
    ) {
        RequestRetryClass::Read
    } else {
        RequestRetryClass::Mutation
    }
}

/// Parses the Retry-After header from a 429 response.
///
/// Returns the number of seconds to wait, or None if header is missing/invalid.
pub(crate) fn parse_retry_after(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    headers
        .get("Retry-After")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exponential_backoff() {
        let base = Duration::from_millis(500);
        assert_eq!(exponential_backoff(0, base).as_millis(), 500);
        assert_eq!(exponential_backoff(1, base).as_millis(), 1000);
        assert_eq!(exponential_backoff(2, base).as_millis(), 2000);
        assert_eq!(exponential_backoff(3, base).as_millis(), 4000);
        // Cap at 30 seconds
        assert_eq!(exponential_backoff(10, base).as_millis(), 30_000);
    }

    #[test]
    fn test_exponential_backoff_overflow() {
        // This should not panic even with large inputs
        let base = Duration::from_millis(500);
        let duration = exponential_backoff(200, base);
        assert_eq!(duration.as_millis(), 30_000);
    }

    #[test]
    fn test_classify_request() {
        assert_eq!(classify_request(&Method::GET), RequestRetryClass::Read);
        assert_eq!(classify_request(&Method::HEAD), RequestRetryClass::Read);
        assert_eq!(classify_request(&Method::OPTIONS), RequestRetryClass::Read);

        assert_eq!(classify_request(&Method::POST), RequestRetryClass::Mutation);
        assert_eq!(classify_request(&Method::PUT), RequestRetryClass::Mutation);
        assert_eq!(
            classify_request(&Method::DELETE),
            RequestRetryClass::Mutation
        );
        assert_eq!(
            classify_request(&Method::PATCH),
            RequestRetryClass::Mutation
        );
        assert_eq!(
            classify_request(&Method::CONNECT),
            RequestRetryClass::Mutation
        );
        assert_eq!(classify_request(&Method::TRACE), RequestRetryClass::Read);
    }

    #[test]
    fn test_exponential_backoff_values() {
        // Verify exact sequence for first few attempts
        // Base 500ms, Multiplier 2^attempt
        // 0: 500 * 1 = 500
        // 1: 500 * 2 = 1000
        // 2: 500 * 4 = 2000
        // 3: 500 * 8 = 4000
        // 4: 500 * 16 = 8000
        // 5: 500 * 32 = 16000
        // 6: 500 * 64 = 32000 -> capped at 30000

        let expected = [500, 1000, 2000, 4000, 8000, 16000, 30000];
        let base = Duration::from_millis(500);

        for (attempt, &ms) in expected.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let attempt_u32 = attempt as u32;
            assert_eq!(
                exponential_backoff(attempt_u32, base).as_millis(),
                ms,
                "Attempt {}",
                attempt
            );
        }
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_parse_retry_after_valid() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Retry-After", "120".parse().unwrap());
        assert_eq!(parse_retry_after(&headers), Some(120));
    }

    #[test]
    fn test_parse_retry_after_missing() {
        let headers = reqwest::header::HeaderMap::new();
        assert_eq!(parse_retry_after(&headers), None);
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_parse_retry_after_invalid() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Retry-After", "soon".parse().unwrap());
        assert_eq!(parse_retry_after(&headers), None);
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_parse_retry_after_negative() {
        let mut headers = reqwest::header::HeaderMap::new();
        // Header value parsing itself doesn't validate numeric, so "-1" is a valid header value string
        headers.insert("Retry-After", "-1".parse().unwrap());
        // But u64 parsing should fail
        assert_eq!(parse_retry_after(&headers), None);
    }

    #[test]
    fn test_exponential_backoff_respects_large_base() {
        let base = Duration::from_secs(60);
        // We expect at least 60s, but the old implementation capped it at 30s
        assert_eq!(exponential_backoff(0, base).as_secs(), 60);
    }

    #[test]
    fn test_request_retry_class_as_str() {
        assert_eq!(RequestRetryClass::Read.as_str(), "read");
        assert_eq!(
            RequestRetryClass::IdempotentMutation.as_str(),
            "idempotent_mutation"
        );
        assert_eq!(RequestRetryClass::Mutation.as_str(), "mutation");
    }
}
