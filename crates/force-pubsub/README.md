# force-pubsub

[![Crates.io](https://img.shields.io/crates/v/force-pubsub.svg)](https://crates.io/crates/force-pubsub)
[![Documentation](https://docs.rs/force-pubsub/badge.svg)](https://docs.rs/force-pubsub)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](../../LICENSE-MIT)

**Salesforce Pub/Sub API (gRPC) client for Rust** — part of the [force-rs](https://github.com/madmax983/force-rs) workspace.

`force-pubsub` provides a streaming gRPC client for the Salesforce Pub/Sub API, enabling real-time event consumption and publishing with Avro-encoded Change Data Capture (CDC) events, Platform Events, and custom channels.

## Features

- **Subscribe** to CDC events, Platform Events, and custom channels with automatic Avro decoding
- **Publish** events with schema-aware Avro encoding and delivery confirmation
- **Schema caching** with concurrent `DashMap`-backed cache for high-throughput workloads
- **Reconnection** with configurable backoff, replay support, and managed resubscription
- **Authentication** via the `force` crate's `Session` (Client Credentials, JWT, etc.)

## Quick Start

```rust
use futures::StreamExt;
use force_pubsub::{PubSubConfig, PubSubHandler, PubSubEvent, ReplayPreset};

// `client` is an authenticated `force::client::ForceClient` (see the force crate docs).
let handler = PubSubHandler::connect(client.session(), PubSubConfig::default()).await?;

// Subscribe to Change Data Capture events
let mut stream = handler
    .subscribe("/data/AccountChangeEvent", ReplayPreset::Latest)
    .await?;

while let Some(item) = stream.next().await {
    if let PubSubEvent::Event(msg) = item? {
        println!("event_id={}", msg.event_id);
    }
}
```

Add `futures` to your `Cargo.toml` for `StreamExt`.

## Dependencies

This crate depends on the [`force`](https://crates.io/crates/force) crate for authentication and session management.

## Live Contract Test

The ignored live smoke test exercises `GetTopic` and `GetSchema` against Salesforce's Pub/Sub gRPC endpoint.

```bash
SF_PUBSUB_TOPIC=/data/AccountChangeEvent \
cargo test -p force-pubsub --test live_salesforce_pubsub -- --ignored --test-threads=1
```

`SF_PUBSUB_ENDPOINT` defaults to `https://api.pubsub.salesforce.com:7443`. The test reuses the same Salesforce auth environment variables as the core `force` live tests.

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or [MIT License](../../LICENSE-MIT) at your option.
