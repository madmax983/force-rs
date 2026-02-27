# ADR-015: Merge Storage Logic into Auth

**Status:** Accepted
**Date:** 2026-02-18
**Deciders:** Codex, Atlas
**Context:**
ADR-009 ("Decouple Storage from Core") proposed moving storage logic to a dedicated `crates/force/src/storage/` module to break circular dependencies between the core client and token management.
However, during implementation, the `TokenManager` (which handles persistence/caching) was placed within `crates/force/src/auth/token_manager.rs`.
There is no `storage` module in the codebase.
This discrepancy between the architectural record and the code creates confusion and violates the principle of Architectural Transparency.

**Decision:**
We have decided to formalize the current structure and supersede ADR-009.
Token storage logic will remain co-located with authentication logic in the `crates/force/src/auth` module.

The `TokenManager` struct handles:
1.  **Acquisition:** Using the `Authenticator` trait to fetch new tokens.
2.  **Storage:** caching the token in an in-memory `RwLock` (and potentially future persistence layers).

This approach still achieves the primary goal of ADR-009 (breaking circular dependencies) because `TokenManager` depends only on `crate::types` and `crate::error`, not on the high-level `ForceClient`.

**Consequences:**
### Positive
-   **Accuracy:** The documentation now accurately reflects the codebase structure.
-   **Simplicity:** Reduces module sprawl by keeping closely related concerns (Auth & Token Management) together.
-   **Discoverability:** Developers looking for "how tokens are stored" will naturally look in `auth`.

### Negative
-   **Coupling:** Storage logic is more tightly coupled to Authentication logic than if it were in a completely agnostic `storage` crate, but this is acceptable for the current scale.

## Related ADRs
- Supersedes [ADR-009: Decouple Storage from Core](009-decouple-storage-from-core.md)
