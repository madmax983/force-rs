# ADR-009: Decouple Storage from Core

**Status:** Accepted
**Date:** 2026-02-17
**Deciders:** Atlas, Codex
**Context:** Circular dependencies were causing build failures. The `force` crate (Core) depended on the `storage` module for token caching, while the `storage` module depended on `force` for type definitions (e.g., `AccessToken`). This created a circular dependency cycle that made compilation fragile and prevented clean separation of concerns.

## Context and Problem Statement

The `force` crate is designed to be the core library for interacting with Salesforce APIs. However, it tightly couples authentication logic with token persistence.

**Problem:** The `storage` module is currently embedded within `force`, leading to:
1.  **Circular Dependencies:** `force` imports `storage`, and `storage` imports types from `force`.
2.  **Bloat:** The core library carries persistence logic that not all consumers need.
3.  **Inflexibility:** Consumers cannot easily swap out the storage implementation without modifying the core crate.

## Decision Drivers

-   **Modularity:** Clean separation between business logic (Core) and infrastructure (Storage).
-   **Build Performance:** Breaking dependency cycles to improve compilation times.
-   **Flexibility:** Allow pluggable storage backends (e.g., disk, Redis, memory).

## Decision

**Decision:** Move persistence logic to a dedicated module structure (`crates/force/src/storage/`) where `Core` defines the interface (Trait) and `Storage` implements it.

In this specific architectural change, we are extracting the `storage` logic from `auth` to a separate module, paving the way for future extraction to a crate if needed, while solving immediate organization issues.

## Consequences

### Positive

-   **Decoupling:** `Core` no longer depends on concrete storage implementations.
-   **Build Times:** Breaking the cycle allows for parallel compilation of independent crates/modules.
-   **Testability:** `Core` can be tested with mock storage implementations easily.

### Negative

-   **Complexity:** Managing multiple crates or stricter module boundaries adds boilerplate.
-   **FFI Complexity:** If split into separate crates, FFI boundaries might become more complex to manage.
-   **Versioning:** Releasing `force` and `force-storage` might require synchronized versioning.

## Related ADRs

-   [ADR-001: Workspace Structure](001-workspace-structure.md)
-   [ADR-002: Authentication Strategy](002-authentication-strategy.md)
