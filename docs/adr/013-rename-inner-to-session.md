# ADR-013: Rename Inner to Session

**Status:** Accepted
**Date:** 2026-02-18
**Deciders:** Codex, Atlas
**Context:**
The `ForceClient` and its handlers (`RestHandler`, `BulkHandler`) share state via a reference-counted struct previously named `Inner`.
The name `Inner` is a generic Rust convention for private implementation details, but in this case, the struct plays a critical architectural role: it holds the `ClientConfig`, the `TokenManager`, and the `HttpExecutor`. It effectively represents the user's active session with the Salesforce API.
Referring to it as `Inner` in documentation and discussions was vague and did not convey its significance or lifecycle.

**Decision:**
We have renamed the `Inner` struct to `Session`.
This struct is located in `crates/force/src/session.rs`.

The `Session` struct is responsible for:
-   Holding the immutable `ClientConfig`.
-   Managing the `TokenManager` (stateful authentication).
-   Providing the `HttpExecutor` for making requests.

**Consequences:**
### Positive
-   **Semantic Clarity:** The name `Session` clearly indicates that this object represents a continuous, stateful interaction with the API.
-   **Documentation:** It is easier to explain that "Multiple handlers share the same Session" than "share the same Inner".
-   **Discoverability:** The file `session.rs` is more discoverable than `inner.rs`.

### Negative
-   **Churn:** Existing code referencing `Inner` (mostly internal) had to be updated.
