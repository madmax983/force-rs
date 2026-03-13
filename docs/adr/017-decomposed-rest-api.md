# ADR-017: Decomposed REST API Module

**Status:** Accepted
**Date:** 2026-03-05
**Deciders:** Codex, Atlas
**Context:**
The `crates/force/src/api/rest` module was growing in complexity and size, handling all REST operations (CRUD, describe, limits, query, search, soql, explain) within a few files or a monolithic `impl RestHandler` block. This centralized approach reduced maintainability, violated the single responsibility principle, and made testing and navigation difficult.

**Decision:**
We have decomposed the REST API module into specialized sub-modules:
1.  **`handler`**: The core `RestHandler` definition and base HTTP capabilities.
2.  **`crud`**: Create, Read, Update, Delete operations.
3.  **`describe`**: Metadata describe operations.
4.  **`explain`**: Query execution plan operations (feature gated behind `nova`).
5.  **`limits`**: Organization limits operations.
6.  **`query`**: SOQL query execution.
7.  **`query_stream`**: Streaming SOQL queries.
8.  **`search`**: SOSL search execution.
9.  **`soql`**: SOQL query builder utilities.

We utilized the facade module pattern (`mod.rs`), where `crates/force/src/api/rest/mod.rs` exposes only the necessary public interfaces via `pub use`. Domain-specific endpoints are separated into their respective modules by utilizing localized `impl<A: Authenticator> RestHandler<A>` blocks, rather than relying on extension traits or a single massive `impl` block.

**Consequences:**
### Positive
-   **High Cohesion:** Related operations are grouped together (e.g., all `query` logic is in one file), improving discoverability and understanding.
-   **Separation of Concerns:** Each module has a single responsibility.
-   **Maintainability:** Smaller file sizes make the codebase easier to navigate, review, and modify.
-   **Encapsulation:** The facade pattern hides internal structural details while presenting a clean public API.

### Negative
-   **Module Complexity:** The `rest` module now contains many files, which could be slightly overwhelming for new contributors compared to a single file.
-   **Scattered Implementation:** The implementation of `RestHandler` is distributed across multiple files, meaning developers must look in specific modules to find the implementations of certain methods.
