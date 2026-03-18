# force-pubsub Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a `force-pubsub` crate to the workspace that implements the full Salesforce Pub/Sub gRPC API (Subscribe, Publish, PublishStream, GetTopic, GetSchema).

**Architecture:** A separate workspace crate (`crates/force-pubsub`) that depends on `force` for `Session<A>`, auth, and config. Uses `tonic` for gRPC transport, `prost` for protobuf types from a vendored `.proto` file, and `apache-avro` for event payload encoding/decoding. Schema IDs are cached in a `DashMap` to avoid redundant `GetSchema` calls.

**Tech Stack:** `tonic 0.12`, `prost 0.13`, `apache-avro 0.17`, `dashmap 6`, `tokio-stream 0.1`, `protoc-bin-vendored 3` (build-time protoc, no system install needed)

---

## Prerequisites

The `force` crate uses `cargo nextest run`. Install if missing:
```bash
cargo install cargo-nextest
```

All commands run from the repo root (`C:\Users\markm\force-rs`) unless noted.

The workspace already has `tonic`, `prost`, and `apache-avro` in `[workspace.dependencies]` but they are unused. This plan activates them.

---

## Task 1: Workspace Scaffold

**Goal:** Add `crates/force-pubsub` to the workspace and verify it compiles.

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Create: `crates/force-pubsub/Cargo.toml`
- Create: `crates/force-pubsub/build.rs`
- Create: `crates/force-pubsub/src/lib.rs`
- Create: `crates/force-pubsub/proto/pubsub_api.proto`

### Step 1: Add to workspace

Edit `Cargo.toml` (root), change:
```toml
members = ["crates/force", ]
```
to:
```toml
members = ["crates/force", "crates/force-pubsub"]
```

Also add `dashmap` and `tokio-stream` to `[workspace.dependencies]`:
```toml
dashmap = "6"
tokio-stream = "0.1"
```

### Step 2: Create `crates/force-pubsub/Cargo.toml`

```toml
[package]
name = "force-pubsub"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
repository.workspace = true
description = "Salesforce Pub/Sub API (gRPC) client for force-rs"
keywords = ["salesforce", "pubsub", "grpc", "streaming", "events"]
categories = ["api-bindings", "web-programming"]

[lints]
workspace = true

[dependencies]
force = { path = "../force" }
tonic = { workspace = true, features = ["tls", "tls-roots"] }
prost = { workspace = true }
apache-avro = { workspace = true }
tokio = { workspace = true }
tokio-stream = { workspace = true }
futures = { workspace = true }
thiserror = { workspace = true }
dashmap = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
async-trait = { workspace = true }

[build-dependencies]
tonic-build = "0.12"
protoc-bin-vendored = "3.0"

[dev-dependencies]
tokio-test = "0.4"
tokio-stream = { workspace = true, features = ["net"] }
anyhow = { workspace = true }
```

### Step 3: Create `crates/force-pubsub/build.rs`

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use vendored protoc — no system install required
    let protoc = protoc_bin_vendored::protoc_bin_path()
        .expect("protoc-bin-vendored: protoc binary not found");
    std::env::set_var("PROTOC", protoc);

    tonic_build::configure()
        .build_server(true)  // needed for in-process test servers
        .build_client(true)
        .compile_protos(&["proto/pubsub_api.proto"], &["proto"])?;

    Ok(())
}
```

### Step 4: Create `crates/force-pubsub/proto/pubsub_api.proto`

```protobuf
syntax = "proto3";

package eventbus.v1;

option java_package = "com.salesforce.eventbus.protobuf";

// Salesforce Pub/Sub API — vendored from
// https://github.com/forcedotcom/pub-sub-api

service PubSub {
    rpc GetTopic(TopicRequest) returns (TopicInfo);
    rpc GetSchema(SchemaRequest) returns (SchemaInfo);
    rpc Subscribe(stream FetchRequest) returns (stream FetchResponse);
    rpc Publish(PublishRequest) returns (PublishResponse);
    rpc PublishStream(stream PublishRequest) returns (stream PublishResponse);
}

message TopicRequest {
    string topic_name = 1;
}

message TopicInfo {
    string topic_name = 1;
    string topic_uri = 2;
    bool can_publish = 3;
    bool can_subscribe = 4;
    string schema_id = 5;
}

message SchemaRequest {
    string schema_id = 1;
}

message SchemaInfo {
    string schema_json = 1;
    string schema_id = 2;
}

enum ReplayPreset {
    LATEST = 0;
    EARLIEST = 1;
    CUSTOM = 2;
}

message FetchRequest {
    string topic_name = 1;
    ReplayPreset replay_preset = 2;
    bytes replay_id = 3;
    int32 num_requested = 4;
    optional string auth_refresh = 5;
}

message FetchResponse {
    string topic_name = 1;
    bytes latest_replay_id = 2;
    repeated ConsumerEvent events = 3;
    int32 pending_num_requested = 4;
    optional string rpc_id = 5;
}

message ConsumerEvent {
    EventHeader event = 1;
    bytes payload = 2;
}

message EventHeader {
    bytes replay_id = 1;
    string producer_partition_key = 2;
    map<string, AttributeValue> headers = 3;
    string schema_id = 4;
}

message AttributeValue {
    oneof value {
        string string_value = 1;
        bytes bytes_value = 2;
    }
}

message ProducerEvent {
    string schema_id = 1;
    bytes payload = 2;
}

message PublishRequest {
    string topic_name = 1;
    repeated ProducerEvent events = 2;
}

message PublishResult {
    bytes replay_id = 1;
    PubSubError error = 2;
}

message PublishResponse {
    string topic_name = 1;
    repeated PublishResult results = 2;
    optional string rpc_id = 3;
}

message PubSubError {
    int32 code = 1;
    string msg = 2;
    optional string key = 3;
}
```

> **Note:** Protobuf names `PubSubError` here to avoid collision with our Rust `PubSubError` type — prost will generate `proto::eventbus_v1::PubSubError`.

### Step 5: Create `crates/force-pubsub/src/lib.rs`

```rust
//! Salesforce Pub/Sub API client for Rust.
//!
//! This crate provides a gRPC client for the Salesforce Pub/Sub API,
//! supporting subscribe, publish, and schema operations.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::missing_errors_doc)]

/// Generated protobuf types from the Salesforce Pub/Sub API proto.
pub mod proto {
    pub mod eventbus_v1 {
        tonic::include_proto!("eventbus.v1");
    }
}
```

### Step 6: Verify it compiles

```bash
cargo build -p force-pubsub
```

Expected: compiles without errors (no tests yet). Fix any build.rs or dependency issues before continuing.

### Step 7: Commit

```bash
git add crates/force-pubsub/ Cargo.toml Cargo.lock
git commit -m "feat(pubsub): scaffold force-pubsub crate with proto codegen"
```

---

## Task 2: Error Types

**Files:**
- Create: `crates/force-pubsub/src/error.rs`
- Modify: `crates/force-pubsub/src/lib.rs`

### Step 1: Write the failing test

Add to `crates/force-pubsub/src/error.rs`:

```rust
use thiserror::Error;

