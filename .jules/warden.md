# Warden's Journal

## [2026-02-10] Exponential Backoff Overflow
- **Risk**: Denial of Service (DoS) via Panic.
- **Trigger**: Setting `max_retries` >= 128 caused `u128` overflow in `2^attempt` calculation.
- **Fix**: Capped `attempt` at 32 (2^32 > 30s cap) before exponentiation.
- **Verification**: Added `tests/regression_backoff_overflow.rs`.
