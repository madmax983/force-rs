//! Subscribe to Salesforce Platform Events via the Pub/Sub API.
//!
//! This example demonstrates the connect/subscribe flow with automatic
//! reconnection and platform event stream consumption.
//!
//! # Setup
//!
//! Set the following environment variables:
//! - `SF_CLIENT_ID`     - OAuth client ID (Connected App Consumer Key)
//! - `SF_CLIENT_SECRET` - OAuth client secret (Connected App Consumer Secret)
//! - `SF_INSTANCE_URL`  - Salesforce instance URL (e.g. `https://myorg.my.salesforce.com`)
//! - `SF_TOPIC_NAME`    - Pub/Sub topic (default: `/event/My_Platform_Event__e`)
//!
//! # Run
//!
//! ```bash
//! SF_CLIENT_ID=xxx SF_CLIENT_SECRET=yyy SF_INSTANCE_URL=zzz \
//!   cargo run --example subscribe_events
//! ```

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use tokio_stream::StreamExt;

use force::auth::ClientCredentials;
use force::client::ForceClientBuilder;
use force_pubsub::{
    BackoffConfig, PubSubConfig, PubSubEvent, PubSubHandler, ReconnectPolicy, ReplayPreset,
};

fn required_env(name: &str) -> anyhow::Result<String> {
    std::env::var(name).with_context(|| format!("{name} environment variable not set"))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    // --- Read configuration from environment ---
    let client_id = required_env("SF_CLIENT_ID")?;
    let client_secret = required_env("SF_CLIENT_SECRET")?;
    let topic = std::env::var("SF_TOPIC_NAME")
        .unwrap_or_else(|_| "/event/My_Platform_Event__e".to_string());

    println!("Authenticating with Salesforce...");
    let auth = ClientCredentials::new_production(client_id, client_secret);
    let force_client = ForceClientBuilder::new()
        .authenticate(auth)
        .build()
        .await
        .context("Failed to build ForceClient")?;
    println!("Authentication successful");

    // --- Build Pub/Sub config with auto-reconnect ---
    let config = PubSubConfig {
        reconnect_policy: ReconnectPolicy::Auto {
            max_retries: 3,
            backoff: BackoffConfig {
                initial_delay: Duration::from_secs(1),
                max_delay: Duration::from_secs(30),
                multiplier: 2.0,
            },
        },
        ..PubSubConfig::default()
    };

    // --- Connect to the Pub/Sub gRPC endpoint ---
    println!("Connecting to Pub/Sub API...");
    let session = force_client.session();
    let handler = PubSubHandler::connect(Arc::clone(&session), config)
        .await
        .context("Failed to connect to Pub/Sub API")?;
    println!("Connected");

    // --- Subscribe from the latest replay position ---
    println!("Subscribing to topic: {topic}");
    let mut stream = handler
        .subscribe(&topic, ReplayPreset::Latest)
        .await
        .context("Failed to subscribe")?;
    drop(handler);

    println!("Waiting for events (will stop after 10)...\n");

    let mut event_count = 0usize;
    while let Some(item) = stream.next().await {
        match item {
            Ok(PubSubEvent::Event(msg)) => {
                event_count += 1;
                use std::fmt::Write;
                let replay_hex =
                    msg.replay_id
                        .as_bytes()
                        .iter()
                        .fold(String::new(), |mut acc, b| {
                            let _ = write!(acc, "{b:02x}");
                            acc
                        });
                println!(
                    "[{event_count:>2}] event_id={} replay_id=0x{replay_hex}",
                    msg.event_id
                );
                if event_count >= 10 {
                    println!("\nReceived 10 events — done.");
                    break;
                }
            }
            Ok(PubSubEvent::Reconnected { attempt, .. }) => {
                println!("Stream reconnected (attempt {attempt})");
            }
            Ok(PubSubEvent::KeepAlive) => {
                print!(".");
            }
            Err(e) => {
                eprintln!("\nStream error: {e}");
                break;
            }
        }
    }

    Ok(())
}
