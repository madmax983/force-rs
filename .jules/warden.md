# Warden's Journal

## [2024-05-22] - Backoff Overflow Fix

- **Vulnerability:** Integer overflow in `exponential_backoff` when `attempt` >= 120.
- **Trigger:** High `max_retries` configuration + prolonged server downtime.
- **Outcome:** Panic in release mode (wrapping logic might produce 0 delay) or debug mode (panic).
- **Fix:** Explicitly cap `attempt` at 6 (30s) to bypass calculation.
- **Verification:** Regression test `regression_backoff_overflow` using `proptest`.
