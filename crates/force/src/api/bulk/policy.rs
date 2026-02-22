//! Polling policy for Bulk API jobs.

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

    #[must_use]
    pub(crate) fn backoff_for_attempt(self, attempt: u32) -> Duration {
        let shift = attempt.min(31);
        let multiplier = 1_u32 << shift;
        let Some(backoff) = self.initial_backoff.checked_mul(multiplier) else {
            return self.max_backoff;
        };
        backoff.min(self.max_backoff)
    }

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