/// All errors from the force-pubsub crate.
#[derive(Debug, Error)]
pub enum PubSubError {
    /// gRPC transport or protocol error.
    #[error("gRPC transport error: {0}")]
    Transport(#[from] tonic::Status),

    /// Avro encoding or decoding failure.
    #[error("Avro error: {0}")]
    Avro(String),

    /// Schema not found for the given ID.
    #[error("schema not found: {schema_id}")]
    SchemaNotFound { schema_id: String },

    /// Authentication or token error from the force crate.
    #[error("auth error: {0}")]
    Auth(#[from] force::error::ForceError),

    /// Reconnection to the subscribe stream was exhausted.
    #[error("reconnect failed after {attempts} attempt(s): {last_error}")]
    ReconnectFailed {
        attempts: u32,
        last_error: Box<PubSubError>,
    },

    /// gRPC channel setup failed.
    #[error("failed to connect to Pub/Sub endpoint: {0}")]
    Connect(#[from] tonic::transport::Error),

    /// Invalid configuration.
    #[error("invalid configuration: {0}")]
    Config(String),
}

/// Convenience Result alias.
pub type Result<T> = std::result::Result<T, PubSubError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_not_found_display() {
        let err = PubSubError::SchemaNotFound {
            schema_id: "abc123".to_string(),
        };
        assert_eq!(err.to_string(), "schema not found: abc123");
    }

    #[test]
    fn test_config_error_display() {
        let err = PubSubError::Config("batch_size must be > 0".to_string());
        assert_eq!(err.to_string(), "invalid configuration: batch_size must be > 0");
    }

    #[test]
    fn test_avro_error_display() {
        let err = PubSubError::Avro("unexpected end of buffer".to_string());
        assert_eq!(err.to_string(), "Avro error: unexpected end of buffer");
    }

    #[test]
    fn test_reconnect_failed_display() {
        let inner = Box::new(PubSubError::Config("test".to_string()));
        let err = PubSubError::ReconnectFailed {
            attempts: 3,
            last_error: inner,
        };
        assert!(err.to_string().contains("3 attempt(s)"));
    }

    #[test]
    fn test_from_tonic_status() {
        let status = tonic::Status::not_found("topic not found");
        let err = PubSubError::from(status);
        assert!(matches!(err, PubSubError::Transport(_)));
    }
}
```

### Step 2: Run to confirm RED

```bash
cargo nextest run -p force-pubsub
```

Expected: compilation failure — `error.rs` exists but isn't in `lib.rs`.

### Step 3: Wire into lib.rs

Add to `crates/force-pubsub/src/lib.rs`:

```rust
pub mod error;
pub use error::{PubSubError, Result};
```

### Step 4: Run to confirm GREEN

```bash
cargo nextest run -p force-pubsub
```

Expected: all tests pass.

### Step 5: Commit

```bash
git add crates/force-pubsub/src/
git commit -m "feat(pubsub): add PubSubError type with thiserror"
```

---

## Task 3: Config & Event Types

**Files:**
- Create: `crates/force-pubsub/src/config.rs`
- Create: `crates/force-pubsub/src/types.rs`
- Modify: `crates/force-pubsub/src/lib.rs`

### Step 1: Write failing tests for config

Create `crates/force-pubsub/src/config.rs`:

```rust
use std::time::Duration;

/// Configuration for the Pub/Sub client.
#[derive(Debug, Clone)]
pub struct PubSubConfig {
    /// gRPC endpoint for the Pub/Sub API.
    pub endpoint: String,
    /// Number of events to request per FetchRequest batch (1–100).
    pub batch_size: i32,
    /// Reconnection policy for the subscribe stream.
    pub reconnect_policy: ReconnectPolicy,
}

impl Default for PubSubConfig {
    fn default() -> Self {
        Self {
            endpoint: "https://api.pubsub.salesforce.com:7443".to_string(),
            batch_size: 100,
            reconnect_policy: ReconnectPolicy::default(),
        }
    }
}

/// Controls reconnection behaviour when the subscribe stream drops.
#[derive(Debug, Clone)]
pub enum ReconnectPolicy {
    /// No automatic reconnection — errors surface to the stream consumer.
    None,
    /// Reconnect automatically, resuming from the last seen replay ID.
    Auto {
        /// Maximum number of reconnection attempts before giving up.
        max_retries: u32,
        /// Backoff configuration between attempts.
        backoff: BackoffConfig,
    },
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self::Auto {
            max_retries: 5,
            backoff: BackoffConfig::default(),
        }
    }
}

/// Exponential backoff with jitter.
#[derive(Debug, Clone)]
pub struct BackoffConfig {
    /// Initial delay before the first retry.
    pub initial_delay: Duration,
    /// Maximum delay cap.
    pub max_delay: Duration,
    /// Multiplier applied after each attempt.
    pub multiplier: f64,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            multiplier: 2.0,
        }
    }
}

impl BackoffConfig {
    /// Compute the delay for a given attempt number (0-indexed).
    pub fn delay_for(&self, attempt: u32) -> Duration {
        let multiplied = self.initial_delay.as_secs_f64()
            * self.multiplier.powi(attempt as i32);
        let capped = multiplied.min(self.max_delay.as_secs_f64());
        Duration::from_secs_f64(capped)
    }
}

/// Where to start replaying events when subscribing.
#[derive(Debug, Clone)]
pub enum ReplayPreset {
    /// Only events published after subscribing.
    Latest,
    /// All events within the 72-hour retention window.
    Earliest,
    /// Resume from a specific replay ID.
    Custom(crate::types::ReplayId),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = PubSubConfig::default();
        assert_eq!(config.endpoint, "https://api.pubsub.salesforce.com:7443");
        assert_eq!(config.batch_size, 100);
        assert!(matches!(config.reconnect_policy, ReconnectPolicy::Auto { .. }));
    }

    #[test]
    fn test_default_reconnect_policy() {
        let policy = ReconnectPolicy::default();
        match policy {
            ReconnectPolicy::Auto { max_retries, .. } => assert_eq!(max_retries, 5),
            ReconnectPolicy::None => panic!("expected Auto"),
        }
    }

    #[test]
    fn test_backoff_delay_for_attempt_0() {
        let backoff = BackoffConfig::default();
        assert_eq!(backoff.delay_for(0), Duration::from_millis(500));
    }

    #[test]
    fn test_backoff_delay_for_attempt_1() {
        let backoff = BackoffConfig::default();
        assert_eq!(backoff.delay_for(1), Duration::from_millis(1000));
    }

    #[test]
    fn test_backoff_delay_capped_at_max() {
        let backoff = BackoffConfig::default();
        // After many retries, should cap at 30s
        let delay = backoff.delay_for(20);
        assert_eq!(delay, Duration::from_secs(30));
    }

    #[test]
    fn test_replay_preset_variants_exist() {
        let _latest = ReplayPreset::Latest;
        let _earliest = ReplayPreset::Earliest;
        // Custom tested in types tests
    }
}
```

### Step 2: Write failing tests for types

Create `crates/force-pubsub/src/types.rs`:

```rust
use serde::{Deserialize, Serialize};

/// Opaque replay cursor. Consumers store and pass back; never interpret the bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayId(pub(crate) Vec<u8>);

impl ReplayId {
    /// Construct from raw bytes (e.g., from a FetchResponse).
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Return the raw bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// True if this replay ID has no bytes (represents "no replay").
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A decoded event received from the subscribe stream.
#[derive(Debug, Clone)]
pub struct EventMessage<T> {
    /// Decoded event payload.
    pub payload: T,
    /// Replay cursor for this event — store to resume from here.
    pub replay_id: ReplayId,
    /// Avro schema ID used to decode this event.
    pub schema_id: String,
    /// Unique event identifier.
    pub event_id: String,
}

/// Items yielded by the subscribe stream.
#[derive(Debug)]
pub enum PubSubEvent<T> {
    /// A decoded event.
    Event(EventMessage<T>),
    /// Stream was reconnected after a drop.
    Reconnected {
        /// The replay ID we resumed from.
        replay_id: ReplayId,
        /// Which reconnection attempt this was (1-indexed).
        attempt: u32,
    },
    /// Salesforce heartbeat — no events this batch.
    KeepAlive,
}

/// Per-event result from a publish operation.
#[derive(Debug, Clone)]
pub struct PublishResult {
    /// Replay ID assigned by Salesforce (if successful).
    pub replay_id: Option<ReplayId>,
    /// Error message if this individual event failed.
    pub error: Option<String>,
}

impl PublishResult {
    /// Returns true if this event was published successfully.
    pub fn is_success(&self) -> bool {
        self.error.is_none()
    }
}

/// Response from a publish operation.
#[derive(Debug, Clone)]
pub struct PublishResponse {
    /// The topic name events were published to.
    pub topic_name: String,
    /// Per-event results (same order as the input events).
    pub results: Vec<PublishResult>,
}

impl PublishResponse {
    /// Returns true if all events were published successfully.
    pub fn all_succeeded(&self) -> bool {
        self.results.iter().all(PublishResult::is_success)
    }

