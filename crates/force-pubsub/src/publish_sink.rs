//! Bidirectional streaming publish sink.
//!
//! [`PublishSink`] wraps the `PublishStream` gRPC bidirectional streaming RPC,
//! providing an ergonomic API for sending batches of Avro-encoded events and
//! receiving per-batch publish responses asynchronously.

use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;

use serde::Serialize;
use tokio::sync::mpsc;
use tokio_stream::{Stream, StreamExt, wrappers::ReceiverStream};
use tonic::transport::Channel;

use force::auth::Authenticator;
use force::session::Session;

use crate::codec::encode_avro;
use crate::error::{PubSubError, Result};
use crate::interceptor;
use crate::proto::eventbus_v1::{ProducerEvent, PublishRequest, pub_sub_client::PubSubClient};
use crate::schema_cache::SchemaCache;
use crate::types::{PublishResponse, PublishResult, ReplayId};

/// Internal trait to abstract token + instance_url retrieval without
/// carrying the full generic `A: Authenticator` parameter on `PublishSink`.
#[async_trait::async_trait]
trait TokenGetter {
    async fn get_token(&self) -> Result<force::auth::AccessToken>;
}

struct SessionTokenGetter<A: Authenticator> {
    session: Arc<Session<A>>,
}

#[async_trait::async_trait]
impl<A: Authenticator + Send + Sync + 'static> TokenGetter for SessionTokenGetter<A> {
    async fn get_token(&self) -> Result<force::auth::AccessToken> {
        self.session
            .token_manager()
            .token()
            .await
            .map_err(PubSubError::Auth)
    }
}

/// A bidirectional streaming publish sink for the Salesforce Pub/Sub API.
///
/// Created by [`crate::handler::PubSubHandler::publish_stream`]. Holds an open
/// gRPC `PublishStream` channel and allows callers to send multiple batches of
/// events, streaming publish acknowledgements back.
///
/// # Type parameter
///
/// `T` is the event payload type. It must implement [`serde::Serialize`] so that
/// payloads can be Avro-encoded before transmission.
///
/// # Example
///
/// ```ignore
/// let mut sink = handler.publish_stream::<MyEvent>("/event/MyEvent__e").await?;
///
/// sink.send("schema-id", vec![MyEvent { id: "e1".into() }]).await?;
/// sink.send("schema-id", vec![MyEvent { id: "e2".into() }]).await?;
///
/// // Drain acknowledgement responses
/// let mut acks = sink.responses();
/// while let Some(resp) = acks.next().await {
///     let r = resp?;
///     println!("acked {} event(s) on {}", r.results.len(), r.topic_name);
/// }
/// sink.close().await?;
/// ```
pub struct PublishSink<T> {
    /// Sender half of the mpsc channel feeding the gRPC request stream.
    sender: mpsc::Sender<PublishRequest>,
    /// Boxed response stream, mapping proto messages to domain types.
    resp_stream: Pin<Box<dyn Stream<Item = Result<PublishResponse>> + Send>>,
    /// Schema cache shared with the handler.
    schema_cache: SchemaCache,
    /// gRPC channel for fetching schemas on demand.
    channel: Channel,
    /// Token manager reference for fresh auth tokens.
    session_token_getter: Arc<dyn TokenGetter + Send + Sync>,
    /// Pre-fetched 18-char org ID.
    tenant_id: String,
    /// The topic name this sink is publishing to.
    topic: String,
    _phantom: PhantomData<T>,
}

impl<T: Serialize + Send + 'static> PublishSink<T> {
    /// Build a `PublishSink` from its constituent parts.
    ///
    /// Called exclusively by [`crate::handler::PubSubHandler::publish_stream`].
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new<A: Authenticator + Send + Sync + 'static>(
        sender: mpsc::Sender<PublishRequest>,
        resp_stream: Pin<Box<dyn Stream<Item = Result<PublishResponse>> + Send>>,
        schema_cache: SchemaCache,
        channel: Channel,
        session: Arc<Session<A>>,
        tenant_id: String,
        topic: String,
    ) -> Self {
        Self {
            sender,
            resp_stream,
            schema_cache,
            channel,
            session_token_getter: Arc::new(SessionTokenGetter { session }),
            tenant_id,
            topic,
            _phantom: PhantomData,
        }
    }

    /// Encode a batch of events and send them to the open `PublishStream`.
    ///
    /// The Avro schema is resolved via the schema cache (or fetched from the
    /// `GetSchema` RPC on a miss). Subsequent calls reuse the cached schema.
    ///
    /// # Errors
    ///
    /// - [`PubSubError::Avro`] if an event cannot be Avro-encoded.
    /// - [`PubSubError::Transport`] if the `GetSchema` RPC fails on a cache miss.
    /// - [`PubSubError::Config`] if the channel to the gRPC stream is closed.
    pub async fn send(&mut self, schema_id: &str, events: Vec<T>) -> Result<()> {
        // Fetch schema (cache hit is O(1)).
        let token = self.session_token_getter.get_token().await?;
        let meta = interceptor::build_metadata(&token, token.instance_url(), &self.tenant_id)?;
        let schema = self
            .schema_cache
            .get_or_fetch(schema_id, &self.channel, meta)
            .await?;

        // Encode each event to Avro bytes.
        let mut producer_events = Vec::with_capacity(events.len());
        for event in &events {
            let payload = encode_avro(&schema, event)?;
            producer_events.push(ProducerEvent {
                schema_id: schema_id.to_string(),
                payload,
            });
        }

        let request = PublishRequest {
            topic_name: self.topic.clone(),
            events: producer_events,
        };

        self.sender.send(request).await.map_err(|_| {
            PubSubError::Config(
                "PublishStream channel closed — server may have terminated the stream".to_string(),
            )
        })
    }

    /// Return a reference to the server acknowledgement response stream.
    ///
    /// Each item is a [`PublishResponse`] containing per-event results for the
    /// most recently acknowledged batch.
    ///
    /// # Errors
    ///
    /// Items may be `Err(PubSubError::Transport)` if the gRPC stream reports
    /// an error.
    pub fn responses(&mut self) -> &mut (impl Stream<Item = Result<PublishResponse>> + '_) {
        &mut self.resp_stream
    }

    /// Close the sink.
    ///
    /// Drops the sender side of the mpsc channel, which signals to tonic that
    /// the client input stream is complete. Then drains any remaining server
    /// acknowledgement responses so the gRPC stream shuts down cleanly.
    ///
    /// # Errors
    ///
    /// Returns the first transport error encountered while draining responses,
    /// if any.
    pub async fn close(mut self) -> Result<()> {
        // Drop sender → closes mpsc → tonic stream sees EOF.
        drop(self.sender);

        // Drain remaining responses.
        while let Some(item) = self.resp_stream.next().await {
            item?;
        }

        Ok(())
    }
}

