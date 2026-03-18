# ADR-018: Implement Pub/Sub as a Separate Workspace Crate

**Status:** Accepted
**Date:** 2026-03-17
**Deciders:** Mark
**Supersedes:** [ADR-011](011-remove-pub-sub-support.md)

## Context

[ADR-011](011-remove-pub-sub-support.md) removed Pub/Sub support from the `force` crate because
gRPC dependencies (`tonic`, `prost`) and the `protoc` code-generation pipeline add meaningful
compile-time overhead and build complexity for users who only need the REST or Bulk APIs. That
decision was correct for the core crate.

However, the Salesforce Pub/Sub API is a first-class integration surface. Platform Events,
Change Data Capture, and high-volume event streams are central to modern Salesforce architectures.
A production-grade Rust ecosystem around Salesforce must offer this capability.

The solution foreshadowed in ADR-011 ("may be re-introduced later as a separate crate, e.g.,
`force-pubsub`") is now being implemented.

## Decision

Create `crates/force-pubsub` as a standalone crate in the `force-rs` workspace.

The crate:

- **Depends on `force`** for `Session<A>`, `Authenticator`, and token management — shared auth
  logic is not duplicated.
- **Uses `tonic 0.12`** for the gRPC transport layer.
- **Uses `prost 0.13`** for protobuf message encoding/decoding (generated via `build.rs` with
  `protoc-bin-vendored` to avoid requiring a system `protoc` installation).
- **Uses `apache-avro 0.17`** for Avro-encoding Platform Event payloads (Salesforce Pub/Sub uses
  Avro for event serialization).
- **Uses `dashmap 6`** for a lock-free, `Clone`-friendly schema cache that avoids `Mutex`
  contention when multiple subscribe streams resolve schemas concurrently.

### API Surface (5 RPCs)

| RPC | Method | Description |
|-----|--------|-------------|
| `GetTopic` | `PubSubHandler::get_topic` | Fetch topic metadata |
| `GetSchema` | `PubSubHandler::get_schema` | Fetch Avro schema by ID |
| `Subscribe` | `PubSubHandler::subscribe` / `subscribe_typed` | Bidirectional streaming event consumption |
| `Publish` | `PubSubHandler::publish` | Unary publish of Avro-encoded events |
| `PublishStream` | `PubSubHandler::publish_stream` → `PublishSink<T>` | Streaming publish for high-throughput scenarios |

`PublishSink<T>` is the concrete type returned by `publish_stream`. It wraps the bidirectional
`PublishStream` gRPC channel and exposes three methods: `send(schema_id, events)` encodes a batch
of `T: Serialize` events to Avro and writes them to the open stream; `responses()` returns a
reference to the server acknowledgement stream; and `close()` drops the sender and drains any
remaining acknowledgements so the gRPC stream shuts down cleanly.

### Key Design Choices

**`PubSubHandler<A>` is cheaply cloneable.** It wraps `Arc<Session<A>>`, a `Channel` (already
cheaply cloneable in tonic), and a `SchemaCache` (backed by `DashMap`, also `Arc`-wrapped).
Clones share state; there is no per-clone connection overhead.

**Reconnection policy is configurable.** `ReconnectPolicy::Auto` drives exponential backoff with
configurable `max_retries`, `initial_delay`, `max_delay`, and `multiplier`. The subscriber
automatically resumes from the last seen `ReplayId` after a stream drop, making transient
disconnects transparent to application code.

**Schema caching is lazy via `SchemaCache::get_or_fetch`.** On a cache miss, `get_or_fetch`
issues a `GetSchema` RPC, parses the returned Avro schema JSON, stores the result in the
`DashMap`, and returns the parsed schema — all in one call. Subsequent calls with the same
`schema_id` return the cached entry without any gRPC round-trip. The cache is shared across all
subscribe streams and `PublishSink` instances that originate from the same `PubSubHandler`.

**`ReplayPreset` controls cursor placement.** Consumers choose `Latest` (events after subscribe
time), `Earliest` (72-hour retention window), or `Custom(ReplayId)` (resume from a checkpoint).

**Auth headers are centralised in the `interceptor` module.** Every RPC call — `Subscribe`,
`Publish`, `PublishStream`, `GetSchema`, and `GetTopic` — requires three gRPC metadata headers:
`accesstoken`, `instanceurl`, and `tenantid` (the 18-char org ID). The `interceptor::build_metadata`
function constructs this `MetadataMap` in one place; `handler`, `subscriber`, and `publisher`
all call it, ensuring consistent header injection with no duplication.

## Consequences

### Positive

- **Zero dependency overhead for REST/Bulk users.** Users who do not add `force-pubsub` to their
  `Cargo.toml` do not pay any compile-time or binary-size cost for gRPC or protobuf.
- **Clean separation of gRPC surface.** The `force` crate stays focused on HTTP-based APIs;
  `force-pubsub` owns the gRPC surface.
- **Shared authentication via `Session<A>`.** Token refresh, credential management, and the
  `Authenticator` trait are shared; no duplication.
- **Full 5-RPC API surface.** All Salesforce Pub/Sub RPCs are covered.
- **Configurable `ReconnectPolicy`.** Production workloads get automatic reconnection with
  exponential backoff out of the box.
- **Avro schema cache with `DashMap`.** Lock-free concurrent schema lookups scale with subscribe
  concurrency.

### Negative

- **Second crate dependency.** Users who need Pub/Sub must add `force-pubsub` explicitly. This is
  a deliberate trade-off; see ADR-011 for rationale.
- **Protobuf codegen in `build.rs`.** Requires `protoc-bin-vendored`, which pulls in a pre-built
  `protoc` binary (~10 MB). This is acceptable in `force-pubsub` because opting in to the crate
  implies acceptance of this overhead.
- **`tonic` version coupling.** The crate targets `tonic 0.12`; upgrading tonic will require a
  coordinated update of generated stubs and the `build.rs` pipeline.

## Supersedes

[ADR-011: Remove Pub/Sub Support](011-remove-pub-sub-support.md) — that ADR removed Pub/Sub from
`force` and left a placeholder note about a future separate crate. This ADR fulfils that note.
