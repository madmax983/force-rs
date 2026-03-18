# Pub/Sub API Design

**Date:** 2026-03-16
**Status:** Approved

## Summary

Add full Salesforce Pub/Sub API support to force-rs as a separate crate (`force-pubsub`). Covers all 5 RPCs: Subscribe, Publish, PublishStream, GetTopic, GetSchema. Uses Avro for event encoding with a schema cache, configurable reconnection policy, and shared auth via the existing `Session<A>` pattern.

## Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Crate placement | Separate `crates/force-pubsub` | ADR-011: gRPC deps (tonic/prost/protoc) shouldn't tax REST/Bulk users |
| API scope | All 5 RPCs from day one | Small surface, better to architect holistically |
| Avro strategy | Schema cache + dynamic decoding, typed opt-in | Mirrors Java/Python SDKs; code-gen breaks for org-specific CDC schemas |
| Reconnection | Configurable `ReconnectPolicy` (None / Auto) | Library shouldn't hardcode consumer's reliability requirements |
| Stream shape | `PubSubEvent<T>` enum (Event / Reconnected / KeepAlive) | Single stream, consumer pattern-matches on what they care about |
| Auth | Pre-fetch from existing `Session<A>` TokenManager | Shared auth across all API surfaces; avoids blocking in tonic interceptor |
| Testing | tonic in-process mock server | Natural for gRPC; wiremock doesn't apply to HTTP/2 frames |

## Crate Structure

```
crates/force-pubsub/
├── Cargo.toml
├── build.rs              # prost/tonic codegen from .proto
├── proto/
│   └── pubsub_api.proto  # Salesforce Pub/Sub API definition (vendored)
├── src/
│   ├── lib.rs
│   ├── handler.rs        # PubSubHandler<A> — entry point
│   ├── subscriber.rs     # Subscribe stream logic + reconnection
│   ├── publisher.rs      # Publish + PublishStream
│   ├── schema_cache.rs   # Avro schema fetch + cache
│   ├── codec.rs          # Avro encode/decode using apache-avro
│   ├── interceptor.rs    # Auth header injection (pre-fetch pattern)
│   ├── config.rs         # PubSubConfig, ReconnectPolicy, ReplayPreset
│   ├── types.rs          # PubSubEvent<T>, EventMessage<T>, ReplayId, etc.
│   └── error.rs          # PubSubError
└── tests/
    └── ...
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `force` | `Session<A>`, auth, config, shared error types |
| `tonic` | gRPC client |
| `prost` | Protobuf codegen |
| `apache-avro` | Avro encode/decode |
| `tokio-stream` | Stream combinators for subscribe |
| `dashmap` | Lock-free concurrent schema cache |

## Core Types

### Configuration

```rust
pub struct PubSubConfig {
    /// gRPC endpoint (default: api.pubsub.salesforce.com:7443)
    pub endpoint: String,
    /// Number of events per FetchRequest batch (max 100)
    pub batch_size: i32,
    /// Reconnection behavior
    pub reconnect_policy: ReconnectPolicy,
}

pub enum ReconnectPolicy {
    None,
    Auto { max_retries: u32, backoff: BackoffConfig },
}

pub struct BackoffConfig {
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub multiplier: f64,
}

pub enum ReplayPreset {
    Latest,
    Earliest,
    Custom(ReplayId),
}
```

### Event Types

```rust
/// Opaque replay cursor for resuming subscriptions
pub struct ReplayId(Vec<u8>);

/// What the consumer receives from the subscribe stream
pub enum PubSubEvent<T> {
    Event(EventMessage<T>),
    Reconnected { replay_id: ReplayId, attempt: u32 },
    KeepAlive,
}

/// A single decoded event with metadata
pub struct EventMessage<T> {
    pub payload: T,
    pub replay_id: ReplayId,
    pub schema_id: String,
    pub event_id: String,
}
```

### Errors

```rust
pub enum PubSubError {
    Transport(tonic::Status),
    Avro(apache_avro::Error),
    SchemaNotFound { schema_id: String },
    Auth(force::error::ForceError),
    ReconnectFailed { attempts: u32, last_error: Box<PubSubError> },
}
```

## Handler API Surface

```rust
pub struct PubSubHandler<A: Authenticator> {
    session: Arc<Session<A>>,
    config: PubSubConfig,
    schema_cache: SchemaCache,
    channel: tonic::transport::Channel,
}

impl<A: Authenticator> PubSubHandler<A> {
    /// Connect to the Pub/Sub API endpoint
    pub async fn connect(
        session: Arc<Session<A>>,
        config: PubSubConfig,
    ) -> Result<Self, PubSubError>;

    // --- Metadata RPCs ---

