# ADR-009: Decouple Storage from Core

**Status:** Accepted
**Date:** 2026-02-17
**Deciders:** Atlas, Codex
**Context:** The `TokenManager` and authentication logic were tightly coupled within the `auth` module. This tight coupling made it difficult to separate concerns, introduce alternative storage backends, and created potential for circular dependencies if storage logic needed to depend on core types while core types depended on storage.

## Context and Problem Statement

The `force` crate is designed to be the core library for interacting with Salesforce APIs. However, it tightly couples authentication logic with token persistence.

**Problem:** The persistence logic is embedded within the `auth` module, leading to:
1.  **Tight Coupling:** Authentication and storage concerns are mixed.
2.  **Bloat:** The core library carries persistence logic that not all consumers need.
3.  **Inflexibility:** Consumers cannot easily swap out the storage implementation without modifying the core crate.

## Decision Drivers

-   **Modularity:** Clean separation between business logic (Core) and infrastructure (Storage).
-   **Maintainability:** Easier to test and reason about storage logic in isolation.
-   **Flexibility:** Allow pluggable storage backends (e.g., disk, Redis, memory) in the future.

## Decision

**Decision:** Move persistence logic (specifically `TokenManager` and related types) to a dedicated `storage` module within the `force` crate (`crates/force/src/storage`). This is the first step towards a fully decoupled storage architecture.

In this specific architectural change, we are extracting the `storage` module to a separate boundary within the crate. This clarifies dependencies and prepares the codebase for potentially moving storage to a separate crate in the future.

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