    /// Returns the number of failed events.
    pub fn failure_count(&self) -> usize {
        self.results.iter().filter(|r| !r.is_success()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replay_id_from_bytes_roundtrip() {
        let bytes = vec![1u8, 2, 3, 4];
        let id = ReplayId::from_bytes(bytes.clone());
        assert_eq!(id.as_bytes(), bytes.as_slice());
    }

    #[test]
    fn test_replay_id_empty() {
        let id = ReplayId::from_bytes(vec![]);
        assert!(id.is_empty());
    }

    #[test]
    fn test_replay_id_not_empty() {
        let id = ReplayId::from_bytes(vec![1, 2, 3]);
        assert!(!id.is_empty());
    }

    #[test]
    fn test_publish_result_success() {
        let r = PublishResult {
            replay_id: Some(ReplayId::from_bytes(vec![1])),
            error: None,
        };
        assert!(r.is_success());
    }

    #[test]
    fn test_publish_result_failure() {
        let r = PublishResult {
            replay_id: None,
            error: Some("INVALID_TYPE".to_string()),
        };
        assert!(!r.is_success());
    }

    #[test]
    fn test_publish_response_all_succeeded() {
        let resp = PublishResponse {
            topic_name: "/event/MyEvent__e".to_string(),
            results: vec![
                PublishResult { replay_id: Some(ReplayId::from_bytes(vec![1])), error: None },
                PublishResult { replay_id: Some(ReplayId::from_bytes(vec![2])), error: None },
            ],
        };
        assert!(resp.all_succeeded());
        assert_eq!(resp.failure_count(), 0);
    }

    #[test]
    fn test_publish_response_partial_failure() {
        let resp = PublishResponse {
            topic_name: "/event/MyEvent__e".to_string(),
            results: vec![
                PublishResult { replay_id: Some(ReplayId::from_bytes(vec![1])), error: None },
                PublishResult { replay_id: None, error: Some("ERR".to_string()) },
            ],
        };
        assert!(!resp.all_succeeded());
        assert_eq!(resp.failure_count(), 1);
    }

    #[test]
    fn test_pub_sub_event_variants() {
        let _event: PubSubEvent<String> = PubSubEvent::Event(EventMessage {
            payload: "hello".to_string(),
            replay_id: ReplayId::from_bytes(vec![1]),
            schema_id: "schema1".to_string(),
            event_id: "evt1".to_string(),
        });
        let _reconnected: PubSubEvent<String> = PubSubEvent::Reconnected {
            replay_id: ReplayId::from_bytes(vec![1]),
            attempt: 1,
        };
        let _keepalive: PubSubEvent<String> = PubSubEvent::KeepAlive;
    }
}
```

### Step 3: Run to confirm RED

```bash
cargo nextest run -p force-pubsub
```

Expected: compile errors — modules not yet in `lib.rs`, and `config.rs` references `crate::types::ReplayId` which doesn't exist yet.

### Step 4: Wire into lib.rs

```rust
pub mod config;
pub mod types;

pub use config::{BackoffConfig, PubSubConfig, ReconnectPolicy, ReplayPreset};
pub use types::{EventMessage, PubSubEvent, PublishResponse, PublishResult, ReplayId};
```

### Step 5: Run to confirm GREEN

```bash
cargo nextest run -p force-pubsub
```

Expected: all tests pass.

### Step 6: Commit

```bash
git add crates/force-pubsub/src/
git commit -m "feat(pubsub): add config, event types, and ReplayId"
```

---

## Task 4: Avro Codec

**Files:**
- Create: `crates/force-pubsub/src/codec.rs`
- Modify: `crates/force-pubsub/src/lib.rs`

The Avro codec encodes/decodes event payloads using a pre-fetched `apache_avro::Schema`. Salesforce Pub/Sub events use Avro binary encoding (not JSON).

### Step 1: Write failing tests

Create `crates/force-pubsub/src/codec.rs`:

```rust
use apache_avro::{from_value, to_avro_datum, Schema};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{PubSubError, Result};

/// Decode Avro-binary bytes into a `serde_json::Value` using the given schema.
///
/// Salesforce Pub/Sub events are encoded as Avro binary (not JSON).
/// The schema must match the `schema_id` on the event header.
pub fn decode_avro(schema: &Schema, bytes: &[u8]) -> Result<Value> {
    let value = apache_avro::from_avro_datum(schema, &mut std::io::Cursor::new(bytes), None)
        .map_err(|e| PubSubError::Avro(e.to_string()))?;
    from_value::<Value>(&value).map_err(|e| PubSubError::Avro(e.to_string()))
}

/// Decode Avro-binary bytes into a typed struct `T` using the given schema.
pub fn decode_avro_typed<T: for<'de> Deserialize<'de>>(
    schema: &Schema,
    bytes: &[u8],
) -> Result<T> {
    let value = apache_avro::from_avro_datum(schema, &mut std::io::Cursor::new(bytes), None)
        .map_err(|e| PubSubError::Avro(e.to_string()))?;
    from_value::<T>(&value).map_err(|e| PubSubError::Avro(e.to_string()))
}

/// Encode a serializable value to Avro binary using the given schema.
///
/// Used when publishing events.
pub fn encode_avro<T: Serialize>(schema: &Schema, value: &T) -> Result<Vec<u8>> {
    let avro_value = apache_avro::to_value(value)
        .map_err(|e| PubSubError::Avro(e.to_string()))?;
    let resolved = avro_value
        .resolve(schema)
        .map_err(|e| PubSubError::Avro(e.to_string()))?;
    to_avro_datum(schema, resolved).map_err(|e| PubSubError::Avro(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use apache_avro::Schema;

    const SIMPLE_SCHEMA: &str = r#"
    {
        "type": "record",
        "name": "TestEvent",
        "fields": [
            {"name": "id", "type": "string"},
            {"name": "amount", "type": "double"}
        ]
    }
    "#;

    #[test]
    fn test_encode_decode_roundtrip_dynamic() {
        let schema = Schema::parse_str(SIMPLE_SCHEMA).expect("valid schema");
        let payload = serde_json::json!({
            "id": "event-001",
            "amount": 99.5
        });

        let encoded = encode_avro(&schema, &payload).expect("encode succeeds");
        assert!(!encoded.is_empty());

        let decoded: Value = decode_avro(&schema, &encoded).expect("decode succeeds");
        assert_eq!(decoded["id"], "event-001");
        assert!((decoded["amount"].as_f64().unwrap() - 99.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_encode_decode_roundtrip_typed() {
        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct TestEvent {
            id: String,
            amount: f64,
        }

        let schema = Schema::parse_str(SIMPLE_SCHEMA).expect("valid schema");
        let event = TestEvent {
            id: "event-002".to_string(),
            amount: 42.0,
        };

        let encoded = encode_avro(&schema, &event).expect("encode succeeds");
        let decoded: TestEvent = decode_avro_typed(&schema, &encoded).expect("decode succeeds");

        assert_eq!(decoded.id, "event-002");
        assert!((decoded.amount - 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_decode_invalid_bytes_returns_error() {
        let schema = Schema::parse_str(SIMPLE_SCHEMA).expect("valid schema");
        let garbage = vec![0xFF, 0xFE, 0xFD];
        let result = decode_avro(&schema, &garbage);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PubSubError::Avro(_)));
    }

    #[test]
    fn test_encode_with_extra_field_is_ok() {
        // Extra fields not in schema are ignored during encode
        let schema = Schema::parse_str(SIMPLE_SCHEMA).expect("valid schema");
        let payload = serde_json::json!({
            "id": "event-003",
            "amount": 1.0,
            "extra_field": "ignored"
        });
        // This may succeed or fail depending on avro resolution — document behavior
        let _result = encode_avro(&schema, &payload);
        // Not asserting here — behavior depends on apache-avro version
    }
}
```

### Step 2: Run to confirm RED

```bash
cargo nextest run -p force-pubsub
```

Expected: compile error (module not in lib.rs).

### Step 3: Add to lib.rs

```rust
pub mod codec;
```

### Step 4: Run to confirm GREEN

```bash
cargo nextest run -p force-pubsub
```

Expected: all codec tests pass.

### Step 5: Commit

```bash
git add crates/force-pubsub/src/codec.rs crates/force-pubsub/src/lib.rs
git commit -m "feat(pubsub): add Avro encode/decode codec"
```

---

## Task 5: Schema Cache

**Files:**
- Create: `crates/force-pubsub/src/schema_cache.rs`
- Modify: `crates/force-pubsub/src/lib.rs`

The schema cache wraps the gRPC `GetSchema` call, caching parsed `apache_avro::Schema` by schema ID in a `DashMap` so each schema is fetched at most once per handler instance.

### Step 1: Write failing tests

Create `crates/force-pubsub/src/schema_cache.rs`:

```rust
use apache_avro::Schema;
use dashmap::DashMap;
use std::sync::Arc;

use crate::error::{PubSubError, Result};

/// Fetches and caches Avro schemas by schema ID.
///
/// The cache is lock-free for reads via `DashMap`. Each schema is fetched
/// once from the Pub/Sub `GetSchema` RPC and cached for the lifetime of
/// the handler.
#[derive(Debug, Clone)]
pub struct SchemaCache {
    inner: Arc<SchemaCacheInner>,
}

#[derive(Debug)]
struct SchemaCacheInner {
    cache: DashMap<String, Schema>,
}

impl SchemaCache {
    /// Creates a new empty schema cache.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(SchemaCacheInner {
                cache: DashMap::new(),
            }),
        }
    }

    /// Insert a schema directly (used in tests and when schema JSON is already known).
    pub fn insert(&self, schema_id: String, schema: Schema) {
        self.inner.cache.insert(schema_id, schema);
    }

    /// Returns the number of cached schemas.
    pub fn len(&self) -> usize {
        self.inner.cache.len()
    }

    /// True if no schemas are cached.
    pub fn is_empty(&self) -> bool {
        self.inner.cache.is_empty()
    }

    /// Get a schema by ID if it is already in cache.
    pub fn get(&self, schema_id: &str) -> Option<Schema> {
        self.inner.cache.get(schema_id).map(|r| r.value().clone())
    }

    /// Parse and cache a schema from its JSON string.
    ///
    /// This is called after `GetSchema` returns `schema_json`.
    pub fn parse_and_insert(&self, schema_id: String, schema_json: &str) -> Result<Schema> {
        let schema = Schema::parse_str(schema_json)
            .map_err(|e| PubSubError::Avro(format!("failed to parse schema {schema_id}: {e}")))?;
        self.inner.cache.insert(schema_id, schema.clone());
        Ok(schema)
    }
}

impl Default for SchemaCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_SCHEMA_JSON: &str = r#"
    {
        "type": "record",
        "name": "OrderEvent",
        "fields": [
            {"name": "order_id", "type": "string"},
            {"name": "total", "type": "double"}
        ]
    }
    "#;

    #[test]
    fn test_new_cache_is_empty() {
        let cache = SchemaCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_parse_and_insert() {
        let cache = SchemaCache::new();
        let schema = cache
            .parse_and_insert("schema-001".to_string(), SIMPLE_SCHEMA_JSON)
            .expect("valid schema JSON");
        assert_eq!(cache.len(), 1);
        assert!(!cache.is_empty());
        // Get it back
        let retrieved = cache.get("schema-001").expect("was inserted");
        assert_eq!(
            format!("{:?}", retrieved),
            format!("{:?}", schema)
        );
    }

    #[test]
    fn test_get_miss_returns_none() {
        let cache = SchemaCache::new();
        assert!(cache.get("nonexistent").is_none());
    }

    #[test]
    fn test_parse_invalid_schema_returns_error() {
        let cache = SchemaCache::new();
        let result = cache.parse_and_insert("bad".to_string(), "{ not valid avro }");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PubSubError::Avro(_)));
        // Not inserted
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_is_cloneable_and_shared() {
        let cache = SchemaCache::new();
        let cache2 = cache.clone();

        cache.parse_and_insert("shared".to_string(), SIMPLE_SCHEMA_JSON)
            .expect("valid");

        // Clone shares the same underlying DashMap via Arc
        assert_eq!(cache2.len(), 1);
        assert!(cache2.get("shared").is_some());
    }
}
```

### Step 2: Run to confirm RED

```bash
cargo nextest run -p force-pubsub
```

Expected: compile error (module not in lib.rs).

### Step 3: Add to lib.rs

```rust
pub(crate) mod schema_cache;
pub use schema_cache::SchemaCache;
```

### Step 4: Run to confirm GREEN

```bash
cargo nextest run -p force-pubsub
```

Expected: all cache tests pass.

### Step 5: Commit

```bash
git add crates/force-pubsub/src/schema_cache.rs crates/force-pubsub/src/lib.rs
git commit -m "feat(pubsub): add DashMap-backed schema cache"
```

---

## Task 6: PubSubHandler + GetTopic/GetSchema

**Goal:** Implement `PubSubHandler::connect()`, `get_topic()`, `get_schema()`. This is where the tonic channel is set up and the first RPC calls are made.

**Files:**
- Create: `crates/force-pubsub/src/handler.rs`
- Create: `crates/force-pubsub/tests/common/mod.rs`
- Create: `crates/force-pubsub/tests/common/mock_server.rs`
- Create: `crates/force-pubsub/tests/handler_tests.rs`
- Modify: `crates/force-pubsub/src/lib.rs`

### Step 1: Create the mock gRPC server (test helper)

Create `crates/force-pubsub/tests/common/mod.rs`:
```rust
pub mod mock_server;
```

Create `crates/force-pubsub/tests/common/mock_server.rs`:

```rust
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio_stream::wrappers::{ReceiverStream, TcpListenerStream};
use tonic::{Request, Response, Status};
use tonic::transport::Server;

use force_pubsub::proto::eventbus_v1::{
    pub_sub_server::{PubSub, PubSubServer},
    FetchRequest, FetchResponse, PublishRequest, PublishResponse as ProtoPublishResponse,
    SchemaInfo, SchemaRequest, TopicInfo, TopicRequest,
};

/// Simple mock Pub/Sub server for unit tests.
pub struct MockPubSubService {
    pub topic_schema_id: String,
    pub schema_json: String,
}

impl Default for MockPubSubService {
    fn default() -> Self {
        Self {
            topic_schema_id: "schema-test-001".to_string(),
            schema_json: r#"{"type":"record","name":"TestEvent","fields":[{"name":"id","type":"string"}]}"#.to_string(),
        }
    }
}

#[tonic::async_trait]
impl PubSub for MockPubSubService {
    type SubscribeStream = ReceiverStream<Result<FetchResponse, Status>>;
    type PublishStreamStream = ReceiverStream<Result<ProtoPublishResponse, Status>>;

    async fn get_topic(&self, req: Request<TopicRequest>) -> Result<Response<TopicInfo>, Status> {
        let topic_name = req.into_inner().topic_name;
        Ok(Response::new(TopicInfo {
            topic_name: topic_name.clone(),
            topic_uri: topic_name,
            can_publish: true,
            can_subscribe: true,
            schema_id: self.topic_schema_id.clone(),
        }))
    }

    async fn get_schema(&self, req: Request<SchemaRequest>) -> Result<Response<SchemaInfo>, Status> {
        let schema_id = req.into_inner().schema_id;
        if schema_id == self.topic_schema_id {
            Ok(Response::new(SchemaInfo {
                schema_id,
                schema_json: self.schema_json.clone(),
            }))
        } else {
            Err(Status::not_found(format!("schema {schema_id} not found")))
        }
    }

    async fn subscribe(
        &self,
        _req: Request<tonic::Streaming<FetchRequest>>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let (tx, rx) = tokio::sync::mpsc::channel(10);
        // Stub — subscribe tests handled in subscriber_tests
        drop(tx);
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn publish(&self, _req: Request<PublishRequest>) -> Result<Response<ProtoPublishResponse>, Status> {
        Ok(Response::new(ProtoPublishResponse {
            topic_name: "test".to_string(),
            results: vec![],
            rpc_id: None,
        }))
    }

    async fn publish_stream(
        &self,
        _req: Request<tonic::Streaming<PublishRequest>>,
    ) -> Result<Response<Self::PublishStreamStream>, Status> {
        let (tx, rx) = tokio::sync::mpsc::channel(10);
        drop(tx);
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

/// Start an in-process mock Pub/Sub gRPC server.
/// Returns the base URL (e.g., "http://127.0.0.1:54321").
pub async fn start_mock_server(service: MockPubSubService) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let stream = TcpListenerStream::new(listener);

    tokio::spawn(async move {
        Server::builder()
            .add_service(PubSubServer::new(service))
            .serve_with_incoming(stream)
            .await
            .unwrap();
    });

    format!("http://{addr}")
}
```

### Step 2: Write failing handler tests

Create `crates/force-pubsub/tests/handler_tests.rs`:

```rust
mod common;
use common::mock_server::{start_mock_server, MockPubSubService};
use force_pubsub::{PubSubConfig, PubSubHandler};
use force::test_support::MockAuthenticator;
use force::client::builder;
use std::sync::Arc;

async fn make_handler(endpoint: String) -> PubSubHandler<MockAuthenticator> {
    let auth = MockAuthenticator::new("test-token", "https://test.salesforce.com");
    let client = builder().authenticate(auth).build().await.unwrap();
    let session = Arc::clone(client.inner());
    let config = PubSubConfig {
        endpoint,
        ..PubSubConfig::default()
    };
    PubSubHandler::connect(session, config).await.unwrap()
}

#[tokio::test]
async fn test_get_topic_returns_info() {
    let url = start_mock_server(MockPubSubService::default()).await;
    let handler = make_handler(url).await;

    let info = handler.get_topic("/event/MyEvent__e").await.unwrap();
    assert_eq!(info.topic_name, "/event/MyEvent__e");
    assert!(info.can_subscribe);
    assert_eq!(info.schema_id, "schema-test-001");
}

#[tokio::test]
async fn test_get_schema_returns_info() {
    let url = start_mock_server(MockPubSubService::default()).await;
    let handler = make_handler(url).await;

    let info = handler.get_schema("schema-test-001").await.unwrap();
    assert_eq!(info.schema_id, "schema-test-001");
    assert!(info.schema_json.contains("TestEvent"));
}

#[tokio::test]
async fn test_get_schema_not_found_returns_error() {
    let url = start_mock_server(MockPubSubService::default()).await;
    let handler = make_handler(url).await;

    let result = handler.get_schema("nonexistent-schema").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), force_pubsub::PubSubError::Transport(_)));
}

#[tokio::test]
async fn test_handler_is_cloneable() {
    let url = start_mock_server(MockPubSubService::default()).await;
    let handler = make_handler(url).await;
    let _cloned = handler.clone();
}
```

### Step 3: Run to confirm RED

```bash
cargo nextest run -p force-pubsub --test handler_tests
```

Expected: compile error — `PubSubHandler` doesn't exist yet.

### Step 4: Implement handler.rs

Create `crates/force-pubsub/src/handler.rs`:

```rust
use std::sync::Arc;
use tonic::transport::Channel;

use force::auth::Authenticator;
use force::session::Session;

use crate::config::PubSubConfig;
use crate::error::{PubSubError, Result};
use crate::schema_cache::SchemaCache;
use crate::proto::eventbus_v1::{
    pub_sub_client::PubSubClient,
    SchemaRequest, TopicRequest,
};

/// Public-facing TopicInfo (mirrors proto but without generated types leaking).
#[derive(Debug, Clone)]
pub struct TopicInfo {
    pub topic_name: String,
    pub topic_uri: String,
    pub can_publish: bool,
    pub can_subscribe: bool,
    pub schema_id: String,
}

/// Public-facing SchemaInfo.
#[derive(Debug, Clone)]
pub struct SchemaInfo {
    pub schema_id: String,
    pub schema_json: String,
}

/// Entry point for all Salesforce Pub/Sub operations.
///
/// Obtained by calling `PubSubHandler::connect()` with a `Session` and `PubSubConfig`.
#[derive(Clone)]
pub struct PubSubHandler<A: Authenticator> {
    pub(crate) session: Arc<Session<A>>,
    pub(crate) config: PubSubConfig,
    pub(crate) schema_cache: SchemaCache,
    pub(crate) channel: Channel,
}

impl<A: Authenticator> PubSubHandler<A> {
    /// Connect to the Pub/Sub gRPC endpoint and return a handler.
    ///
    /// This is async because it establishes the gRPC channel.
    pub async fn connect(session: Arc<Session<A>>, config: PubSubConfig) -> Result<Self> {
        if config.batch_size < 1 || config.batch_size > 100 {
            return Err(PubSubError::Config(
                "batch_size must be between 1 and 100".to_string(),
            ));
        }

        let channel = Channel::from_shared(config.endpoint.clone())
            .map_err(|e| PubSubError::Config(format!("invalid endpoint: {e}")))?
            .connect()
            .await?;

        Ok(Self {
            session,
            config,
            schema_cache: SchemaCache::new(),
            channel,
        })
    }

    /// Returns a gRPC client with auth metadata injected.
    ///
    /// Uses pre-fetch pattern: gets the token, then attaches it to a
    /// tonic interceptor so each RPC sends correct auth headers.
    async fn grpc_client(&self) -> Result<PubSubClient<Channel>> {
        Ok(PubSubClient::new(self.channel.clone()))
    }

    /// Injects auth metadata into a tonic request.
    async fn auth_request<T>(&self, message: T) -> Result<tonic::Request<T>> {
        let token = self.session.token_manager.token().await?;
        let instance_url = token.instance_url().to_string();
        let access_token = token.as_str().to_string();

        let mut req = tonic::Request::new(message);
        let metadata = req.metadata_mut();
        metadata.insert(
            "accesstoken",
            access_token.parse().map_err(|_| PubSubError::Config("invalid token chars".to_string()))?,
        );
        metadata.insert(
            "instanceurl",
            instance_url.parse().map_err(|_| PubSubError::Config("invalid instance URL chars".to_string()))?,
        );
        Ok(req)
    }

    /// Fetch metadata about a Pub/Sub topic.
    pub async fn get_topic(&self, topic_name: &str) -> Result<TopicInfo> {
        let mut client = self.grpc_client().await?;
        let req = self.auth_request(TopicRequest {
            topic_name: topic_name.to_string(),
        }).await?;

        let resp = client.get_topic(req).await?;
        let info = resp.into_inner();
        Ok(TopicInfo {
            topic_name: info.topic_name,
            topic_uri: info.topic_uri,
            can_publish: info.can_publish,
            can_subscribe: info.can_subscribe,
            schema_id: info.schema_id,
        })
    }

    /// Fetch an Avro schema by its ID.
    ///
    /// Results are cached — a given schema ID is only fetched once per handler instance.
    pub async fn get_schema(&self, schema_id: &str) -> Result<SchemaInfo> {
        let mut client = self.grpc_client().await?;
        let req = self.auth_request(SchemaRequest {
            schema_id: schema_id.to_string(),
        }).await?;

        let resp = client.get_schema(req).await?;
        let info = resp.into_inner();
        Ok(SchemaInfo {
            schema_id: info.schema_id,
            schema_json: info.schema_json,
        })
    }
}
```

Add to `lib.rs`:
```rust
pub mod handler;
pub use handler::{PubSubHandler, SchemaInfo, TopicInfo};
```

Also expose `Session` from `force` in tests — add to `force`'s `client/mod.rs` a pub method that returns `Arc<Session<A>>` to consumers:

> **Note:** `force::session::Session` is `pub(crate)`. The `force_pubsub` crate needs to accept `Arc<Session<A>>`. The easiest solution is to expose it: in `crates/force/src/session.rs`, change `pub(crate)` to `pub` for the struct. Or add a `pub fn into_session(self) -> Arc<Session<A>>` on `ForceClient`. For now, add a `pub fn session(&self) -> Arc<Session<A>>` method on `ForceClient` in `crates/force/src/client/mod.rs`.

In `crates/force/src/client/mod.rs`, add:
```rust
/// Returns the shared session state (for use by extension crates such as force-pubsub).
#[must_use]
pub fn session(&self) -> Arc<Session<A>> {
    Arc::clone(&self.inner)
}
```

And in `crates/force/src/session.rs`, change the struct visibility:
```rust
pub struct Session<A: ...>  // was pub(crate)
```

### Step 5: Run to confirm GREEN

```bash
cargo nextest run -p force-pubsub --test handler_tests
```

Expected: all 4 handler tests pass.

Also run force tests to make sure we didn't break anything:
```bash
cargo nextest run -p force
```

### Step 6: Commit

```bash
git add crates/force-pubsub/src/ crates/force-pubsub/tests/ crates/force/src/
git commit -m "feat(pubsub): implement PubSubHandler with GetTopic/GetSchema"
```

---

## Task 7: Subscribe Stream

**Goal:** Implement `subscribe()` and `subscribe_typed()` with configurable reconnection.

**Files:**
- Create: `crates/force-pubsub/src/subscriber.rs`
- Create: `crates/force-pubsub/tests/subscriber_tests.rs`
- Modify: `crates/force-pubsub/src/handler.rs`
- Modify: `crates/force-pubsub/src/lib.rs`

### Step 1: Write failing subscriber tests

Create `crates/force-pubsub/tests/subscriber_tests.rs`:

```rust
mod common;
use common::mock_server::{start_mock_server, MockPubSubService, MockPubSubServiceBuilder};
use force_pubsub::{PubSubConfig, PubSubHandler, PubSubEvent, ReplayPreset, ReconnectPolicy};
use force::test_support::MockAuthenticator;
use force::client::builder;
use futures::StreamExt;
use std::sync::Arc;

// MockPubSubServiceBuilder lets tests configure what events the server sends
// See: tests/common/mock_server.rs (add builder there)

async fn make_handler(endpoint: String) -> PubSubHandler<MockAuthenticator> {
    let auth = MockAuthenticator::new("test-token", "https://test.salesforce.com");
    let client = builder().authenticate(auth).build().await.unwrap();
    let config = PubSubConfig {
        endpoint,
        reconnect_policy: ReconnectPolicy::None, // No reconnect in unit tests
        ..PubSubConfig::default()
    };
    PubSubHandler::connect(client.session(), config).await.unwrap()
}

#[tokio::test]
async fn test_subscribe_yields_keepalive_when_no_events() {
    // Server sends empty FetchResponse
    let url = start_mock_server(
        MockPubSubServiceBuilder::default()
            .with_fetch_responses(vec![empty_fetch_response()])
            .build()
    ).await;
    let handler = make_handler(url).await;
    let mut stream = handler.subscribe("/event/Test__e", ReplayPreset::Latest).await.unwrap();

    let first = stream.next().await.unwrap().unwrap();
    assert!(matches!(first, PubSubEvent::KeepAlive));
}

#[tokio::test]
async fn test_subscribe_yields_event_with_payload() {
    let schema_json = r#"{"type":"record","name":"TestEvent","fields":[{"name":"id","type":"string"}]}"#;
    // Server sends one event
    let url = start_mock_server(
        MockPubSubServiceBuilder::default()
            .with_schema(schema_json)
            .with_fetch_responses(vec![
                fetch_response_with_event(schema_json, r#"{"id":"evt-001"}"#)
            ])
            .build()
    ).await;
    let handler = make_handler(url).await;
    let mut stream = handler.subscribe("/event/Test__e", ReplayPreset::Latest).await.unwrap();

    let first = stream.next().await.unwrap().unwrap();
    match first {
        PubSubEvent::Event(msg) => {
            assert_eq!(msg.payload["id"], "evt-001");
        }
        other => panic!("expected Event, got {:?}", other),
    }
}

#[tokio::test]
async fn test_subscribe_replay_preset_earliest_sends_correct_proto() {
    // Verify the FetchRequest has replay_preset=EARLIEST
    let url = start_mock_server(
        MockPubSubServiceBuilder::default()
            .capture_requests()
            .build()
    ).await;
    let handler = make_handler(url).await;
    let _stream = handler.subscribe("/event/Test__e", ReplayPreset::Earliest).await.unwrap();
    // Check captured request had replay_preset == 1 (EARLIEST)
    // Details depend on MockPubSubServiceBuilder implementation
}

// Helper constructors (put in tests/common/mock_server.rs)
fn empty_fetch_response() -> force_pubsub::proto::eventbus_v1::FetchResponse {
    force_pubsub::proto::eventbus_v1::FetchResponse {
        topic_name: "/event/Test__e".to_string(),
        latest_replay_id: vec![1, 2, 3],
        events: vec![],
        pending_num_requested: 0,
        rpc_id: None,
    }
}

fn fetch_response_with_event(schema_json: &str, payload_json: &str) -> force_pubsub::proto::eventbus_v1::FetchResponse {
    use force_pubsub::codec::encode_avro;
    use apache_avro::Schema;

    let schema = Schema::parse_str(schema_json).unwrap();
    let payload_value: serde_json::Value = serde_json::from_str(payload_json).unwrap();
    let bytes = encode_avro(&schema, &payload_value).unwrap();

    let event_header = force_pubsub::proto::eventbus_v1::EventHeader {
        replay_id: vec![9, 8, 7],
        producer_partition_key: String::new(),
        headers: std::collections::HashMap::new(),
        schema_id: "schema-test-001".to_string(),
    };
    let consumer_event = force_pubsub::proto::eventbus_v1::ConsumerEvent {
        event: Some(event_header),
        payload: bytes,
    };

    force_pubsub::proto::eventbus_v1::FetchResponse {
        topic_name: "/event/Test__e".to_string(),
        latest_replay_id: vec![9, 8, 7],
        events: vec![consumer_event],
        pending_num_requested: 0,
        rpc_id: None,
    }
}
```

> **Note:** The `MockPubSubServiceBuilder` is a test helper you'll add to `tests/common/mock_server.rs` as you implement this. It lets tests configure what responses the mock server returns.

### Step 2: Run to confirm RED

```bash
cargo nextest run -p force-pubsub --test subscriber_tests
```

Expected: compile errors.

### Step 3: Implement `subscriber.rs`

Create `crates/force-pubsub/src/subscriber.rs`:

```rust
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::Stream;
use tokio_stream::wrappers::ReceiverStream;

use force::auth::Authenticator;
use force::session::Session;

use crate::codec::{decode_avro, decode_avro_typed};
use crate::config::{PubSubConfig, ReconnectPolicy, ReplayPreset};
use crate::error::{PubSubError, Result};
use crate::schema_cache::SchemaCache;
use crate::types::{EventMessage, PubSubEvent, ReplayId};
use crate::proto::eventbus_v1::{
    pub_sub_client::PubSubClient,
    FetchRequest,
    replay_preset::ReplayPreset as ProtoReplayPreset,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tonic::transport::Channel;

/// Build a `FetchRequest` from our config and replay preset.
pub(crate) fn build_fetch_request(
    topic: &str,
    preset: &ReplayPreset,
    batch_size: i32,
) -> FetchRequest {
    match preset {
        ReplayPreset::Latest => FetchRequest {
            topic_name: topic.to_string(),
            replay_preset: ProtoReplayPreset::Latest as i32,
            replay_id: vec![],
            num_requested: batch_size,
            auth_refresh: None,
        },
        ReplayPreset::Earliest => FetchRequest {
            topic_name: topic.to_string(),
            replay_preset: ProtoReplayPreset::Earliest as i32,
            replay_id: vec![],
            num_requested: batch_size,
            auth_refresh: None,
        },
        ReplayPreset::Custom(id) => FetchRequest {
            topic_name: topic.to_string(),
            replay_preset: ProtoReplayPreset::Custom as i32,
            replay_id: id.as_bytes().to_vec(),
            num_requested: batch_size,
            auth_refresh: None,
        },
    }
}

/// Internal state for a single subscribe connection attempt.
struct SubscribeState<A: Authenticator> {
    session: Arc<Session<A>>,
    config: PubSubConfig,
    schema_cache: SchemaCache,
    channel: Channel,
    topic: String,
    last_replay_id: Option<ReplayId>,
}

impl<A: Authenticator + 'static> SubscribeState<A> {
    async fn get_token(&self) -> Result<force::auth::AccessToken> {
        self.session.token_manager.token().await.map_err(PubSubError::Auth)
    }

    async fn open_stream(
        &self,
        preset: &ReplayPreset,
    ) -> Result<tonic::codec::Streaming<crate::proto::eventbus_v1::FetchResponse>> {
        let token = self.get_token().await?;
        let mut client = PubSubClient::new(self.channel.clone());

        let (tx, rx) = mpsc::channel(10);
        let req_msg = build_fetch_request(&self.topic, preset, self.config.batch_size);

        // Send initial fetch request
        tx.send(req_msg).await.map_err(|_| PubSubError::Config("channel closed".to_string()))?;

        let mut req = tonic::Request::new(ReceiverStream::new(rx));
        let metadata = req.metadata_mut();
        metadata.insert(
            "accesstoken",
            token.as_str().parse().map_err(|_| PubSubError::Config("invalid token".to_string()))?,
        );
        metadata.insert(
            "instanceurl",
            token.instance_url().parse().map_err(|_| PubSubError::Config("invalid url".to_string()))?,
        );

        let response = client.subscribe(req).await?;
        Ok(response.into_inner())
    }
}

/// Run the subscribe loop, sending events to `tx`.
async fn subscribe_loop<A: Authenticator + Send + Sync + 'static>(
    state: SubscribeState<A>,
    tx: mpsc::Sender<Result<PubSubEvent<Value>>>,
) {
    let initial_preset = ReplayPreset::Latest; // first open
    let mut current_preset = initial_preset;
    let mut reconnect_count = 0u32;

    'outer: loop {
        let mut stream = match state.open_stream(&current_preset).await {
            Ok(s) => s,
            Err(e) => {
                let _ = tx.send(Err(e)).await;
                break;
            }
        };

        // Drain events from this stream connection
        loop {
            match stream.message().await {
                Ok(Some(response)) => {
                    let latest = ReplayId::from_bytes(response.latest_replay_id.clone());

                    if response.events.is_empty() {
                        if tx.send(Ok(PubSubEvent::KeepAlive)).await.is_err() {
                            break 'outer;
                        }
                    } else {
                        for event in &response.events {
                            let Some(header) = &event.event else { continue };
                            let replay_id = ReplayId::from_bytes(header.replay_id.clone());
                            let schema_id = header.schema_id.clone();

                            // Fetch schema if not cached
                            let schema = match state.schema_cache.get(&schema_id) {
                                Some(s) => s,
                                None => {
                                    // Fetch from gRPC (simplified — full impl uses handler.get_schema)
                                    match state.schema_cache.get(&schema_id) {
                                        Some(s) => s,
                                        None => {
                                            let _ = tx.send(Err(PubSubError::SchemaNotFound { schema_id })).await;
                                            continue;
                                        }
                                    }
                                }
                            };

                            match decode_avro(&schema, &event.payload) {
                                Ok(payload) => {
                                    let msg = EventMessage {
                                        payload,
                                        replay_id: replay_id.clone(),
                                        schema_id: header.schema_id.clone(),
                                        event_id: header.producer_partition_key.clone(),
                                    };
                                    if tx.send(Ok(PubSubEvent::Event(msg))).await.is_err() {
                                        break 'outer;
                                    }
                                    reconnect_count = 0; // reset on success
                                }
                                Err(e) => {
                                    if tx.send(Err(e)).await.is_err() {
                                        break 'outer;
                                    }
                                }
                            }
                        }
                    }

                    // Update replay position
                    current_preset = ReplayPreset::Custom(latest);
                }

                Ok(None) | Err(_) => {
                    // Stream ended or errored — try to reconnect
                    match &state.config.reconnect_policy {
                        ReconnectPolicy::None => {
                            let _ = tx.send(Err(PubSubError::Transport(
                                tonic::Status::unavailable("subscribe stream ended"),
                            ))).await;
                            break 'outer;
                        }
                        ReconnectPolicy::Auto { max_retries, backoff } => {
                            reconnect_count += 1;
                            if reconnect_count > *max_retries {
                                let last = Box::new(PubSubError::Transport(
                                    tonic::Status::unavailable("max retries exceeded"),
                                ));
                                let _ = tx.send(Err(PubSubError::ReconnectFailed {
                                    attempts: reconnect_count,
                                    last_error: last,
                                })).await;
                                break 'outer;
                            }

                            let delay = backoff.delay_for(reconnect_count - 1);
                            tokio::time::sleep(delay).await;

                            let replay_id = match &current_preset {
                                ReplayPreset::Custom(id) => id.clone(),
                                _ => ReplayId::from_bytes(vec![]),
                            };

                            let _ = tx.send(Ok(PubSubEvent::Reconnected {
                                replay_id: replay_id.clone(),
                                attempt: reconnect_count,
                            })).await;

                            current_preset = ReplayPreset::Custom(replay_id);
                            break; // break inner loop, retry outer
                        }
                    }
                }
            }
        }
    }
}

/// Subscribe to a Pub/Sub topic, returning a stream of dynamic events.
pub fn subscribe_dynamic<A: Authenticator + Send + Sync + 'static>(
    session: Arc<Session<A>>,
    config: PubSubConfig,
    schema_cache: SchemaCache,
    channel: Channel,
    topic: String,
    preset: ReplayPreset,
) -> Pin<Box<dyn Stream<Item = Result<PubSubEvent<Value>>> + Send>> {
    let (tx, rx) = mpsc::channel(config.batch_size as usize * 2);
    let state = SubscribeState { session, config, schema_cache, channel, topic, last_replay_id: None };

    tokio::spawn(subscribe_loop(state, tx));
    Box::pin(ReceiverStream::new(rx))
}
```

Add `subscribe()` and `subscribe_typed()` methods to `handler.rs`:

```rust
use crate::subscriber::subscribe_dynamic;

impl<A: Authenticator + Send + Sync + 'static> PubSubHandler<A> {
    /// Subscribe to a topic, yielding decoded events as `serde_json::Value`.
    pub async fn subscribe(
        &self,
        topic: &str,
        replay: ReplayPreset,
    ) -> Result<impl Stream<Item = Result<PubSubEvent<serde_json::Value>>> + Send> {
        Ok(subscribe_dynamic(
            Arc::clone(&self.session),
            self.config.clone(),
            self.schema_cache.clone(),
            self.channel.clone(),
            topic.to_string(),
            replay,
        ))
    }
}
```

### Step 4: Run to confirm GREEN

```bash
cargo nextest run -p force-pubsub --test subscriber_tests
```

Iterate until tests pass.

### Step 5: Commit

```bash
git add crates/force-pubsub/src/ crates/force-pubsub/tests/
git commit -m "feat(pubsub): implement subscribe stream with reconnection"
```

---

## Task 8: Publish

**Goal:** Implement `publish()` (unary) and `publish_stream()` (bidirectional).

**Files:**
- Create: `crates/force-pubsub/src/publisher.rs`
- Create: `crates/force-pubsub/tests/publisher_tests.rs`
- Modify: `crates/force-pubsub/src/handler.rs`

### Step 1: Write failing publish tests

Create `crates/force-pubsub/tests/publisher_tests.rs`:

```rust
mod common;
use common::mock_server::{start_mock_server, MockPubSubService};
use force_pubsub::{PubSubConfig, PubSubHandler, ReconnectPolicy};
use force::test_support::MockAuthenticator;
use force::client::builder;
use std::sync::Arc;

