//! Publish event implementation.

use serde::Serialize;
use std::sync::Arc;
use tonic::transport::Channel;

use force::auth::Authenticator;
use force::session::Session;

use crate::codec::encode_avro;
use crate::error::{PubSubError, Result};
use crate::interceptor;
use crate::schema_cache::SchemaCache;
use crate::types::{PublishResponse, PublishResult, ReplayId};

use crate::proto::eventbus_v1::{ProducerEvent, PublishRequest, pub_sub_client::PubSubClient};

/// Encode events and publish via the unary Publish RPC.
///
/// `tenant_id` must be the 18-char Salesforce org ID, used as the `tenantid` gRPC header.
pub async fn publish_unary<A, T>(
    session: &Arc<Session<A>>,
    channel: &Channel,
    schema_cache: &SchemaCache,
    schema_id: &str,
    topic: &str,
    events: Vec<T>,
    tenant_id: &str,
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
    let meta = interceptor::build_metadata(&token, token.instance_url(), tenant_id)?;

    let mut req = tonic::Request::new(PublishRequest {
        topic_name: topic.to_string(),
        events: producer_events,
    });
    *req.metadata_mut() = meta;

    let resp = PubSubClient::new(channel.clone())
        .publish(req)
        .await?
        .into_inner();

    let results = map_publish_results(resp.results);

    Ok(PublishResponse {
        topic_name: resp.topic_name,
        results,
    })
}

/// Extracted mapping logic for unit testing. Maps the proto results to the domain results.
pub fn map_publish_results(
    proto_results: Vec<crate::proto::eventbus_v1::PublishResult>,
) -> Vec<PublishResult> {
    proto_results
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
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::eventbus_v1::{
        PubSubError as ProtoError, PublishResult as ProtoPublishResult,
    };

    #[test]
    fn test_empty_events_vec_is_valid() {
        // No Avro encoding happens for empty events — just verify the type compiles
        let _: Vec<ProducerEvent> = Vec::new();
    }

    #[test]
    fn test_publish_unary_error_mapping() {
        let proto_results = vec![
            // Success case: code 0 and empty msg
            ProtoPublishResult {
                replay_id: vec![1, 2, 3],
                error: Some(ProtoError {
                    code: 0,
                    msg: String::new(),
                    key: None,
                }),
            },
            // Error case: code 1 and non-empty msg
            ProtoPublishResult {
                replay_id: vec![],
                error: Some(ProtoError {
                    code: 1,
                    msg: "Some error".to_string(),
                    key: None,
                }),
            },
            // Edge case: no error object
            ProtoPublishResult {
                replay_id: vec![4, 5, 6],
                error: None,
            },
        ];

        let results = map_publish_results(proto_results);

        assert_eq!(results.len(), 3);

        // First is success
        let Some(replay1) = results[0].replay_id.as_ref() else {
            panic!("expected replay_id");
        };
        assert_eq!(replay1.as_bytes(), &[1, 2, 3]);
        assert_eq!(results[0].error, None);

        // Second is error
        assert_eq!(results[1].replay_id, None);
        assert_eq!(results[1].error.as_deref(), Some("Some error"));

        // Third is success (no error object)
        let Some(replay3) = results[2].replay_id.as_ref() else {
            panic!("expected replay_id");
        };
        assert_eq!(replay3.as_bytes(), &[4, 5, 6]);
        assert_eq!(results[2].error, None);
    }
}
