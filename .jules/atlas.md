**2024-05-26 - [The Bulk API Module]**
**Tangle:** The `crates/force/src/api/bulk/mod.rs` file was a "Blob" (1789 lines) containing `BulkHandler` definition, `BulkPollPolicy`, all inherent implementation methods (ingest, query), and massive tests.
**Blueprint:** Refactored `bulk` module into cohesive submodules: `handler.rs` (struct def), `policy.rs` (polling logic), `ingest.rs` (ingest methods), and `query.rs` (query methods). `mod.rs` is now a facade. Used Rust's ability to split inherent implementations across modules in the same crate to maintain the public API without extension traits.

**2024-05-27 - [The Session: Breaking the Cycle]**
**Tangle:** The `client` module (ForceClient) and `api` modules (handlers) were in a circular dependency at the module level. `client` imported `api` to expose handlers, but `api` imported `client::inner` to access shared state. This "Hub and Spoke" issue meant `api` could not exist without `client`'s internal structure.
**Blueprint:** Extracted the `Inner` struct into a new `session` module as `Session`. `ForceClient` and all `api` handlers now depend on `session::Session` for their shared state. This creates a clean DAG: `client` -> `session`, `api` -> `session`, and `client` -> `api` (for convenience methods), with no back-references from `api` to `client`.

**[Removed Deprecated Auth Re-exports]**
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

**[The Facade: Enforcing Module Boundaries in Bulk API]**
**Tangle:** The `api::bulk` module leaked its internal structures (`ingest`, `query`, `types`, `csv`, `smart_ingest`) directly into the public API by declaring them as `pub mod`. This violated the Facade pattern and exposed implementation details that users shouldn't depend on.
**Blueprint:** Changed visibility of these internal modules to `pub(crate) mod` and explicitly re-exported their main public structs/enums using `pub use` statements at the module root (`api/bulk/mod.rs`). This enforces encapsulation and presents a clean, un-nested API surface to consumers.

**[The Facade: Eliminating Empty Static Structs]**
**Tangle:** Several experimental utilities (`SchemaAnalyzer`, `SchemaDiff`, `SchemaChangelogGenerator`) were implemented as empty, stateless structs with methods (e.g., `pub struct SchemaAnalyzer; impl SchemaAnalyzer { pub fn analyze(...) }`). This is a Java-style object-oriented anti-pattern in Rust that introduces unnecessary namespacing, obfuscates intent, and requires boilerplate instantiations.
**Blueprint:** Refactored these structs into simple module-level free functions (`analyze_schema`, `compare_schemas`, `generate_changelog`). This aligns with idiomatic Rust, enforces simplicity (KISS), and cleans up the public API by removing empty structs that carry no data.

**[The Facade: Eliminating Empty Static Struct in sql_exporter]**
**Tangle:** The `sql_exporter` module was exposing an empty static struct `SqlExporter` which acts as a namespace instead of data. Also, `sql_exporter` was `pub mod` which leaked internal boundaries.
**Blueprint:** Refactored `SqlExporter` into `generate_ddl` free function. Changed `sql_exporter` to `pub(crate) mod` and exported `generate_ddl` via `pub use` in `experimental/mod.rs`.

**[The Facade: Eliminating Empty Static Struct in data_faker]**
**Tangle:** The `DataFaker` struct was an empty static struct used only as a namespace for `generate_mock_record`.
**Blueprint:** Refactored `DataFaker` into the free function `generate_mock_record`.

**[The Facade: Eliminating Empty Static Structs for Type Generators]**
**Tangle:** The `type_generator` and `typescript_generator` modules were exposing empty static structs `StructGenerator` and `TypescriptGenerator`, which acted merely as namespaces for `generate` functions. This is a Java-style object-oriented anti-pattern in Rust that introduces unnecessary namespacing and obfuscates intent.
**Blueprint:** Refactored `StructGenerator` into the `generate_rust_struct` free function and `TypescriptGenerator` into the `generate_typescript_interface` free function. Removed the empty structs and their `impl` blocks. Exported the free functions via `pub use` in `experimental/mod.rs` to present a clean, idiomatic Rust API.

**2024-05-31 - [The Facade: Finalizing Module Boundaries for API Handlers]**
**Tangle:** The remaining `force::api` submodules (`graphql`, `data_cloud`, `cpq`, `consent`, `tooling`, `ui`) were defined as `pub mod`, leaking their internal structural implementation details (like `types`, `error`, `query`, etc.) directly into the public API.
**Blueprint:** Refactored module visibility in these Facade files from `pub mod` to `pub(crate) mod` while retaining explicit `pub use` for strictly public interfaces at the module root. This enforces encapsulation and presents a clean, un-nested API surface to consumers.

**[The Tangle: QueryStream's Blob structure and circular dependency]**
**Tangle:** `QueryStream` was located in `crates/force/src/api/rest/query_stream.rs`, importing `RestHandler` from `super`. `RestHandler` imported `QueryStream` from `query_stream.rs`. This tied the generic stream logic specifically to the REST handler, preventing other handlers (like `ToolingHandler`) from using it without importing `RestHandler`, breaking boundaries.
**Blueprint:** Extracted `QueryStream` out to `crates/force/src/api/query_stream.rs` and made it generic over `O: RestOperation<A> + Clone`. Re-exported it in `api::mod` and `api::rest::mod` updated its query logic to construct the decoupled, generic query stream.

**[The Facade: Enforcing Module Boundaries in force-sync and force-pubsub]**
**Tangle:** The `force-sync` and `force-pubsub` crates leaked internal submodules directly into the public API by declaring them as `pub mod` (e.g. `apply`, `capture`, `config`, `codec`, `error`). This violated the Facade pattern and exposed messy implementation details to consumers.
**Blueprint:** Refactored module visibility to `pub(crate) mod` across both crates and explicitly re-exported only the necessary public types (e.g., `SalesforceApplier`, `capture_batch`, `encode_avro`) via explicit `pub use` statements at the root `lib.rs`. Fixed integration tests to depend on the explicit public facade. Reduced nested submodule visibility conflicts to satisfy `clippy::redundant_pub_crate` by allowing `pub mod` exclusively inside already private structures.

**[RestOperation: Exposing query_stream]**
**Tangle:** The `query_stream` method was implemented directly on `RestHandler` in `api/rest/mod.rs`. This prevented other API handlers like `ToolingHandler` (which also implements `RestOperation`) from leveraging paginated streaming queries for SOQL, breaking cohesion and domain reuse.
**Blueprint:** Moved `query_stream` to be a provided method on the `RestOperation` trait. This required adding a `Self: Sized + Clone` bound to the method signature and updating all consumer test files to import the `RestOperation` trait so the method would be in scope.
**[Extracted describe types]**
**Tangle:** The `describe` schema types were located in `crates/force/src/types/describe.rs`, but the tests were in `crates/force/src/api/rest/describe.rs`, which also exported the types via `pub use crate::types::describe::*`. This created a "Hub and Spoke" issue where many domain modules (`schema`, `data`, `test_support`) imported these fundamental types through the intermediate `api::rest::describe` module, causing tight coupling to the REST API handler and unnecessary feature-gate dependencies.
**Blueprint:** Refactored the codebase by moving the `describe` tests from `api::rest::describe.rs` into `types/describe.rs` directly, removing the `api::rest::describe` module entirely. Updated all import statements across the `schema`, `data`, and `test_support` modules to import directly from `crate::types::describe`, enforcing clear domain boundaries and reducing coupling.
