//! 👺 Havoc: Cache Stampede (Thundering Herd) Vulnerability

#![allow(clippy::unwrap_used)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::time::Duration;
use tonic::transport::{Server, Endpoint};
use tonic::{Request, Response, Status};

use force_pubsub::proto::eventbus_v1::pub_sub_server::{PubSub, PubSubServer};
use force_pubsub::proto::eventbus_v1::{
    PublishRequest, PublishResponse, SchemaRequest, SchemaInfo, TopicRequest, TopicInfo,
    FetchRequest, FetchResponse,
};
use force_pubsub::SchemaCache;

#[derive(Default, Debug)]
struct MockPubSub {
    schema_calls: Arc<AtomicUsize>,
}

#[tonic::async_trait]
impl PubSub for MockPubSub {
    type SubscribeStream = tokio_stream::wrappers::ReceiverStream<Result<FetchResponse, Status>>;
    type PublishStreamStream = tokio_stream::wrappers::ReceiverStream<Result<PublishResponse, Status>>;

    async fn subscribe(
        &self,
        _request: Request<tonic::Streaming<FetchRequest>>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }

    async fn get_schema(
        &self,
        request: Request<SchemaRequest>,
    ) -> Result<Response<SchemaInfo>, Status> {
        self.schema_calls.fetch_add(1, Ordering::SeqCst);

        // Simulate a slow RPC call
        tokio::time::sleep(Duration::from_millis(50)).await;

        let schema_id = request.into_inner().schema_id;

        // Return a dummy valid schema
        let schema_json = r#"{
            "type": "record",
            "name": "OrderEvent",
            "fields": [
                {"name": "order_id", "type": "string"}
            ]
        }"#.to_string();

        Ok(Response::new(SchemaInfo {
            schema_id,
            schema_json,
        }))
    }

    async fn get_topic(
        &self,
        _request: Request<TopicRequest>,
    ) -> Result<Response<TopicInfo>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }

    async fn publish(
        &self,
        _request: Request<PublishRequest>,
    ) -> Result<Response<PublishResponse>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }

    async fn publish_stream(
        &self,
        _request: Request<tonic::Streaming<PublishRequest>>,
    ) -> Result<Response<Self::PublishStreamStream>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cache_stampede() {
    let mock = MockPubSub::default();
    let schema_calls = mock.schema_calls.clone();

    let (tx, rx) = tokio::sync::oneshot::channel::<()>();

    let port = 50051; // Or find an open port
    let addr = format!("127.0.0.1:{port}").parse().unwrap();

    let server = tokio::spawn(async move {
        Server::builder()
            .add_service(PubSubServer::new(mock))
            .serve_with_shutdown(addr, async {
                rx.await.ok();
            })
            .await
            .unwrap();
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    let endpoint = Endpoint::from_shared(format!("http://127.0.0.1:{port}")).unwrap();
    let channel = endpoint.connect().await.unwrap();

    let cache = SchemaCache::new();

    // Trigger stampede
    let mut handles = vec![];
    for _ in 0..10 {
        let cache_clone = cache.clone();
        let channel_clone = channel.clone();

        handles.push(tokio::spawn(async move {
            cache_clone.get_or_fetch(
                "schema-001",
                &channel_clone,
                tonic::metadata::MetadataMap::new(),
            ).await
        }));
    }

    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    tx.send(()).unwrap();
    server.await.unwrap();

    // The cache should only be populated once. If we have > 1 calls, we have a stampede.
    let calls = schema_calls.load(Ordering::SeqCst);
    assert_eq!(calls, 1, "Cache stampede detected! get_schema was called {calls} times");
}
