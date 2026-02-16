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

**Decision:** We have moved persistence logic to a dedicated module `crates/force/src/storage/`.

The `TokenManager` struct now encapsulates storage logic, breaking the circular dependency by relying only on `crate::types` and `crate::error`, rather than depending on the main `ForceClient` or `Inner` types.

The `ForceClient` (Core) depends on `TokenManager` via module import. While initially envisioned as a fully decoupled trait-based injection, the current implementation uses a concrete `TokenManager<A>` struct which is generic over the `Authenticator` trait. This provides sufficient decoupling to resolve the circular dependency and allow different authentication strategies, without the overhead of a separate crate or complex trait bounds for storage itself.

## Consequences

### Positive

-   **Decoupling:** `Core` no longer has a circular dependency with `storage`.
-   **Build Times:** Breaking the cycle allows for parallel compilation of independent modules.
-   **Simplicity:** Using a concrete `TokenManager` with a generic `Authenticator` avoids dynamic dispatch overhead for storage operations.

### Negative

-   **Mockability:** Since `TokenManager` is a concrete struct, mocking the storage layer directly is harder than if it were a trait. However, `Authenticator` is a trait, allowing for flexible auth mocking.
-   **Coupling:** `ForceClient` is still coupled to `TokenManager`'s implementation details (e.g., `RwLock`), making it harder to swap out the entire storage engine without changing `ForceClient`.

## Related ADRs

-   [ADR-001: Workspace Structure](001-workspace-structure.md)
-   [ADR-002: Authentication Strategy](002-authentication-strategy.md)
