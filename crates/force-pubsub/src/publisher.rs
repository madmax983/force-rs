//! Publish event implementation.

use serde::Serialize;
use std::sync::Arc;
use tonic::transport::Channel;

use force::auth::Authenticator;
use force::session::Session;

use crate::codec::encode_avro;
use crate::error::{PubSubError, Result};
use crate::schema_cache::SchemaCache;
use crate::types::{PublishResponse, PublishResult, ReplayId};

use crate::proto::eventbus_v1::{pub_sub_client::PubSubClient, ProducerEvent, PublishRequest};

/// Encode events and publish via the unary Publish RPC.
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
        .ok_or_else(|| PubSubError::SchemaNotFound {
            schema_id: schema_id.to_string(),
        })?;

    let mut producer_events = Vec::with_capacity(events.len());
    for event in &events {
        let payload = encode_avro(&schema, event)?;
        producer_events.push(ProducerEvent {
            schema_id: schema_id.to_string(),
            payload,
        });
    }

    let token = session.token_manager().token().await?;

    let mut req = tonic::Request::new(PublishRequest {
        topic_name: topic.to_string(),
        events: producer_events,
    });

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

    let resp = PubSubClient::new(channel.clone())
        .publish(req)
        .await?
        .into_inner();

    let results = resp
        .results
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_events_vec_is_valid() {
        // No Avro encoding happens for empty events — just verify the type compiles
        let _: Vec<ProducerEvent> = Vec::new();
    }
}
