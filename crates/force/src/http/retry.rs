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
pub fn exponential_backoff(attempt: u32, base: Duration) -> Duration {
    let base_ms = base.as_millis();
    // Cap should at least be the base duration, otherwise we retry faster than the base
    let max_cap = std::cmp::max(base_ms, u128::from(MAX_BACKOFF_MS));

    // If max_cap exceeds u64::MAX, cap it to u64::MAX to prevent truncation
    // Duration::from_millis only accepts u64, so we can't represent > u64::MAX ms
    let safe_max_cap = u64::try_from(max_cap.min(u128::from(u64::MAX))).unwrap_or(u64::MAX);

    // Cap at 64 to prevent overflow in 2^attempt
    if attempt >= 64 {
        return Duration::from_millis(safe_max_cap);
    }

    let multiplier = 2_u128.pow(attempt);
    let backoff_ms = base_ms.saturating_mul(multiplier);

    // We can safely cast backoff_ms because we min() it with max_cap first
    // And safe_max_cap already handles the truncation case
    let safe_backoff =
        u64::try_from(backoff_ms.min(u128::from(safe_max_cap))).unwrap_or(safe_max_cap);

    Duration::from_millis(safe_backoff)
}

pub fn classify_request(method: &Method) -> RequestRetryClass {
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
pub fn parse_retry_after(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Must;

    #[test]
    fn test_retry_policy_initialization() {
        let policy = RetryPolicy::new(3, 1);
        assert_eq!(policy.read_max_retries, 3);
        assert_eq!(policy.mutation_max_retries, 1);
        // Should default to read_max_retries
        assert_eq!(policy.idempotent_mutation_max_retries, 3);
    }

    #[test]
    fn test_retry_policy_with_idempotent_mutation_retries() {
        let policy = RetryPolicy::new(3, 1).with_idempotent_mutation_retries(5);
        assert_eq!(policy.read_max_retries, 3);
        assert_eq!(policy.mutation_max_retries, 1);
        // Should be overridden
        assert_eq!(policy.idempotent_mutation_max_retries, 5);
    }

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
            let Ok(attempt_u32) = u32::try_from(attempt) else {
                panic!("test attempts exceeded u32");
            };
            assert_eq!(
                exponential_backoff(attempt_u32, base).as_millis(),
                ms,
                "Attempt {}",
                attempt
            );
        }
    }

    #[test]
    fn test_parse_retry_after_valid() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Retry-After", "120".parse().must());
        assert_eq!(parse_retry_after(&headers), Some(120));
    }

    #[test]
    fn test_parse_retry_after_missing() {
        let headers = reqwest::header::HeaderMap::new();
        assert_eq!(parse_retry_after(&headers), None);
    }

    #[test]
    fn test_parse_retry_after_invalid() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Retry-After", "soon".parse().must());
        assert_eq!(parse_retry_after(&headers), None);
    }

    #[test]
    fn test_parse_retry_after_negative() {
        let mut headers = reqwest::header::HeaderMap::new();
        // Header value parsing itself doesn't validate numeric, so "-1" is a valid header value string
        headers.insert("Retry-After", "-1".parse().must());
        // But u64 parsing should fail
        assert_eq!(parse_retry_after(&headers), None);
    }

    #[test]
    fn test_parse_retry_after_empty() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Retry-After", reqwest::header::HeaderValue::from_static(""));
        assert_eq!(parse_retry_after(&headers), None);
    }

    #[test]
    fn test_parse_retry_after_float() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Retry-After",
            reqwest::header::HeaderValue::from_static("1.5"),
        );
        assert_eq!(parse_retry_after(&headers), None);
    }

    #[test]
    fn test_parse_retry_after_large() {
        let mut headers = reqwest::header::HeaderMap::new();
        // Exceeds u64::MAX
        headers.insert(
            "Retry-After",
            reqwest::header::HeaderValue::from_static("18446744073709551616"),
        );
        assert_eq!(parse_retry_after(&headers), None);
    }

    #[test]
    fn test_exponential_backoff_respects_large_base() {
        let base = Duration::from_mins(1);
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

    #[test]
    fn test_exponential_backoff_max_attempts() {
        let base = Duration::from_millis(500);
        let duration = exponential_backoff(u32::MAX, base);
        // Should be capped at 30 seconds
        assert_eq!(duration.as_millis(), 30_000);
    }

    #[test]
    fn test_exponential_backoff_overflow_truncation() {
        // Base larger than u64::MAX should saturate, not truncate
        // 2^64 millis
        let secs = 18_446_744_073_709_551;
        let nanos = 616_000_000;
        let huge_duration = Duration::new(secs, nanos);

        let result = exponential_backoff(65, huge_duration);
        assert_eq!(result.as_millis(), u128::from(u64::MAX));
    }

    #[test]
    fn test_classify_request_all_methods() {
        let methods = vec![
            (Method::GET, RequestRetryClass::Read),
            (Method::HEAD, RequestRetryClass::Read),
            (Method::OPTIONS, RequestRetryClass::Read),
            (Method::TRACE, RequestRetryClass::Read),
            (Method::POST, RequestRetryClass::Mutation),
            (Method::PUT, RequestRetryClass::Mutation),
            (Method::DELETE, RequestRetryClass::Mutation),
            (Method::PATCH, RequestRetryClass::Mutation),
            (Method::CONNECT, RequestRetryClass::Mutation),
        ];

        for (method, expected) in methods {
            assert_eq!(
                classify_request(&method),
                expected,
                "Failed to classify method: {}",
                method
            );
        }
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_exponential_backoff_no_panic(attempt in any::<u32>(), base_ms in any::<u64>()) {
            let base = Duration::from_millis(base_ms);
            let _ = exponential_backoff(attempt, base);
        }

        #[test]
        fn test_exponential_backoff_monotonic(attempt in 0u32..100, base_ms in 1u64..1000) {
            let base = Duration::from_millis(base_ms);
            let t1 = exponential_backoff(attempt, base);
            let t2 = exponential_backoff(attempt + 1, base);
            prop_assert!(t2 >= t1);
        }
    }
}