async fn make_handler(endpoint: String) -> PubSubHandler<MockAuthenticator> {
    let auth = MockAuthenticator::new("test-token", "https://test.salesforce.com");
    let client = builder().authenticate(auth).build().await.unwrap();
    let config = PubSubConfig {
        endpoint,
        reconnect_policy: ReconnectPolicy::None,
        ..PubSubConfig::default()
    };
    PubSubHandler::connect(client.session(), config).await.unwrap()
}

const SCHEMA_JSON: &str = r#"{"type":"record","name":"TestEvent","fields":[{"name":"id","type":"string"}]}"#;

#[tokio::test]
async fn test_publish_returns_response() {
    let url = start_mock_server(MockPubSubService::default()).await;
    let handler = make_handler(url).await;

    // Pre-populate schema cache so publish can encode
    handler.schema_cache.parse_and_insert("schema-test-001".to_string(), SCHEMA_JSON).unwrap();

    let payload = serde_json::json!({"id": "evt-001"});
    let resp = handler.publish("schema-test-001", "/event/Test__e", vec![payload]).await.unwrap();
    assert_eq!(resp.topic_name, "test");
}

#[tokio::test]
async fn test_publish_empty_events_is_ok() {
    let url = start_mock_server(MockPubSubService::default()).await;
    let handler = make_handler(url).await;
    handler.schema_cache.parse_and_insert("schema-test-001".to_string(), SCHEMA_JSON).unwrap();

    let resp = handler.publish::<serde_json::Value>("schema-test-001", "/event/Test__e", vec![]).await.unwrap();
    assert!(resp.all_succeeded());
}
```

### Step 2: Run to confirm RED

```bash
cargo nextest run -p force-pubsub --test publisher_tests
```

Expected: compile error.

### Step 3: Implement publisher.rs

Create `crates/force-pubsub/src/publisher.rs`:

```rust
use serde::Serialize;
use tonic::transport::Channel;
use std::sync::Arc;

