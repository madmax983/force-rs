# ADR-009: Decouple Storage from Core

**Status:** Proposed
**Date:** 2026-03-01
**Deciders:** Codex (acting on behalf of Atlas)
**Context:** Circular dependencies were causing build failures and increasing compilation times. The core business logic was too tightly coupled with the storage implementation, making it difficult to swap storage backends or test components in isolation.

## Context and Problem Statement

The `force-rs` crate has grown significantly, and the monolithic structure is becoming a bottleneck. Specifically, the persistence layer (`storage`) is intertwined with the core domain logic (`core`), leading to circular dependencies that complicate the build process.

**Problem:** How can we structure the codebase to eliminate circular dependencies and improve modularity?

## Decision Drivers

- **Build Stability** - Eliminate circular dependencies.
- **compilation Speed** - Reduce rebuild times by isolating changes.
- **Testability** - Allow easier mocking of storage interfaces.
- **Modularity** - Enable future support for different storage backends (e.g., SQLite, Redis).

## Decisions

### Decision 1: Move Persistence Logic to a Dedicated Crate

We will extract all storage-related code into a new crate, likely named `force-storage` or simply `storage` within the workspace.

**Rationale:**
- Creates a clear boundary between domain logic and persistence.
- Enforces a unidirectional dependency graph (`Core` depends on `Storage` interfaces, implementation depends on `Core` types, or `App` wires them up).
- Follows the Clean Architecture principle.

### Decision 2: Define Storage Traits in Core

The core crate will define the *interface* (traits) for storage, while the new crate will provide the *implementation*.

**Rationale:**
- Allows `Core` to remain agnostic of the underlying storage mechanism.
- Facilitates dependency injection.

## Consequences

### Positive
- **Build Times:** improved significantly as changes in storage don't trigger core rebuilds (and vice-versa, depending on dependency direction).
- **Decoupling:** Clear separation of concerns.
- **Flexibility:** Easier to add new storage backends.

### Negative
- **Complexity:** Managing multiple crates adds some overhead.
- **FFI Complexity:** If we expose C bindings, the separation might complicate the FFI layer.
