//! Subscribe stream implementation with configurable reconnection.

use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::{Stream, StreamExt};

use force::auth::Authenticator;
use force::session::Session;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tonic::transport::Channel;

use crate::codec::decode_avro;
use crate::config::{PubSubConfig, ReconnectPolicy, ReplayPreset};
use crate::error::{PubSubError, Result};
use crate::proto::eventbus_v1::{pub_sub_client::PubSubClient, FetchRequest};
use crate::schema_cache::SchemaCache;
use crate::types::{EventMessage, PubSubEvent, ReplayId};

/// Convert our [`ReplayPreset`] to the proto integer value.
#[allow(clippy::missing_const_for_fn)]
fn preset_to_proto(preset: &ReplayPreset) -> i32 {
    match preset {
        ReplayPreset::Latest => 0,
        ReplayPreset::Earliest => 1,
        ReplayPreset::Custom(_) => 2,
    }
}

/// Build a [`FetchRequest`] from config and replay preset.
pub fn build_fetch_request(topic: &str, preset: &ReplayPreset, batch_size: i32) -> FetchRequest {
    let replay_id = match preset {
        ReplayPreset::Custom(id) => id.as_bytes().to_vec(),
        _ => vec![],
    };
    FetchRequest {
        topic_name: topic.to_string(),
        replay_preset: preset_to_proto(preset),
        replay_id,
        num_requested: batch_size,
        auth_refresh: None,
    }
}

/// Internal state for one subscribe session.
struct SubscribeState<A: Authenticator> {
    session: Arc<Session<A>>,
    config: PubSubConfig,
    schema_cache: SchemaCache,
    channel: Channel,
    topic: String,
}

impl<A: Authenticator> SubscribeState<A> {
    async fn get_token(&self) -> Result<force::auth::AccessToken> {
        self.session
            .token_manager()
            .token()
            .await
            .map_err(PubSubError::Auth)
    }

    /// Open a new bidirectional Subscribe stream.
    async fn open_stream(
        &self,
        preset: &ReplayPreset,
    ) -> Result<tonic::codec::Streaming<crate::proto::eventbus_v1::FetchResponse>> {
        let token = self.get_token().await?;
        let (tx, rx) = mpsc::channel(4);
        let req_msg = build_fetch_request(&self.topic, preset, self.config.batch_size);
        tx.send(req_msg)
            .await
            .map_err(|_| PubSubError::Config("initial FetchRequest send failed".to_string()))?;

        let mut req = tonic::Request::new(ReceiverStream::new(rx));
        let metadata = req.metadata_mut();
        metadata.insert(
            "accesstoken",
            token
                .as_str()
                .parse()
                .map_err(|_| PubSubError::Config("invalid token characters".to_string()))?,
        );
        metadata.insert(
            "instanceurl",
            token
                .instance_url()
                .parse()
                .map_err(|_| PubSubError::Config("invalid instance URL characters".to_string()))?,
        );

        let response = PubSubClient::new(self.channel.clone()).subscribe(req).await?;
        Ok(response.into_inner())
    }
}

