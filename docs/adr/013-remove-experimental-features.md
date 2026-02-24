# 13. Remove Experimental & Nova Features

Date: 2024-05-23

## Status

Accepted

## Context

The `force` crate included an `experimental` module and a `nova` feature flag to test new capabilities before stabilizing them.

The `experimental` module contained:
- `FieldUsageScanner`: A utility to analyze field population usage.
- `QueryBatch`: A utility to batch operations based on query results.

The `nova` feature provided:
- `api::rest::explain`: Access to the Salesforce Query Plan API (`/query/?explain=`) via `RestHandler::explain`.

These features were prototypes. `SmartIngest` (formerly experimental) was successfully promoted to `api::bulk::smart_ingest`, but the remaining experimental features have not been prioritized for stabilization. Retaining them creates maintenance overhead, increases compilation time (slightly), and confuses the API surface with unsupported utilities.

## Decision

We will remove the `experimental` module and the `nova` feature entirely from the crate.

Specifically:
1.  Delete `crates/force/src/experimental`.
2.  Remove `pub mod experimental` from the library root.
3.  Remove the `nova` feature flag from `Cargo.toml`.
4.  Remove `crates/force/src/api/rest/explain.rs` and the corresponding `explain` method in `RestHandler`.

## Consequences

-   **Positive:** The codebase is cleaner and focuses on the core, production-ready APIs (REST, Bulk, Composite).
-   **Positive:** There is no ambiguity about which features are supported.
-   **Negative:** Users relying on `FieldUsageScanner`, `QueryBatch`, or `explain()` will need to implement these utilities in their own code or use the raw `execute_get` / `execute_post` methods (which remain available for custom endpoints).
