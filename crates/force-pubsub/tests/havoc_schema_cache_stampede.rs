//! 👺 Havoc: Schema Cache Stampede
//!
//! **The Trigger:** Multiple concurrent requests for the same schema miss the cache at the same time.
//! **The Stack Trace:** No panic, but we'll assert that the RPC is called multiple times.
//! **Reproduction:** Run `cargo test -p force-pubsub --test havoc_schema_cache_stampede`
//! **Comment:** You assumed the cache would prevent stampedes. You were wrong.

#![allow(clippy::unwrap_used)]
#![allow(clippy::uninlined_format_args)]

use force_pubsub::SchemaCache;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{Request, Response, Status, transport::Server};

use force_pubsub::proto::eventbus_v1::pub_sub_server::{PubSub, PubSubServer};
use force_pubsub::proto::eventbus_v1::{
    FetchRequest, FetchResponse, PublishRequest, PublishResponse, SchemaInfo, SchemaRequest,
    TopicInfo, TopicRequest,
};

#[derive(Clone)]
struct MockPubSub {
    schema_calls: Arc<AtomicUsize>,
}

#[tonic::async_trait]
impl PubSub for MockPubSub {
    type SubscribeStream = tokio_stream::wrappers::ReceiverStream<Result<FetchResponse, Status>>;
    type PublishStreamStream =
        tokio_stream::wrappers::ReceiverStream<Result<PublishResponse, Status>>;

    async fn get_topic(&self, _req: Request<TopicRequest>) -> Result<Response<TopicInfo>, Status> {
        unimplemented!()
    }

    async fn get_schema(
        &self,
        req: Request<SchemaRequest>,
    ) -> Result<Response<SchemaInfo>, Status> {
        self.schema_calls.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let schema_id = req.into_inner().schema_id;
        let schema_json = r#"{"type":"record","name":"Test","fields":[]}"#.to_string();
        Ok(Response::new(SchemaInfo {
            schema_id,
            schema_json,
        }))
    }

    async fn subscribe(
        &self,
        _req: Request<tonic::Streaming<FetchRequest>>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        unimplemented!()
    }

    async fn publish(
        &self,
        _req: Request<PublishRequest>,
    ) -> Result<Response<PublishResponse>, Status> {
        unimplemented!()
    }

    async fn publish_stream(
        &self,
        _req: Request<tonic::Streaming<PublishRequest>>,
    ) -> Result<Response<Self::PublishStreamStream>, Status> {
        unimplemented!()
    }
}

#[tokio::test]
async fn test_havoc_schema_cache_stampede() {
    let calls = Arc::new(AtomicUsize::new(0));
    let service = MockPubSub {
        schema_calls: calls.clone(),
    };

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let stream = TcpListenerStream::new(listener);

    tokio::spawn(async move {
        Server::builder()
            .add_service(PubSubServer::new(service))
            .serve_with_incoming(stream)
            .await
            .unwrap();
    });

    let endpoint = tonic::transport::Endpoint::from_shared(format!("http://{}", addr)).unwrap();
    let channel = endpoint.connect().await.unwrap();

    let cache = Arc::new(SchemaCache::new());

    let mut handles = vec![];

    // Spawn 100 concurrent requests for the same schema
    for _ in 0..100 {
        let cache_clone = cache.clone();
        let channel_clone = channel.clone();

        handles.push(tokio::spawn(async move {
            cache_clone
                .get_or_fetch(
                    "schema-123",
                    &channel_clone,
                    tonic::metadata::MetadataMap::new(),
                )
                .await
                .unwrap();
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let total_calls = calls.load(Ordering::SeqCst);

    // In a stampede scenario, this will be roughly 100.
    // With proper singleflight/serialization, it should be EXACTLY 1.
    assert_eq!(
        total_calls, 1,
        "👺 Havoc: Schema Cache Stampede! Expected 1 RPC call, but got {}",
        total_calls
    );
}