    pub async fn get_topic(&self, topic_name: &str)
        -> Result<TopicInfo, PubSubError>;

    pub async fn get_schema(&self, schema_id: &str)
        -> Result<SchemaInfo, PubSubError>;

    // --- Subscribe ---

    /// Dynamic subscribe — payloads as serde_json::Value
    pub async fn subscribe(
        &self, topic: &str, replay: ReplayPreset,
    ) -> Result<impl Stream<Item = Result<PubSubEvent<Value>, PubSubError>>, PubSubError>;

    /// Typed subscribe — payloads deserialized to T
    pub async fn subscribe_typed<T: DeserializeOwned>(
        &self, topic: &str, replay: ReplayPreset,
    ) -> Result<impl Stream<Item = Result<PubSubEvent<T>, PubSubError>>, PubSubError>;

    // --- Publish ---

    /// Publish events to a topic (unary)
    pub async fn publish<T: Serialize>(
        &self, topic: &str, events: Vec<T>,
    ) -> Result<PublishResponse, PubSubError>;

    /// Streaming publish — bidirectional
    pub async fn publish_stream<T: Serialize>(
        &self, topic: &str,
    ) -> Result<PublishSink<T>, PubSubError>;
}
```

## Auth Integration

The gRPC endpoint requires three metadata headers: `accesstoken`, `instanceurl`, `tenantid`.

Rather than implementing tonic's synchronous `Interceptor` trait (which conflicts with async `TokenManager::get_token()`), we use a **pre-fetch pattern**: each handler method calls `session.token_manager.get_token()` before invoking the gRPC stub, then manually injects the metadata.

The `tenantid` (org ID) is fetched lazily from `/services/oauth2/userinfo` on first use and cached via `tokio::sync::OnceCell`.

## Subscribe Flow

1. `handler.subscribe("topic", ReplayPreset::Latest)` builds internal `SubscriptionState` and opens bidirectional stream
2. Client sends `FetchRequest` with batch size and replay preset
3. Server streams `FetchResponse` — each contains 0+ events and `latest_replay_id`
4. For each response:
   - Events present → decode Avro via `SchemaCache`, yield `PubSubEvent::Event` per event
   - No events → yield `PubSubEvent::KeepAlive`
   - Update `last_replay_id`
5. Client sends another `FetchRequest` to maintain flow control

### Reconnection (when `ReconnectPolicy::Auto`)

1. gRPC stream errors → check retry budget
2. Backoff with jitter → reconnect with `ReplayPreset::Custom(last_replay_id)`
3. Yield `PubSubEvent::Reconnected { replay_id, attempt }`
4. Resume normal event flow; reset `reconnect_count` on first successful event
5. Retries exhausted → yield `Err(PubSubError::ReconnectFailed)` and terminate

### No Reconnection (when `ReconnectPolicy::None`)

- Stream error → yield `Err(PubSubError::Transport)` and terminate

## Publish Flow

### Unary Publish

1. Caller passes `Vec<T>` where `T: Serialize`
2. Handler fetches topic schema via `SchemaCache`
3. Encodes each event to Avro binary
4. Sends single `PublishRequest`
5. Returns `PublishResponse` with per-event success/failure

### Streaming Publish

```rust
pub struct PublishSink<T> {
    sender: mpsc::Sender<PublishRequest>,
    responses: Pin<Box<dyn Stream<Item = Result<PublishResponse, PubSubError>>>>,
}

impl<T: Serialize> PublishSink<T> {
    pub async fn send(&self, events: Vec<T>) -> Result<(), PubSubError>;
    pub fn responses(&mut self) -> impl Stream<Item = Result<PublishResponse, PubSubError>>;
    pub async fn close(self) -> Result<(), PubSubError>;
}
```

## Schema Cache

```rust
pub struct SchemaCache {
    cache: DashMap<String, apache_avro::Schema>,
    client: PubSubClient,
}

impl SchemaCache {
    /// Get or fetch a schema. Cache hit is lock-free via DashMap.
    pub async fn get_or_fetch(&self, schema_id: &str)
        -> Result<apache_avro::Schema, PubSubError>;
}
```

## Testing Strategy

| Layer | Approach |
|-------|----------|
| Avro codec | Unit tests — encode/decode roundtrips, edge cases |
| Schema cache | Unit tests — cache hit/miss, concurrent access |
| Config | Unit tests — validation, defaults |
| Subscribe flow | Mock gRPC server (tonic in-process) with canned responses |
| Publish flow | Mock gRPC server with request validation |
| Reconnection | Mock server drops connection after N events, verify resume with correct replay ID |
| Integration | `#[ignore]` tests against real Salesforce org (same pattern as existing live tests) |
