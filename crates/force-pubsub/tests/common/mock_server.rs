//! In-process mock Pub/Sub gRPC server for testing.

use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

use force_pubsub::proto::eventbus_v1::{
    FetchRequest, FetchResponse, PublishRequest, PublishResponse as ProtoPublishResponse,
    SchemaInfo, SchemaRequest, TopicInfo, TopicRequest,
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
        drop(tx);
        Ok(Response::new(ReceiverStream::new(rx)))
    }
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
