# Warden's Journal

2025-01-27 - Exponential Backoff Overflow
**Threat:** Integer overflow in `2_u128.pow(attempt)` when `attempt >= 128` (or practically 64+), causing a panic.
**Defense:** Capped `attempt` at 64 before calculation.
