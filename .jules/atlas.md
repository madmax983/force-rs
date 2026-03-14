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

**2024-05-26 - [Decomposed Bulk API Module]**
**Tangle:** The `crates/force/src/api/bulk/mod.rs` file was a "Blob" (1789 lines) containing `BulkHandler` definition, `BulkPollPolicy`, all inherent implementation methods (ingest, query), and massive tests.
**Blueprint:** Refactored `bulk` module into cohesive submodules: `handler.rs` (struct def), `policy.rs` (polling logic), `ingest.rs` (ingest methods), and `query.rs` (query methods). `mod.rs` is now a facade. Used Rust's ability to split inherent implementations across modules in the same crate to maintain the public API without extension traits.

**2024-05-27 - [The Session: Breaking the Cycle]**
**Tangle:** The `client` module (ForceClient) and `api` modules (handlers) were in a circular dependency at the module level. `client` imported `api` to expose handlers, but `api` imported `client::inner` to access shared state. This "Hub and Spoke" issue meant `api` could not exist without `client`'s internal structure.
**Blueprint:** Extracted the `Inner` struct into a new `session` module as `Session`. `ForceClient` and all `api` handlers now depend on `session::Session` for their shared state. This creates a clean DAG: `client` -> `session`, `api` -> `session`, and `client` -> `api` (for convenience methods), with no back-references from `api` to `client`.

**[Removed Deprecated Auth Re-exports]
**Tangle:** The `types.rs` module contained deprecated re-exports for `AccessToken`, `Authenticator`, and `TokenResponse` which had been moved to the `auth` module, blurring module boundaries.
**Blueprint:** Removed the re-exports from `types.rs` and updated all internal usages to import directly from `crate::auth::Authenticator`.

**2024-05-28 - [ApiError Struct Duplication]**
**Tangle:** There were two identical `ApiError` structs: one in `crates/force/src/error.rs` (used as a `thiserror` variant) and one in `crates/force/src/types/common.rs` (used for deserializing JSON responses from Salesforce). This violated DRY and caused domain confusion, as they effectively represented the exact same concept but required different serialization logic (e.g. `errorCode` vs `statusCode` depending on REST vs Composite APIs).
**Blueprint:** Merged `crate::types::common::ApiError` into `crate::error::ApiError`. Added `Serialize, Deserialize, Clone, PartialEq, Eq` to the unified struct, and mapped both `statusCode` and `errorCode` JSON fields via `#[serde(alias)]`. Removed the duplicate struct and updated the `types` module to re-export `crate::error::ApiError` so the public API contract is not broken.

**2024-05-29 - [The Facade: Enforcing Module Boundaries]**
**Tangle:** Facade modules (`auth`, `http`, `types`, `api::bulk`, `api::rest`, `api::composite`, `experimental`) were defined as `pub mod`, leaking their internal structural implementation details. They were exposing both their internal sub-module structures and the types via `pub use`, which blurred the boundaries of what is internal organization and what is public contract.
**Blueprint:** Refactored module visibility in the Facade files from `pub mod` to `pub(crate) mod` (or standard `mod` where applicable) while retaining explicit `pub use` for strictly public interfaces. If specific sub-modules genuinely represent a logical grouping needed by consumers (e.g. `api::rest::describe` for its return types), they were explicitly kept as `pub mod`. This enforces encapsulation, reduces IDE autocompletion noise, and solidifies the public API contract as an explicit graph node.

**2024-05-30 - [The Facade: Fixing the Leak in Composite and Experimental]**
**Tangle:** The `api::composite` and `experimental` modules leaked their internal structures (`batch`, `query_batch`, `schema_graph`, etc.) directly into the public API by declaring them as `pub mod`. This violated the Facade pattern and exposed implementation details that users shouldn't depend on.
**Blueprint:** Changed visibility of these internal modules to `pub(crate) mod` and explicitly re-exported their main public structs/enums (like `BatchBuilder`, `SchemaGraph`, `DataDictionary`) using `pub use` statements at the module root (`api/composite/mod.rs` and `experimental/mod.rs`). This enforces encapsulation and presents a clean, un-nested API surface to consumers.
**[Extracted SOQL Query Builder]**
**Tangle:** The `SoqlQueryBuilder` was located in `crates/force/src/api/rest/soql.rs`, which created an inverted dependency where sibling modules like `composite` (specifically `BatchBuilder`) had to reach into the `rest` module to construct queries.
**Blueprint:** Moved `SoqlQueryBuilder` to the root `api` module (`crates/force/src/api/soql.rs`) to establish a clear structural boundary and eliminate the leaky abstraction. The `rest` module now re-exports it to preserve backward compatibility.
**[Facade] RestAPI Module Consolidation**
**Tangle:** The `force::api::rest` sub-modules (`describe`, `limits`, `search`) were publicly exposed, leaking the internal module structure and forcing users to navigate a nested module hierarchy.
**Blueprint:** Applied the Facade pattern by changing sub-module visibility to `pub(crate)` and selectively re-exporting only the public types (Structs, Enums, Builders) via `pub use` at the root of `force::api::rest::mod.rs`.