/// Map a single proto `PublishResponse` to the domain `PublishResponse`.
fn map_proto_response(proto_resp: crate::proto::eventbus_v1::PublishResponse) -> PublishResponse {
    let results = proto_resp
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
    PublishResponse {
        topic_name: proto_resp.topic_name,
        results,
    }
}

/// Open a `PublishStream` RPC and return a [`PublishSink`].
///
/// Called by [`crate::handler::PubSubHandler::publish_stream`] after
/// auth setup.
pub async fn open_publish_stream<A, T>(
    session: Arc<Session<A>>,
    channel: Channel,
    schema_cache: SchemaCache,
    tenant_id: String,
    topic: String,
    token: &force::auth::AccessToken,
) -> Result<PublishSink<T>>
where
    A: Authenticator + Send + Sync + 'static,
    T: Serialize + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<PublishRequest>(32);
    let meta = interceptor::build_metadata(token, token.instance_url(), &tenant_id)?;

    let mut req = tonic::Request::new(ReceiverStream::new(rx));
    *req.metadata_mut() = meta;

    let streaming = PubSubClient::new(channel.clone())
        .publish_stream(req)
        .await?
        .into_inner();

    // Convert the tonic Streaming<ProtoPublishResponse> into a pinned boxed
    // domain stream so PublishSink doesn't need to be generic over Streaming.
    let resp_stream: Pin<Box<dyn Stream<Item = Result<PublishResponse>> + Send>> =
        Box::pin(streaming.map(|item| match item {
            Ok(proto_resp) => Ok(map_proto_response(proto_resp)),
            Err(status) => Err(PubSubError::Transport(status)),
        }));

    Ok(PublishSink::new(
        tx,
        resp_stream,
        schema_cache,
        channel,
        session,
        tenant_id,
        topic,
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_map_proto_response_success() {
        use crate::proto::eventbus_v1::{
            PublishResponse as ProtoResp, PublishResult as ProtoResult,
        };
        let proto = ProtoResp {
            topic_name: "/event/Test__e".to_string(),
            results: vec![ProtoResult {
                replay_id: vec![1, 2, 3],
                error: None,
            }],
            rpc_id: None,
        };
        let resp = map_proto_response(proto);
        assert_eq!(resp.topic_name, "/event/Test__e");
        assert_eq!(resp.results.len(), 1);
        assert!(resp.results[0].is_success());
        assert_eq!(
            resp.results[0].replay_id.as_ref().unwrap().as_bytes(),
            &[1, 2, 3]
        );
    }

    #[test]
    fn test_map_proto_response_error_result() {
        use crate::proto::eventbus_v1::{
            PubSubError as ProtoErr, PublishResponse as ProtoResp, PublishResult as ProtoResult,
        };
        let proto = ProtoResp {
            topic_name: "/event/Test__e".to_string(),
            results: vec![ProtoResult {
                replay_id: vec![],
                error: Some(ProtoErr {
                    code: 1,
                    msg: "INVALID_PAYLOAD".to_string(),
                    key: None,
                }),
            }],
            rpc_id: None,
        };
        let resp = map_proto_response(proto);
        assert!(!resp.results[0].is_success());
        assert_eq!(resp.results[0].error.as_deref(), Some("INVALID_PAYLOAD"));
    }

    #[test]
    fn test_publish_result_success_is_success() {
        let r = PublishResult {
            replay_id: Some(ReplayId::from_bytes(vec![1, 2, 3])),
            error: None,
        };
        assert!(r.is_success());
    }

    #[test]
    fn test_publish_result_error_is_not_success() {
        let r = PublishResult {
            replay_id: None,
            error: Some("PUBLISH_ERROR".to_string()),
        };
        assert!(!r.is_success());
    }
}
