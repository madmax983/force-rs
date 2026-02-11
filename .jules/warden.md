2026-02-11 - Integer Overflow Protection
**Threat:** Integer overflow in exponential backoff calculation allowed potential panic.
**Defense:** Capped retry attempt count and switched to saturating multiplication.

2026-02-11 - Date Overflow Protection
**Threat:** Date overflow in token expiration calculation allowed potential panic if `expires_in` was extremely large.
**Defense:** Switched to checked arithmetic for `Duration` addition, handling overflow by defaulting to safe state (no expiration or forced refresh).
