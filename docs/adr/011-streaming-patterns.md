# 11. Streaming Patterns and Cleanup

Date: 2024-05-22

## Status

Accepted

## Context

The `pub_sub` module was introduced as a placeholder for the Salesforce Pub/Sub API, intending to use gRPC for streaming events. However, this implementation was never completed and remained as "Zombie Code".

Simultaneously, the need for efficient handling of large datasets has been addressed by:
1.  **SmartIngest (Bulk API 2.0):** A high-level utility for streaming large volumes of data into Salesforce.
2.  **QueryStream (REST API):** An async iterator pattern for paginating through large SOQL query results.

These patterns have proven effective and are now considered the standard for streaming operations in the SDK. The `experimental` module currently houses `SmartIngest`, which causes confusion about its stability.

## Decision

1.  **Remove `pub_sub`:** The `pub_sub` module and its associated dependencies (`tonic`, `prost`, `apache-avro`) will be removed. This eliminates dead code and reduces the build footprint.
2.  **Promote Streaming Patterns:** `SmartIngest` and `QueryStream` are recognized as the standard streaming patterns.
3.  **Deprecate `experimental`:** The `experimental` module is deprecated. `SmartIngest` is now canonically located in `crate::api::bulk::smart_ingest`.

## Consequences

*   **Cleanup:** The codebase is cleaner without the unimplemented `pub_sub` module.
*   **Build Size:** Removing `tonic`, `prost`, and `apache-avro` significantly reduces the dependency tree and build times for users not needing gRPC (which was everyone, as it was non-functional).
*   **Clarity:** Users have a clear path for streaming data (SmartIngest/QueryStream) without being misled by a placeholder module.
*   **Migration:** Any users strictly relying on the `pub_sub` feature flag (unlikely as it did nothing) will need to remove it.
