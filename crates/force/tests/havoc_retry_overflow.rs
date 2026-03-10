//! Tests for preventing integer overflows in retry policies.
#![allow(missing_docs)]

use force::api::bulk::BulkPollPolicy;
use std::time::Duration;

#[test]
fn test_bulk_poll_policy_timeout_seconds_overflow() {
    let policy = BulkPollPolicy::new(u32::MAX, Duration::from_secs(1), Duration::from_secs(30));
    // Verify that timeout_seconds calculation does not cause integer overflow panics
    // when attempting to calculate for an extremely high number of max attempts.
    let timeout = policy.timeout_seconds();
    assert!(timeout > 0);
}