/// Run the subscribe loop, emitting `PubSubEvent<Value>` to `tx`.
#[allow(clippy::too_many_lines)]
async fn subscribe_loop<A: Authenticator + Send + Sync + 'static>(
    state: SubscribeState<A>,
    initial_preset: ReplayPreset,
    tx: mpsc::Sender<Result<PubSubEvent<Value>>>,
) {
    let mut current_preset = initial_preset;
    let mut reconnect_count: u32 = 0;

    'outer: loop {
        let mut stream = match state.open_stream(&current_preset).await {
            Ok(s) => s,
            Err(e) => {
                let _ = tx.send(Err(e)).await;
                break;
            }
        };

        loop {
            if let Ok(Some(response)) = stream.message().await {
                // Update replay position
                if !response.latest_replay_id.is_empty() {
                    current_preset = ReplayPreset::Custom(ReplayId::from_bytes(
                        response.latest_replay_id.clone(),
                    ));
                }

                if response.events.is_empty() {
                    if tx.send(Ok(PubSubEvent::KeepAlive)).await.is_err() {
                        break 'outer;
                    }
                } else {
                    for event in &response.events {
                        let Some(header) = &event.event else { continue };
                        let schema_id = &header.schema_id;
                        let replay_id = ReplayId::from_bytes(header.replay_id.clone());

                        let Some(schema) = state.schema_cache.get(schema_id) else {
                            let _ = tx
                                .send(Err(PubSubError::SchemaNotFound {
                                    schema_id: schema_id.clone(),
                                }))
                                .await;
                            continue;
                        };

                        match decode_avro(&schema, &event.payload) {
                            Ok(payload) => {
                                let msg = EventMessage {
                                    payload,
                                    replay_id,
                                    schema_id: schema_id.clone(),
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
            } else {
                // Stream ended or errored
                match &state.config.reconnect_policy {
                    ReconnectPolicy::None => {
                        let _ = tx
                            .send(Err(PubSubError::Transport(tonic::Status::unavailable(
                                "subscribe stream ended",
                            ))))
                            .await;
                        break 'outer;
                    }
                    ReconnectPolicy::Auto { max_retries, backoff } => {
                        reconnect_count += 1;
                        if reconnect_count > *max_retries {
                            let _ = tx
                                .send(Err(PubSubError::ReconnectFailed {
                                    attempts: reconnect_count,
                                    last_error: Box::new(PubSubError::Transport(
                                        tonic::Status::unavailable("max retries exceeded"),
                                    )),
                                }))
                                .await;
                            break 'outer;
                        }

                        let delay = backoff.delay_for(reconnect_count - 1);
                        tokio::time::sleep(delay).await;

                        let replay_id = match &current_preset {
                            ReplayPreset::Custom(id) => id.clone(),
                            _ => ReplayId::from_bytes(vec![]),
                        };

                        let _ = tx
                            .send(Ok(PubSubEvent::Reconnected {
                                replay_id: replay_id.clone(),
                                attempt: reconnect_count,
                            }))
                            .await;
                    }
                }
                break; // restart outer loop (reconnect)
            }
        }
    }
}

/// Subscribe to a topic, yielding decoded events as [`serde_json::Value`].
///
/// Spawns a background task that drives the gRPC subscribe stream and sends
/// decoded events through a channel. The returned stream emits
/// [`PubSubEvent<Value>`] items until the channel is closed or an unrecoverable
/// error occurs.
pub fn subscribe_dynamic<A: Authenticator + Send + Sync + 'static>(
    session: Arc<Session<A>>,
    config: PubSubConfig,
    schema_cache: SchemaCache,
    channel: Channel,
    topic: String,
    preset: ReplayPreset,
) -> Pin<Box<dyn Stream<Item = Result<PubSubEvent<Value>>> + Send>> {
    #[allow(clippy::cast_sign_loss)] // batch_size validated to 1..=100 by connect()
    let capacity = config.batch_size as usize * 2;
    let (tx, rx) = mpsc::channel(capacity);
    tokio::spawn(subscribe_loop(
        SubscribeState {
            session,
            config,
            schema_cache,
            channel,
            topic,
        },
        preset,
        tx,
    ));
    Box::pin(ReceiverStream::new(rx))
}

/// Subscribe to a topic, yielding typed events deserialized as `T`.
///
/// Internally calls [`subscribe_dynamic`] and maps each decoded
/// [`serde_json::Value`] payload to `T` via [`serde_json::from_value`].
pub fn subscribe_typed_dynamic<A, T>(
    session: Arc<Session<A>>,
    config: PubSubConfig,
    schema_cache: SchemaCache,
    channel: Channel,
    topic: String,
    preset: ReplayPreset,
) -> Pin<Box<dyn Stream<Item = Result<PubSubEvent<T>>> + Send>>
where
    A: Authenticator + Send + Sync + 'static,
    T: DeserializeOwned + Send + 'static,
{
    #[allow(clippy::cast_sign_loss)] // batch_size validated to 1..=100 by connect()
    let capacity = config.batch_size as usize * 2;
    let (tx, rx) = mpsc::channel(capacity);

    let dynamic = subscribe_dynamic(session, config, schema_cache, channel, topic, preset);

    tokio::spawn(async move {
        let mut stream = dynamic;
        while let Some(item) = stream.next().await {
            let mapped = item.and_then(|event| match event {
                PubSubEvent::Event(msg) => {
                    // Re-decode the Value to T — schema already applied; just re-deserialize.
                    serde_json::from_value::<T>(msg.payload.clone())
                        .map_err(|e| PubSubError::Avro(e.to_string()))
                        .map(|typed_payload| {
                            PubSubEvent::Event(EventMessage {
                                payload: typed_payload,
                                replay_id: msg.replay_id,
                                schema_id: msg.schema_id,
                                event_id: msg.event_id,
                            })
                        })
                }
                PubSubEvent::KeepAlive => Ok(PubSubEvent::KeepAlive),
                PubSubEvent::Reconnected { replay_id, attempt } => {
                    Ok(PubSubEvent::Reconnected { replay_id, attempt })
                }
            });
            if tx.send(mapped).await.is_err() {
                break;
            }
        }
    });

    Box::pin(ReceiverStream::new(rx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_to_proto_latest() {
        assert_eq!(preset_to_proto(&ReplayPreset::Latest), 0);
    }

    #[test]
    fn test_preset_to_proto_earliest() {
        assert_eq!(preset_to_proto(&ReplayPreset::Earliest), 1);
    }

    #[test]
    fn test_preset_to_proto_custom() {
        let id = ReplayId::from_bytes(vec![1, 2, 3]);
        assert_eq!(preset_to_proto(&ReplayPreset::Custom(id)), 2);
    }

    #[test]
    fn test_build_fetch_request_latest() {
        let req = build_fetch_request("/event/Test__e", &ReplayPreset::Latest, 50);
        assert_eq!(req.topic_name, "/event/Test__e");
        assert_eq!(req.replay_preset, 0);
        assert!(req.replay_id.is_empty());
        assert_eq!(req.num_requested, 50);
    }

    #[test]
    fn test_build_fetch_request_earliest() {
        let req = build_fetch_request("/event/Test__e", &ReplayPreset::Earliest, 10);
        assert_eq!(req.replay_preset, 1);
        assert!(req.replay_id.is_empty());
    }

    #[test]
    fn test_build_fetch_request_custom() {
        let id = ReplayId::from_bytes(vec![9, 8, 7]);
        let req = build_fetch_request("/event/Test__e", &ReplayPreset::Custom(id), 10);
        assert_eq!(req.replay_preset, 2);
        assert_eq!(req.replay_id, vec![9, 8, 7]);
    }
}