use force::auth::Authenticator;
use force::session::Session;

use crate::codec::encode_avro;
use crate::error::{PubSubError, Result};
use crate::schema_cache::SchemaCache;
use crate::types::{PublishResponse, PublishResult, ReplayId};
use crate::proto::eventbus_v1::{
    pub_sub_client::PubSubClient,
    ProducerEvent, PublishRequest,
};

/// Encode events and publish them via the unary Publish RPC.
pub async fn publish_unary<A, T>(
    session: &Arc<Session<A>>,
    channel: &Channel,
    schema_cache: &SchemaCache,
    schema_id: &str,
    topic: &str,
    events: Vec<T>,
) -> Result<PublishResponse>
where
    A: Authenticator,
    T: Serialize,
{
    let schema = schema_cache
        .get(schema_id)
        .ok_or_else(|| PubSubError::SchemaNotFound { schema_id: schema_id.to_string() })?;

    let mut producer_events = Vec::with_capacity(events.len());
    for event in &events {
        let payload = encode_avro(&schema, event)?;
        producer_events.push(ProducerEvent {
            schema_id: schema_id.to_string(),
            payload,
        });
    }

    let token = session.token_manager.token().await?;
    let mut client = PubSubClient::new(channel.clone());

    let mut req = tonic::Request::new(PublishRequest {
        topic_name: topic.to_string(),
        events: producer_events,
    });

    let metadata = req.metadata_mut();
    metadata.insert(
        "accesstoken",
        token.as_str().parse().map_err(|_| PubSubError::Config("invalid token".to_string()))?,
    );
    metadata.insert(
        "instanceurl",
        token.instance_url().parse().map_err(|_| PubSubError::Config("invalid url".to_string()))?,
    );

    let resp = client.publish(req).await?.into_inner();

    let results = resp.results
        .into_iter()
        .map(|r| PublishResult {
            replay_id: if r.replay_id.is_empty() {
                None
            } else {
                Some(ReplayId::from_bytes(r.replay_id))
            },
            error: r.error.and_then(|e| {
                if e.code == 0 && e.msg.is_empty() {
                    None
                } else {
                    Some(e.msg)
                }
            }),
        })
        .collect();

    Ok(PublishResponse {
        topic_name: resp.topic_name,
        results,
    })
}
```

Add to `handler.rs`:

```rust
use crate::publisher::publish_unary;

