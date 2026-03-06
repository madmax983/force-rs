# ADR-017: Decouple Storage from Core

**Status:** Proposed
**Context:** Circular dependencies were causing build failures and tight coupling between the core logic and persistence mechanisms.
**Decision:** Move persistence logic to a dedicated crate to isolate storage concerns and break the circular dependency.
**Consequences:** Build times improve, but FFI complexity increases slightly due to cross-crate boundaries.
