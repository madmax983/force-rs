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

    let results = resp
        .results
        .into_iter()
        .map(map_proto_publish_result)
        .collect();

    Ok(PublishResponse {
        topic_name: resp.topic_name,
        results,
    })
}

pub fn map_proto_publish_result(r: crate::proto::eventbus_v1::PublishResult) -> PublishResult {
    PublishResult {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_events_vec_is_valid() {
        // No Avro encoding happens for empty events — just verify the type compiles
        let _: Vec<ProducerEvent> = Vec::new();
    }
    #[test]
    fn test_publish_result_error_mapping() {
        use crate::proto::eventbus_v1::PubSubError as ProtoError;

        let proto_results = vec![
            crate::proto::eventbus_v1::PublishResult {
                replay_id: vec![],
                error: Some(ProtoError {
                    code: 0,
                    msg: String::new(),
                    key: None,
                }),
            },
            crate::proto::eventbus_v1::PublishResult {
                replay_id: vec![],
                error: Some(ProtoError {
                    code: 1,
                    msg: "Failed".to_string(),
                    key: None,
                }),
            },
            crate::proto::eventbus_v1::PublishResult {
                replay_id: vec![],
                error: None,
            },
            crate::proto::eventbus_v1::PublishResult {
                replay_id: vec![],
                error: Some(ProtoError {
                    code: 0,
                    msg: "Error msg but zero code".to_string(),
                    key: None,
                }),
            },
        ];

        let results: Vec<crate::types::PublishResult> = proto_results
            .into_iter()
            .map(super::map_proto_publish_result)
            .collect();

        assert_eq!(results[0].error, None);
        assert_eq!(results[1].error, Some("Failed".to_string()));
        assert_eq!(results[2].error, None);
        assert_eq!(
            results[3].error,
            Some("Error msg but zero code".to_string())
        );
    }
}
