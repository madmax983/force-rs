# ADR-009: Decouple Storage from Core

**Status:** Proposed
**Date:** 2026-02-09
**Deciders:** Atlas, Codex
**Context:** Circular dependencies were causing build failures between the core logic and persistence layers.

## Context and Problem Statement

The `force` crate's core logic and storage mechanisms were tightly coupled, leading to circular dependencies. This caused build failures and made it difficult to test components in isolation.

**Problem:** How can we structure the application to avoid circular dependencies while maintaining a cohesive architecture?

## Decision Drivers

- **Build Stability** - Eliminate circular dependency errors during compilation.
- **Testability** - Allow independent testing of core logic without storage side effects.
- **Maintainability** - Clear separation of concerns.

## Considered Options

- **Keep existing structure** - Rejected: Continuing to patch the circular dependencies is unsustainable.
- **Decouple Storage** - Accepted: Move persistence logic to a dedicated module/crate.

## Decision Outcome

**Decision:** Move persistence logic to a dedicated `storage` module (and potentially a separate crate in the future).

**Rationale:**
- Breaks the circular dependency cycle.
- Allows `Core` to depend on `Storage` via trait bounds or direct dependency, but not vice-versa.
- Simplifies the build graph.

## Consequences

### Positive

- **Build Stability** - Circular dependencies are resolved.
- **Build Times** - Incremental compilation should be faster due to better separation.
- **Clarity** - Clearer boundaries between business logic and data persistence.

### Negative

- **Complexity** - May introduce some FFI complexity if moved to a separate crate in the workspace.
- **Boilerplate** - May require additional trait definitions to bridge the layers.

## Validation

- The project compiles without circular dependency errors.
- Tests for `Core` run without requiring a full storage implementation (mocking is easier).

## Related Decisions

- [ADR-001: Workspace Structure](001-workspace-structure.md)

## References

- [Refactoring Circular Dependencies](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html)
