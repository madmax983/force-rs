# ADR-016: Remove Deprecated Auth Re-exports

**Status:** Accepted
**Date:** 2026-03-02
**Deciders:** Codex, Atlas
**Context:**
The `types.rs` module historically contained re-exports for `AccessToken`, `Authenticator`, and `TokenResponse`. These types were fundamentally tied to authentication logic, which had been previously consolidated into the `auth` module (see ADR-002 and earlier refactorings).
Leaving these re-exports in the `types.rs` module blurred the architectural boundaries between domain types and authentication logic. It created a situation where consumers and internal modules might incorrectly depend on `types` for auth-related structs, increasing coupling and violating the single-responsibility principle for modules.

**Decision:**
We have removed the deprecated re-exports for authentication types from `crates/force/src/types/mod.rs` (and related files) and updated all internal usages, particularly in the `experimental` modules, to import these types directly from `crate::auth::`.

**Consequences:**
### Positive
- **Clearer Boundaries:** The separation between `types` (domain entities, IDs, errors) and `auth` (authentication, tokens, credentials) is now strictly enforced.
- **Architectural Transparency:** The codebase structure more accurately reflects the documented architecture, improving maintainability.

### Negative
- **Migration Overhead:** Internal code using the old import paths had to be updated. External consumers relying on `force::types::Authenticator` will face a breaking change and must update their imports to `force::auth::Authenticator`.
