#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::uninlined_format_args)]
#![allow(missing_docs)]

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::Barrier;
use force_pubsub::SchemaCache;
use force_pubsub::proto::eventbus_v1::{SchemaRequest, SchemaInfo, pub_sub_server::{PubSub, PubSubServer}};
use tokio_stream::wrappers::ReceiverStream;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::{Server, Channel};
use tonic::{Request, Response, Status};

struct StampedeMockPubSub {
    call_count: Arc<AtomicU32>,
}

#[tonic::async_trait]
impl PubSub for StampedeMockPubSub {
    type SubscribeStream = ReceiverStream<Result<force_pubsub::proto::eventbus_v1::FetchResponse, Status>>;
    type PublishStreamStream = ReceiverStream<Result<force_pubsub::proto::eventbus_v1::PublishResponse, Status>>;

    async fn get_schema(&self, req: Request<SchemaRequest>) -> Result<Response<SchemaInfo>, Status> {
        self.call_count.fetch_add(1, Ordering::SeqCst);

        // Add a small delay to ensure the race condition occurs
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let id = req.into_inner().schema_id;
        Ok(Response::new(SchemaInfo {
            schema_id: id,
            schema_json: r#"{"type":"record","name":"TestEvent","fields":[{"name":"id","type":"string"}]}"#.to_string(),
        }))
    }

    async fn get_topic(&self, _req: Request<force_pubsub::proto::eventbus_v1::TopicRequest>) -> Result<Response<force_pubsub::proto::eventbus_v1::TopicInfo>, Status> {
        Err(Status::unimplemented(""))
    }

    async fn subscribe(&self, _req: Request<tonic::Streaming<force_pubsub::proto::eventbus_v1::FetchRequest>>) -> Result<Response<Self::SubscribeStream>, Status> {
        Err(Status::unimplemented(""))
    }

    async fn publish(&self, _req: Request<force_pubsub::proto::eventbus_v1::PublishRequest>) -> Result<Response<force_pubsub::proto::eventbus_v1::PublishResponse>, Status> {
        Err(Status::unimplemented(""))
    }

    async fn publish_stream(&self, _req: Request<tonic::Streaming<force_pubsub::proto::eventbus_v1::PublishRequest>>) -> Result<Response<Self::PublishStreamStream>, Status> {
        Err(Status::unimplemented(""))
    }
}

#[tokio::test]
async fn test_schema_cache_stampede() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let call_count = Arc::new(AtomicU32::new(0));
    let service = StampedeMockPubSub { call_count: call_count.clone() };

    tokio::spawn(async move {
        Server::builder()
            .add_service(PubSubServer::new(service))
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await
            .unwrap();
    });

    // Give the server a moment to start
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let endpoint = Channel::from_shared(format!("http://{addr}")).unwrap();
    let channel = endpoint.connect().await.unwrap();
    let schema_cache = Arc::new(SchemaCache::new());

    // Spawn 100 concurrent tasks
    let num_tasks = 100;
    let barrier = Arc::new(Barrier::new(num_tasks));
    let mut handles = vec![];

    for _ in 0..num_tasks {
        let b = barrier.clone();
        let cache = schema_cache.clone();
        let ch = channel.clone();

        handles.push(tokio::spawn(async move {
            b.wait().await;
            cache.get_or_fetch("schema-123", &ch, tonic::metadata::MetadataMap::new()).await.unwrap();
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let actual_calls = call_count.load(Ordering::SeqCst);

    assert_eq!(
        actual_calls, 1,
        "👺 Havoc: get_or_fetch triggered a stampede! Concurrent cache misses caused {actual_calls} RPC calls instead of 1."
    );
}