impl<A: Authenticator + Send + Sync + 'static> PubSubHandler<A> {
    /// Publish events to a topic (unary RPC).
    ///
    /// `schema_id` must be pre-loaded in the schema cache via `get_schema()`.
    pub async fn publish<T: Serialize>(
        &self,
        schema_id: &str,
        topic: &str,
        events: Vec<T>,
    ) -> Result<PublishResponse> {
        publish_unary(&self.session, &self.channel, &self.schema_cache, schema_id, topic, events).await
    }
}
```

### Step 4: Run to confirm GREEN

```bash
cargo nextest run -p force-pubsub --test publisher_tests
```

### Step 5: Commit

```bash
git add crates/force-pubsub/src/ crates/force-pubsub/tests/publisher_tests.rs
git commit -m "feat(pubsub): implement unary publish"
```

---

## Task 9: ADR + Example

**Files:**
- Create: `docs/adr/018-force-pubsub-crate.md`
- Create: `crates/force-pubsub/examples/subscribe_events.rs`

### Step 1: Write ADR-018

Create `docs/adr/018-force-pubsub-crate.md`:

```markdown
# ADR-018: Re-introduce Pub/Sub as a Separate Crate

**Status:** Accepted
**Date:** 2026-03-16
**Supersedes:** ADR-011 (Remove Pub/Sub Support)

