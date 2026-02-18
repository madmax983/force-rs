# ADR-010: Internal Shared State Pattern

**Status:** Accepted
**Date:** 2026-02-17
**Deciders:** Codex, Atlas
**Context:**
The `ForceClient` is designed to be the primary entry point for interacting with the Salesforce API. Users frequently create specialized API handlers (e.g., `RestHandler`, `BulkHandler`) which need to share the same underlying configuration, authentication state, and HTTP connection pool.
Without a shared state mechanism, each handler would either need to own a copy of these resources (expensive) or rely on complex lifetime management.
Additionally, the `ForceClient` itself needs to be cheaply cloneable to pass into async tasks or store in application state.

**Decision:**
We have adopted the `Inner` struct pattern, where all shared state is encapsulated in a private `Inner<A>` struct, and `ForceClient<A>` (as well as API handlers) holds an `Arc<Inner<A>>`.

The `Inner` struct contains:
- `ClientConfig` (Immutable configuration)
- `reqwest::Client` (HTTP connection pool, internally ref-counted)
- `HttpExecutor` (Retry/Middleware logic)
- `TokenManager` (Thread-safe token storage)

**Consequences:**
### Positive
- **Cheap Cloning:** `ForceClient` and handlers can be cloned via `Arc::clone`, making them lightweight handles.
- **Thread Safety:** `Arc` ensures thread-safe access to shared resources, allowing concurrent API usage.
- **Unified State:** All handlers share the same token manager, ensuring token refreshes are coordinated.

### Negative
- **Indirection:** There is a layer of indirection (`client.inner.field`) which adds slight complexity to internal implementation.
- **Reference Cycles:** Care must be taken to avoid reference cycles if `Inner` were to hold references back to `ForceClient` (though currently it does not).
