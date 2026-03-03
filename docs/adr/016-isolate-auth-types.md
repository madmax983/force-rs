# ADR-016: Isolate Auth Types

**Status:** Proposed
**Date:** 2026-03-02
**Deciders:** Codex, Atlas
**Context:**
The core `crates/force/src/types.rs` module contained re-exports for `AccessToken`, `Authenticator`, and `TokenResponse`. However, these types inherently belong to the `auth` module, as they are central to authentication and token management.
Re-exporting them in a generic `types` module creates a false sense of generic applicability and blurs the module boundaries, making it harder for developers to locate the source of truth for authentication logic. It also violates the principle of high cohesion, as the `types` module should be reserved for fundamental domain primitives (like `SalesforceId`, `ApiVersion`, `QueryResult`) that are used broadly across the entire SDK.

**Decision:**
We have decided to deprecate the re-exports of `AccessToken`, `Authenticator`, and `TokenResponse` in `crates/force/src/types.rs`.
Consumers must now import these types directly from the `force::auth` module where they are defined and primarily used.

**Consequences:**
### Positive
-   **Cohesion:** Authentication types are kept together with authentication logic, improving discoverability and understanding.
-   **Clear Boundaries:** The `types` module is strictly reserved for core domain primitives, preventing it from becoming a catch-all for unrelated structs.
-   **Structural Integrity:** Enforces proper module boundaries, making future refactoring of the `auth` module easier without impacting consumers of the `types` module.

### Negative
-   **Migration:** Existing consumers using the old re-exports will see deprecation warnings and will need to update their imports to `force::auth::*`.
