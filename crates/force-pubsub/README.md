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
use force_pubsub::{PubSubConfig, PubSubHandler, ReplayPreset};

// Build a force Session first (see force crate docs)
let session = /* ... */;

let config = PubSubConfig::default();
let handler = PubSubHandler::connect(session, config).await?;

// Subscribe to Change Data Capture events
let mut stream = handler
    .subscribe("/data/AccountChangeEvent", ReplayPreset::Latest, None)
    .await?;

while let Some(event) = stream.next().await {
    println!("Received: {:?}", event?);
}
```

## Dependencies

This crate depends on the [`force`](https://crates.io/crates/force) crate for authentication and session management.

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or [MIT License](../../LICENSE-MIT) at your option.
