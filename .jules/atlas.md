# Atlas's Journal 🗺️

**2024-05-22 - [The Knot: ForceClient vs API Handlers]**
**Tangle:** The `client` module (containing `ForceClient`) depends on `api` modules (`rest`, `bulk`) to expose handlers via methods like `client.rest()`. However, these handlers depend back on the `client` module to access the `Inner` struct which holds the shared state (HTTP client, config, token manager). This creates a circular dependency between `client` and `api` at the module level.
**Blueprint:** Extract the `Inner` struct and its implementation into a new leaf module `client::inner`. The `api` handlers will then depend on `client::inner` instead of the top-level `client` module. `ForceClient` will depend on `client::inner` and `api`. This breaks the cycle and establishes a clear DAG: `client` -> `api` -> `client::inner`.

**2024-05-23 - [The Sprawl: Consolidating API Methods]**
**Tangle:** The `api::rest` modules (`crud`, `query`, `search`, etc.) were extending `ForceClient` with convenience methods, creating a sprawl of business logic on the main client struct and introducing a conceptual circular dependency between `client` and `api`.
**Blueprint:** Removed `impl ForceClient` blocks from `api::rest` modules. Moved `query` implementation to `RestHandler`. Standardized all API access through handlers (`client.rest().method(...)`, `client.bulk().method(...)`). This enforces high cohesion and eliminates the cycle.

**2024-05-24 - [Unified Authentication Module]**
**Tangle:** Authentication logic (`TokenManager`), types (`AccessToken`), and traits (`Authenticator`) were scattered across `storage`, `types`, and `auth` modules, creating a fragmented domain model and confusing import paths. `storage` was a misnomer for an in-memory token manager.
**Blueprint:** Consolidated all authentication-related code into `crates/force/src/auth`. Moved `TokenManager`, `AccessToken`, and `Authenticator` to `auth`. Re-exported types from `types` for backward compatibility but deprecated the old locations structure-wise. Removed the `storage` module entirely.

**2024-05-25 - [Decomposed HTTP Module]**
**Tangle:** The `http` module was a "Blob" containing mixed concerns: execution logic, retry policies, telemetry, and error parsing (over 600 lines).
**Blueprint:** Refactored `http` into four cohesive submodules: `executor` (HTTP execution), `retry` (policies & backoff), `telemetry` (hooks & events), and `error` (parsing logic). This separates concerns and improves maintainability.

**2024-05-25 - [Removed Zombie Code]**
**Tangle:** `crates/force/src/api/pub_sub.rs` existed as a placeholder for a removed feature (ADR-011).
**Blueprint:** Deleted the file and removed the module declaration to keep the codebase clean.
