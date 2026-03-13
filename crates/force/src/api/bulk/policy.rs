//! Polling policy for Bulk API operations.
//!
//! This module provides the `BulkPollPolicy` which controls how the client polls
//! for job completion, including backoff strategies and timeout configuration.

use std::time::Duration;

/// Polling behavior for asynchronous Bulk API jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BulkPollPolicy {
    /// Maximum number of polling retries while a job remains non-terminal.
    pub max_attempts: u32,
    /// Initial backoff delay before the next poll attempt.
    pub initial_backoff: Duration,
    /// Upper bound for exponential polling backoff.
    pub max_backoff: Duration,
}

impl BulkPollPolicy {
    /// Creates a new polling policy.
    #[must_use]
    pub const fn new(max_attempts: u32, initial_backoff: Duration, max_backoff: Duration) -> Self {
        Self {
            max_attempts,
            initial_backoff,
            max_backoff,
        }
    }

    /// Calculates the backoff duration for a given attempt.
    #[must_use]
    pub(crate) fn backoff_for_attempt(self, attempt: u32) -> Duration {
        let shift = attempt.min(31);
        let multiplier = 1_u32 << shift;
        let Some(backoff) = self.initial_backoff.checked_mul(multiplier) else {
            return self.max_backoff;
        };
        backoff.min(self.max_backoff)
    }

    /// Calculates the total theoretical timeout in seconds.
    #[must_use]
    pub(crate) fn timeout_seconds(self) -> u64 {
        let mut total = Duration::ZERO;
        let mut attempt = 0;
        while attempt < self.max_attempts {
            total = total.saturating_add(self.backoff_for_attempt(attempt));
            attempt += 1;
        }
        total.as_secs()
    }
}

impl Default for BulkPollPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 10,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(30),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backoff_for_attempt_basic() {
        let policy = BulkPollPolicy::new(10, Duration::from_secs(1), Duration::from_secs(30));
        assert_eq!(policy.backoff_for_attempt(0), Duration::from_secs(1));
        assert_eq!(policy.backoff_for_attempt(1), Duration::from_secs(2));
        assert_eq!(policy.backoff_for_attempt(2), Duration::from_secs(4));
        assert_eq!(policy.backoff_for_attempt(3), Duration::from_secs(8));
    }

    #[test]
    fn test_backoff_for_attempt_respects_max() {
        let policy = BulkPollPolicy::new(10, Duration::from_secs(1), Duration::from_secs(10));
        assert_eq!(policy.backoff_for_attempt(3), Duration::from_secs(8));
        assert_eq!(policy.backoff_for_attempt(4), Duration::from_secs(10));
        assert_eq!(policy.backoff_for_attempt(5), Duration::from_secs(10));
    }

    #[test]
    fn test_backoff_for_attempt_overflow_protection() {
        let policy = BulkPollPolicy::new(100, Duration::from_secs(1), Duration::from_secs(30));
        assert_eq!(policy.backoff_for_attempt(32), Duration::from_secs(30));
        assert_eq!(
            policy.backoff_for_attempt(u32::MAX),
            Duration::from_secs(30)
        );
    }

    #[test]
    fn test_timeout_seconds_basic() {
        let policy = BulkPollPolicy::new(4, Duration::from_secs(1), Duration::from_secs(30));
        // 1s + 2s + 4s + 8s = 15s
        assert_eq!(policy.timeout_seconds(), 15);
    }

    #[test]
    fn test_timeout_seconds_with_max() {
        let policy = BulkPollPolicy::new(4, Duration::from_secs(1), Duration::from_secs(5));
        // 1s + 2s + 4s + 5s = 12s
        assert_eq!(policy.timeout_seconds(), 12);
    }

    #[test]
    fn test_timeout_seconds_zero_attempts() {
        let policy = BulkPollPolicy::new(0, Duration::from_secs(1), Duration::from_secs(30));
        assert_eq!(policy.timeout_seconds(), 0);
    }
}