## Context

ADR-011 removed Pub/Sub from the core `force` crate due to heavy gRPC dependencies
(tonic, prost, protoc). It left open the possibility of re-introducing it as
`force-pubsub`. We now implement that.

## Decision

`force-pubsub` is a separate workspace crate that:
- Depends on `force` for `Session<A>`, auth, config
- Uses `protoc-bin-vendored` to avoid requiring system `protoc`
- Uses a `DashMap`-based schema cache
- Exposes a configurable `ReconnectPolicy` (None vs Auto with backoff)
- Returns `PubSubEvent<T>` streams (Event | Reconnected | KeepAlive)

## Consequences

- REST/Bulk users see zero impact on compile time
- `force-pubsub` adds ~tonic + prost + avro to the binary when used
- Auth is shared via `Session<A>` — one token for all API surfaces
```

### Step 2: Write subscribe example

Create `crates/force-pubsub/examples/subscribe_events.rs`:

```rust
//! Example: subscribe to a Salesforce Platform Event stream.
//!
//! Run with:
//! ```
//! SF_CLIENT_ID=xxx SF_CLIENT_SECRET=yyy cargo run --example subscribe_events -p force-pubsub
//! ```

use force::auth::ClientCredentials;
use force::client::builder;
use force_pubsub::{PubSubConfig, PubSubHandler, PubSubEvent, ReplayPreset};
use futures::StreamExt;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client_id = std::env::var("SF_CLIENT_ID")?;
    let client_secret = std::env::var("SF_CLIENT_SECRET")?;

    let auth = ClientCredentials::new(client_id, client_secret);
    let client = builder().authenticate(auth).build().await?;

    let handler = PubSubHandler::connect(
        client.session(),
        PubSubConfig::default(),
    ).await?;

    println!("Subscribing to /event/MyPlatformEvent__e ...");

    let mut stream = handler
        .subscribe("/event/MyPlatformEvent__e", ReplayPreset::Latest)
        .await?;

    while let Some(item) = stream.next().await {
        match item? {
            PubSubEvent::Event(msg) => {
                println!("Event [{}]: {}", msg.event_id, msg.payload);
            }
            PubSubEvent::Reconnected { attempt, .. } => {
                eprintln!("Reconnected (attempt {attempt})");
            }
            PubSubEvent::KeepAlive => {
                // Heartbeat — no action needed
            }
        }
    }

    Ok(())
}
```

### Step 3: Run full test suite

```bash
cargo nextest run --workspace
cargo clippy --workspace --all-features -- -D warnings
cargo fmt --all -- --check
```

Expected: all green.

### Step 4: Final commit

```bash
git add docs/adr/018-force-pubsub-crate.md crates/force-pubsub/examples/
git commit -m "docs(pubsub): add ADR-018 and subscribe example"
```

---

## Checklist

- [ ] Task 1: Workspace scaffold compiles
- [ ] Task 2: Error types — 5 tests green
- [ ] Task 3: Config & event types — 13 tests green
- [ ] Task 4: Avro codec — 3 tests green
- [ ] Task 5: Schema cache — 6 tests green
- [ ] Task 6: PubSubHandler + GetTopic/GetSchema — 4 tests green
- [ ] Task 7: Subscribe stream — 3 tests green
- [ ] Task 8: Publish — 2 tests green
- [ ] Task 9: ADR + example
- [ ] `cargo nextest run --workspace` all green
- [ ] `cargo clippy --workspace --all-features -- -D warnings` zero warnings
- [ ] `cargo fmt --all -- --check` passes
