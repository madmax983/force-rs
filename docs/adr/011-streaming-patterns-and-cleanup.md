# 11. Streaming Patterns and Cleanup

Date: 2024-05-20
Status: Accepted

## Context

The `force` crate originally included a `pub_sub` feature intended to support the Salesforce Pub/Sub API via gRPC. This implementation has remained a placeholder ("zombie code") for an extended period.

However, enabling the `pub_sub` feature pulls in heavy dependencies:
- `tonic` (gRPC)
- `prost` (Protocol Buffers)
- `apache-avro` (Avro serialization)

These dependencies significantly increase compile times and binary sizes for users who likely aren't using the feature since it's incomplete.

At the same time, two other patterns for handling large datasets have matured within the codebase:
1. **SmartIngest** (`crate::api::bulk::smart_ingest`): A robust, stream-based implementation for the Bulk API 2.0 Ingest flow.
2. **QueryStream** (`crate::api::rest::QueryStream`): An async iterator pattern for paginated REST API queries.

## Decision

1. **Remove the `pub_sub` module**: We will remove the `crates/force/src/api/pub_sub.rs` module and the corresponding `pub_sub` feature flag.
2. **Remove Heavy Dependencies**: We will remove `tonic`, `prost`, and `apache-avro` from `Cargo.toml`.
3. **Promote Streaming Patterns**: We officially recognize `SmartIngest` and `QueryStream` as the standard patterns for high-volume data operations.

## Consequences

### Positive
- **Reduced Bloat**: Removing `tonic`, `prost`, and `avro` will significantly reduce the dependency tree and compile times.
- **Clarity**: Users will not be misled by a placeholder feature that doesn't work.
- **Focus**: The team can focus on maintaining the working REST and Bulk API implementations.

### Negative
- **Feature Gap**: Users who specifically need the Pub/Sub API will need to implement it themselves or wait for a future implementation.
- **Breaking Change**: This is a breaking change for any user who was enabling the `pub_sub` feature (even if they weren't using it effectively).

## Compliance

- `SmartIngest` must handle backpressure and memory limits (capped buffers).
- `QueryStream` must handle pagination seamlessly.
