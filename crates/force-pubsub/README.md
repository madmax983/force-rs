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

```toml
[dependencies]
force = "0.4"
force-pubsub = "0.4"
tokio = { version = "1", features = ["full"] }
tokio-stream = "0.1"   # for `StreamExt::next`
```

```rust,no_run
use force::auth::ClientCredentials;
use force::client::ForceClientBuilder;
use force_pubsub::{PubSubConfig, PubSubEvent, PubSubHandler, ReplayPreset};
use tokio_stream::StreamExt;

# async fn run() -> Result<(), Box<dyn std::error::Error>> {
// Authenticate with the `force` crate, then hand its session to Pub/Sub.
let auth = ClientCredentials::new_production("client_id", "client_secret");
let client = ForceClientBuilder::new().authenticate(auth).build().await?;

let handler = PubSubHandler::connect(client.session(), PubSubConfig::default()).await?;

// Subscribe to Change Data Capture events
let mut stream = handler
    .subscribe("/data/AccountChangeEvent", ReplayPreset::Latest)
    .await?;

while let Some(item) = stream.next().await {
    match item? {
        PubSubEvent::Event(msg) => println!("Received event {}", msg.event_id),
        PubSubEvent::Reconnected { attempt, .. } => println!("Reconnected (attempt {attempt})"),
        PubSubEvent::KeepAlive => {}
    }
}
# Ok(())
# }
```

A complete runnable version (env-var config, reconnect policy) is in
[`examples/subscribe_events.rs`](examples/subscribe_events.rs).

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
