//! In-process mock Pub/Sub gRPC server for testing.
#![allow(dead_code)] // Shared helper module — items used by different test binaries.
#![allow(clippy::doc_markdown)] // RPC names (GetTopic, GetSchema) need not be backtick'd.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::net::TcpListener;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use wiremock::matchers::{header_exists, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use force_pubsub::proto::eventbus_v1::{
    ConsumerEvent, EventHeader, FetchRequest, FetchResponse, PublishRequest,
    PublishResponse as ProtoPublishResponse, PublishResult as ProtoPublishResult, SchemaInfo,
    SchemaRequest, TopicInfo, TopicRequest,
    pub_sub_server::{PubSub, PubSubServer},
};

/// Simple mock Pub/Sub gRPC server.
pub struct MockPubSubService {
    /// Schema ID returned by GetTopic.
    pub topic_schema_id: String,
    /// Avro schema JSON returned by GetSchema.
    pub schema_json: String,
}

impl Default for MockPubSubService {
    fn default() -> Self {
        Self {
            topic_schema_id: "schema-test-001".to_string(),
            schema_json:
                r#"{"type":"record","name":"TestEvent","fields":[{"name":"id","type":"string"}]}"#
                    .to_string(),
        }
    }
}

#[tonic::async_trait]
impl PubSub for MockPubSubService {
    type SubscribeStream = ReceiverStream<Result<FetchResponse, Status>>;
    type PublishStreamStream = ReceiverStream<Result<ProtoPublishResponse, Status>>;

    async fn get_topic(&self, req: Request<TopicRequest>) -> Result<Response<TopicInfo>, Status> {
        let name = req.into_inner().topic_name;
        Ok(Response::new(TopicInfo {
            topic_name: name.clone(),
            topic_uri: name,
            can_publish: true,
            can_subscribe: true,
            schema_id: self.topic_schema_id.clone(),
        }))
    }

    async fn get_schema(
        &self,
        req: Request<SchemaRequest>,
    ) -> Result<Response<SchemaInfo>, Status> {
        let id = req.into_inner().schema_id;
        if id == self.topic_schema_id {
            Ok(Response::new(SchemaInfo {
                schema_id: id,
                schema_json: self.schema_json.clone(),
            }))
        } else {
            Err(Status::not_found(format!("schema {id} not found")))
        }
    }

