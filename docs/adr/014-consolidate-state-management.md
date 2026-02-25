# ADR-014: Consolidate State Management & Rename Inner to Session

**Status:** Accepted
**Date:** 2026-02-18
**Deciders:** Atlas, Codex
**Context:** The `force` crate previously utilized a struct named `Inner` to hold shared state (configuration, HTTP client, token manager) for the `ForceClient` and its handlers. The name `Inner` was generic and failed to convey the semantic meaning of the shared context.

Additionally, ADR-009 proposed decoupling storage logic into a dedicated `crates/force/storage` module/crate. While the decoupling of logic was successful (via `TokenManager`), the implementation ultimately placed this logic within `crates/force/auth/token_manager.rs` to maintain cohesion with authentication strategies, rather than creating a separate top-level module or crate.

**Decision:**

1.  **Rename `Inner` to `Session`:** The shared state struct is renamed to `Session` to explicitly denote its role as the context for API operations, carrying authentication and configuration across requests.
2.  **Consolidate Storage into Auth:** The persistence logic (`TokenManager`) is officially recognized as part of the `Auth` module (`crates/force/auth`). The concept of a separate `Storage` component in architectural diagrams is merged into the `Auth` component to reflect the actual code structure.

**Consequences:**

### Positive
-   **Clarity:** `Session` accurately describes the lifecycle and purpose of the shared state.
-   **Simplicity:** Avoiding a separate `storage` crate reduces project complexity while still maintaining modular separation within `crates/force/auth`.
-   **Accuracy:** Architectural diagrams now match the physical code structure.

### Negative
-   **None:** This change primarily rectifies naming and documentation to match the implemented reality.

## Related ADRs
-   [ADR-009: Decouple Storage from Core](009-decouple-storage-from-core.md)
-   [ADR-010: Internal Shared State](010-internal-shared-state.md)