    async fn subscribe(
        &self,
        _req: Request<tonic::Streaming<FetchRequest>>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        drop(tx); // immediately end stream — subscribe tests handle this separately
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn publish(
        &self,
        _req: Request<PublishRequest>,
    ) -> Result<Response<ProtoPublishResponse>, Status> {
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
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tokio::spawn(async move {
            let _ = tx
                .send(Err(Status::unavailable("always unavailable")))
                .await;
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

/// A mock Pub/Sub service that echoes `PublishStream` requests back as responses.
///
/// For every `PublishRequest` received on the input stream, it sends back a
/// `PublishResponse` with one `PublishResult` per event, carrying a trivial
/// replay ID of `[1]`.
pub struct EchoPublishStreamService {
    /// Schema ID returned by GetTopic / GetSchema.
    pub topic_schema_id: String,
    /// Avro schema JSON.
    pub schema_json: String,
}

impl Default for EchoPublishStreamService {
    fn default() -> Self {
        Self {
            topic_schema_id: "schema-test-001".to_string(),
            schema_json:
                r#"{"type":"record","name":"TestEvent","fields":[{"name":"id","type":"string"}]}"#
                    .to_string(),
        }
    }
}

#[tonic::async_trait]
impl PubSub for EchoPublishStreamService {
    type SubscribeStream = ReceiverStream<Result<FetchResponse, Status>>;
    type PublishStreamStream = ReceiverStream<Result<ProtoPublishResponse, Status>>;

    async fn get_topic(&self, req: Request<TopicRequest>) -> Result<Response<TopicInfo>, Status> {
        let name = req.into_inner().topic_name;
        Ok(Response::new(TopicInfo {
            topic_name: name.clone(),
            topic_uri: name,
            can_publish: true,
            can_subscribe: true,
            schema_id: self.topic_schema_id.clone(),
        }))
    }

    async fn get_schema(
        &self,
        req: Request<SchemaRequest>,
    ) -> Result<Response<SchemaInfo>, Status> {
        let id = req.into_inner().schema_id;
        if id == self.topic_schema_id {
            Ok(Response::new(SchemaInfo {
                schema_id: id,
                schema_json: self.schema_json.clone(),
            }))
        } else {
            Err(Status::not_found(format!("schema {id} not found")))
        }
    }

    async fn subscribe(
        &self,
        _req: Request<tonic::Streaming<FetchRequest>>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        drop(tx);
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn publish(
        &self,
        req: Request<PublishRequest>,
    ) -> Result<Response<ProtoPublishResponse>, Status> {
        let inner = req.into_inner();
        let results = inner
            .events
            .iter()
            .map(|_| ProtoPublishResult {
                replay_id: vec![1u8],
                error: None,
            })
            .collect();
        Ok(Response::new(ProtoPublishResponse {
            topic_name: inner.topic_name,
            results,
            rpc_id: None,
        }))
    }

    async fn publish_stream(
        &self,
        req: Request<tonic::Streaming<PublishRequest>>,
    ) -> Result<Response<Self::PublishStreamStream>, Status> {
        let (resp_tx, resp_rx) = tokio::sync::mpsc::channel(32);
        let mut in_stream = req.into_inner();

        tokio::spawn(async move {
            use tokio_stream::StreamExt as _;
            while let Some(Ok(publish_req)) = in_stream.next().await {
                let results = publish_req
                    .events
                    .iter()
                    .map(|_| ProtoPublishResult {
                        replay_id: vec![1u8],
                        error: None,
                    })
                    .collect();
                let response = ProtoPublishResponse {
                    topic_name: publish_req.topic_name,
                    results,
                    rpc_id: None,
                };
                if resp_tx.send(Ok(response)).await.is_err() {
                    break;
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(resp_rx)))
    }
}

/// Start an in-process echo publish-stream mock server.
/// Returns the base URL (e.g., `http://127.0.0.1:PORT`).
pub async fn start_echo_stream_server(service: EchoPublishStreamService) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let stream = TcpListenerStream::new(listener);

    tokio::spawn(async move {
        if let Err(e) = Server::builder()
            .add_service(PubSubServer::new(service))
            .serve_with_incoming(stream)
            .await
        {
            eprintln!("echo stream Pub/Sub server error: {e}");
        }
    });

    format!("http://{addr}")
}

/// Start an in-process mock server. Returns the base URL (e.g., `http://127.0.0.1:12345`).
pub async fn start_mock_server(service: MockPubSubService) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let stream = TcpListenerStream::new(listener);

    tokio::spawn(async move {
        if let Err(e) = Server::builder()
            .add_service(PubSubServer::new(service))
            .serve_with_incoming(stream)
            .await
        {
            eprintln!("mock Pub/Sub server error: {e}");
        }
    });

    format!("http://{addr}")
}

// ─── Helpers for building FetchResponse / ConsumerEvent in tests ──────────────

/// Build a `ConsumerEvent` with the given schema_id, replay_id bytes, and Avro payload.
pub fn make_consumer_event(schema_id: &str, replay_id: Vec<u8>, payload: Vec<u8>) -> ConsumerEvent {
    ConsumerEvent {
        event: Some(EventHeader {
            schema_id: schema_id.to_string(),
            replay_id,
            producer_partition_key: "test-partition".to_string(),
            headers: std::collections::HashMap::new(),
        }),
        payload,
    }
}

/// Build a `FetchResponse` containing the given events.
pub fn make_fetch_response(
    topic: &str,
    events: Vec<ConsumerEvent>,
    latest_replay_id: Vec<u8>,
) -> FetchResponse {
    FetchResponse {
        topic_name: topic.to_string(),
        latest_replay_id,
        events,
        pending_num_requested: 0,
        rpc_id: Some("test-rpc-id".to_string()),
    }
}

/// Build a keepalive `FetchResponse` (empty events list, non-empty replay ID).
pub fn make_keepalive_response(topic: &str, latest_replay_id: Vec<u8>) -> FetchResponse {
    make_fetch_response(topic, vec![], latest_replay_id)
}

// ─── EventStreamService ───────────────────────────────────────────────────────

/// A mock Pub/Sub service that sends a pre-configured list of `FetchResponse`
/// messages, then closes the stream (for event-delivery integration tests).
pub struct EventStreamService {
    /// Schema ID returned by GetTopic / GetSchema.
    pub topic_schema_id: String,
    /// Avro schema JSON returned by GetSchema.
    pub schema_json: String,
    /// Responses to send to the first (and only) subscriber, in order.
    pub responses: Vec<FetchResponse>,
}

#[tonic::async_trait]
impl PubSub for EventStreamService {
    type SubscribeStream = ReceiverStream<Result<FetchResponse, Status>>;
    type PublishStreamStream = ReceiverStream<Result<ProtoPublishResponse, Status>>;

    async fn get_topic(&self, req: Request<TopicRequest>) -> Result<Response<TopicInfo>, Status> {
        let name = req.into_inner().topic_name;
        Ok(Response::new(TopicInfo {
            topic_name: name.clone(),
            topic_uri: name,
            can_publish: true,
            can_subscribe: true,
            schema_id: self.topic_schema_id.clone(),
        }))
    }

    async fn get_schema(
        &self,
        req: Request<SchemaRequest>,
    ) -> Result<Response<SchemaInfo>, Status> {
        let id = req.into_inner().schema_id;
        if id == self.topic_schema_id {
            Ok(Response::new(SchemaInfo {
                schema_id: id,
                schema_json: self.schema_json.clone(),
            }))
        } else {
            Err(Status::not_found(format!("schema {id} not found")))
        }
    }

    async fn subscribe(
        &self,
        _req: Request<tonic::Streaming<FetchRequest>>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let (tx, rx) = tokio::sync::mpsc::channel(self.responses.len().max(1));
        let responses = self.responses.clone();
        tokio::spawn(async move {
            for resp in responses {
                if tx.send(Ok(resp)).await.is_err() {
                    return;
                }
            }
            // Drop tx → stream ends cleanly (None returned by stream.message())
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn publish(
        &self,
        _req: Request<PublishRequest>,
    ) -> Result<Response<ProtoPublishResponse>, Status> {
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
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tokio::spawn(async move {
            let _ = tx
                .send(Err(Status::unavailable("always unavailable")))
                .await;
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

/// Start an `EventStreamService` gRPC mock server.
/// Returns the base URL (e.g., `http://127.0.0.1:PORT`).
pub async fn start_event_stream_server(service: EventStreamService) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let stream = TcpListenerStream::new(listener);
    tokio::spawn(async move {
        if let Err(e) = Server::builder()
            .add_service(PubSubServer::new(service))
            .serve_with_incoming(stream)
            .await
        {
            eprintln!("event stream Pub/Sub server error: {e}");
        }
    });
    format!("http://{addr}")
}

// ─── ReconnectingStreamService ────────────────────────────────────────────────

/// A mock Pub/Sub service that:
/// - On the **first** connection: sends `first_responses`, then returns
///   `Status::unavailable` (simulating a disconnect).
/// - On **subsequent** connections: sends `second_responses`, then closes cleanly.
///
/// Uses an `Arc<AtomicU32>` to count connections across instances.
pub struct ReconnectingStreamService {
    /// Schema ID returned by GetTopic / GetSchema.
    pub topic_schema_id: String,
    /// Avro schema JSON returned by GetSchema.
    pub schema_json: String,
    /// Responses sent on the first connection, followed by a transport error.
    pub first_responses: Vec<FetchResponse>,
    /// Responses sent on subsequent connections; stream closes cleanly after.
    pub second_responses: Vec<FetchResponse>,
    /// Shared connection counter (incremented on each Subscribe call).
    pub connection_count: Arc<AtomicU32>,
}

#[tonic::async_trait]
impl PubSub for ReconnectingStreamService {
    type SubscribeStream = ReceiverStream<Result<FetchResponse, Status>>;
    type PublishStreamStream = ReceiverStream<Result<ProtoPublishResponse, Status>>;

    async fn get_topic(&self, req: Request<TopicRequest>) -> Result<Response<TopicInfo>, Status> {
        let name = req.into_inner().topic_name;
        Ok(Response::new(TopicInfo {
            topic_name: name.clone(),
            topic_uri: name,
            can_publish: true,
            can_subscribe: true,
            schema_id: self.topic_schema_id.clone(),
        }))
    }

    async fn get_schema(
        &self,
        req: Request<SchemaRequest>,
    ) -> Result<Response<SchemaInfo>, Status> {
        let id = req.into_inner().schema_id;
        if id == self.topic_schema_id {
            Ok(Response::new(SchemaInfo {
                schema_id: id,
                schema_json: self.schema_json.clone(),
            }))
        } else {
            Err(Status::not_found(format!("schema {id} not found")))
        }
    }

    async fn subscribe(
        &self,
        _req: Request<tonic::Streaming<FetchRequest>>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let conn_num = self.connection_count.fetch_add(1, Ordering::SeqCst) + 1;

        if conn_num == 1 {
            // First connection: send responses, then error
            let (tx, rx) = tokio::sync::mpsc::channel(self.first_responses.len().max(1) + 1);
            let responses = self.first_responses.clone();
            tokio::spawn(async move {
                for resp in responses {
                    if tx.send(Ok(resp)).await.is_err() {
                        return;
                    }
                }
                // Send a terminal error to trigger reconnect
                let _ = tx
                    .send(Err(Status::unavailable("simulated disconnect")))
                    .await;
            });
            Ok(Response::new(ReceiverStream::new(rx)))
        } else {
            // Second+ connection: send responses, then close cleanly
            let (tx, rx) = tokio::sync::mpsc::channel(self.second_responses.len().max(1));
            let responses = self.second_responses.clone();
            tokio::spawn(async move {
                for resp in responses {
                    if tx.send(Ok(resp)).await.is_err() {
                        return;
                    }
                }
                // Drop tx → clean close
            });
            Ok(Response::new(ReceiverStream::new(rx)))
        }
    }

    async fn publish(
        &self,
        _req: Request<PublishRequest>,
    ) -> Result<Response<ProtoPublishResponse>, Status> {
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
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tokio::spawn(async move {
            let _ = tx
                .send(Err(Status::unavailable("always unavailable")))
                .await;
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

/// Start a `ReconnectingStreamService` gRPC mock server.
/// Returns the base URL and the shared connection counter so tests can inspect it.
pub async fn start_reconnecting_stream_server(service: ReconnectingStreamService) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let stream = TcpListenerStream::new(listener);
    tokio::spawn(async move {
        if let Err(e) = Server::builder()
            .add_service(PubSubServer::new(service))
            .serve_with_incoming(stream)
            .await
        {
            eprintln!("reconnecting stream Pub/Sub server error: {e}");
        }
    });
    format!("http://{addr}")
}

// ─── AlwaysErrorService ───────────────────────────────────────────────────────

/// A mock Pub/Sub service whose `subscribe` always immediately returns
/// `Status::unavailable`, used for testing retry exhaustion.
pub struct AlwaysErrorService {
    /// Schema ID returned by GetTopic / GetSchema.
    pub topic_schema_id: String,
    /// Avro schema JSON.
    pub schema_json: String,
}

impl Default for AlwaysErrorService {
    fn default() -> Self {
        Self {
            topic_schema_id: "schema-test-001".to_string(),
            schema_json:
                r#"{"type":"record","name":"TestEvent","fields":[{"name":"field","type":"string"}]}"#
                    .to_string(),
        }
    }
}

#[tonic::async_trait]
impl PubSub for AlwaysErrorService {
    type SubscribeStream = ReceiverStream<Result<FetchResponse, Status>>;
    type PublishStreamStream = ReceiverStream<Result<ProtoPublishResponse, Status>>;

    async fn get_topic(&self, req: Request<TopicRequest>) -> Result<Response<TopicInfo>, Status> {
        let name = req.into_inner().topic_name;
        Ok(Response::new(TopicInfo {
            topic_name: name.clone(),
            topic_uri: name,
            can_publish: true,
            can_subscribe: true,
            schema_id: self.topic_schema_id.clone(),
        }))
    }

    async fn get_schema(
        &self,
        req: Request<SchemaRequest>,
    ) -> Result<Response<SchemaInfo>, Status> {
        let id = req.into_inner().schema_id;
        if id == self.topic_schema_id {
            Ok(Response::new(SchemaInfo {
                schema_id: id,
                schema_json: self.schema_json.clone(),
            }))
        } else {
            Err(Status::not_found(format!("schema {id} not found")))
        }
    }

    async fn subscribe(
        &self,
        _req: Request<tonic::Streaming<FetchRequest>>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        // Immediately error the stream
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tokio::spawn(async move {
            let _ = tx
                .send(Err(Status::unavailable("always unavailable")))
                .await;
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn publish(
        &self,
        _req: Request<PublishRequest>,
    ) -> Result<Response<ProtoPublishResponse>, Status> {
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
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tokio::spawn(async move {
            let _ = tx
                .send(Err(Status::unavailable("always unavailable")))
                .await;
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

/// Start an `AlwaysErrorService` gRPC mock server.
pub async fn start_always_error_server(service: AlwaysErrorService) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let stream = TcpListenerStream::new(listener);
    tokio::spawn(async move {
        if let Err(e) = Server::builder()
            .add_service(PubSubServer::new(service))
            .serve_with_incoming(stream)
            .await
        {
            eprintln!("always error Pub/Sub server error: {e}");
        }
    });
    format!("http://{addr}")
}

// ─── Wiremock userinfo helper ──────────────────────────────────────────────────

/// Start a wiremock HTTP server that responds to `/services/oauth2/userinfo` with a
/// fake org ID. Returns the server (keep alive for the test duration) and its base URL.
///
/// The returned `MockServer` must be kept alive for the duration of the test.
pub async fn start_userinfo_mock(org_id: &str) -> (MockServer, String) {
    let server = MockServer::start().await;
    let body = format!(
        r#"{{"sub":"https://login.salesforce.com/id/00D000000000001EAA/005000000000001AAA","user_id":"005000000000001AAA","organization_id":"{org_id}","username":"test@example.com"}}"#
    );
    Mock::given(method("GET"))
        .and(path("/services/oauth2/userinfo"))
        .and(header_exists("authorization"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_string(body),
        )
        .mount(&server)
        .await;
    let url = server.uri();
    (server, url)
}
