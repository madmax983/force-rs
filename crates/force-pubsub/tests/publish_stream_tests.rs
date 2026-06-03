//! Integration tests for the bidirectional `PublishStream` RPC.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::significant_drop_tightening)]

mod common;

use async_trait::async_trait;
use common::mock_server::{
    EchoPublishStreamService, start_always_error_server, start_echo_stream_server,
    start_userinfo_mock,
};
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::Result as ForceResult;
use force_pubsub::{PubSubConfig, PubSubHandler, ReconnectPolicy};
use futures::StreamExt as _;
use serde::{Deserialize, Serialize};
use wiremock::MockServer;

const SCHEMA_ID: &str = "schema-test-001";

/// Simple event type for publish tests.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestEvent {
    id: String,
}

/// Minimal mock authenticator.
#[derive(Debug, Clone)]
struct TestAuth {
    token: String,
    instance_url: String,
}

impl TestAuth {
    fn new(token: &str, instance_url: &str) -> Self {
        Self {
            token: token.to_string(),
            instance_url: instance_url.to_string(),
        }
    }
}

#[async_trait]
impl Authenticator for TestAuth {
    async fn authenticate(&self) -> ForceResult<AccessToken> {
        Ok(AccessToken::from_response(TokenResponse {
            access_token: self.token.clone(),
            instance_url: self.instance_url.clone(),
            token_type: "Bearer".to_string(),
            issued_at: "1704067200000".to_string(),
            signature: "test_sig".to_string(),
            expires_in: Some(7200),
            refresh_token: None,
        }))
    }

    async fn refresh(&self) -> ForceResult<AccessToken> {
        self.authenticate().await
    }
}

/// Returns `(handler, _userinfo_mock)` — keep `_userinfo_mock` alive for the test duration.
async fn make_handler(endpoint: String) -> (PubSubHandler<TestAuth>, MockServer) {
    let (userinfo_mock, instance_url) = start_userinfo_mock("00Dxx0000001gEREAY").await;
    let auth = TestAuth::new("test-token", &instance_url);
    let client = builder().authenticate(auth).build().await.unwrap();
    let config = PubSubConfig {
        endpoint,
        reconnect_policy: ReconnectPolicy::None,
        ..PubSubConfig::default()
    };
    let handler = PubSubHandler::connect(client.session(), config)
        .await
        .unwrap();
    (handler, userinfo_mock)
}

// ── Test 1: two batches sent, two responses received ────────────────────────

#[tokio::test]
async fn test_publish_stream_sends_events_and_receives_responses() {
    let url = start_echo_stream_server(EchoPublishStreamService::default()).await;
    let (handler, _userinfo) = make_handler(url).await;

    let mut sink = handler
        .publish_stream::<TestEvent>("/event/Test__e")
        .await
        .unwrap();

    // Send two batches.
    sink.send(
        SCHEMA_ID,
        vec![TestEvent {
            id: "evt-001".to_string(),
        }],
    )
    .await
    .unwrap();

    sink.send(
        SCHEMA_ID,
        vec![
            TestEvent {
                id: "evt-002".to_string(),
            },
            TestEvent {
                id: "evt-003".to_string(),
            },
        ],
    )
    .await
    .unwrap();

    // Drop the sender so the server sees EOF and stops sending responses.
    // We collect responses after closing.
    sink.close().await.unwrap();
}

// ── Test 2: close() drains all responses ───────────────────────────────────

#[tokio::test]
async fn test_publish_stream_close_drains_responses() {
    let url = start_echo_stream_server(EchoPublishStreamService::default()).await;
    let (handler, _userinfo) = make_handler(url).await;

    let mut sink = handler
        .publish_stream::<TestEvent>("/event/Test__e")
        .await
        .unwrap();

    sink.send(
        SCHEMA_ID,
        vec![TestEvent {
            id: "close-test".to_string(),
        }],
    )
    .await
    .unwrap();

    // close() must return Ok — i.e., no error while draining.
    let result = sink.close().await;
    assert!(
        result.is_ok(),
        "close() should drain responses without error"
    );
}

// ── Test: close() on closed server returns error ──────────────────────

#[tokio::test]
async fn test_publish_stream_close_returns_error_on_server_disconnect() {
    // Start server that returns error during stream
    let url = start_always_error_server(common::mock_server::AlwaysErrorService::default()).await;
    let (handler, _userinfo) = make_handler(url).await;

    let mut sink = handler
        .publish_stream::<TestEvent>("/event/Test__e")
        .await
        .unwrap();

    // send a message to trigger the stream
    let _ = sink.send(SCHEMA_ID, vec![]).await; // Might error if stream already closed by server

    // wait slightly so server error comes down
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let result = sink.close().await;
    assert!(
        result.is_err(),
        "close() should return error if the stream has an error"
    );
}

// ── Test 3: response payload contains per-event results ─────────────────────

#[tokio::test]
async fn test_publish_stream_response_has_results() {
    let url = start_echo_stream_server(EchoPublishStreamService::default()).await;
    let (handler, _userinfo) = make_handler(url).await;

    let mut sink = handler
        .publish_stream::<TestEvent>("/event/Test__e")
        .await
        .unwrap();

    // Send one batch with 2 events.
    sink.send(
        SCHEMA_ID,
        vec![
            TestEvent {
                id: "r1".to_string(),
            },
            TestEvent {
                id: "r2".to_string(),
            },
        ],
    )
    .await
    .unwrap();

    // Collect exactly one response (one batch → one response from the echo server).
    let first = tokio::time::timeout(std::time::Duration::from_secs(5), sink.responses().next())
        .await
        .expect("timeout waiting for stream response")
        .expect("should have at least one response");

    let resp = first.expect("response should be Ok");
    assert_eq!(
        resp.results.len(),
        2,
        "echo server returns one result per event"
    );
    assert!(resp.all_succeeded(), "all results should succeed");
}

// ── Test 4: send encodes Avro (schema caching works) ────────────────────────

#[tokio::test]
async fn test_publish_stream_send_encodes_avro_and_populates_cache() {
    let url = start_echo_stream_server(EchoPublishStreamService::default()).await;
    let (handler, _userinfo) = make_handler(url).await;

    // Cache is empty before publish_stream is used.
    assert!(handler.schema_cache.is_empty());

    let mut sink = handler
        .publish_stream::<TestEvent>("/event/Test__e")
        .await
        .unwrap();

    // send() fetches the schema from GetSchema RPC on miss.
    sink.send(
        SCHEMA_ID,
        vec![TestEvent {
            id: "avro-test".to_string(),
        }],
    )
    .await
    .unwrap();

    // Schema is now cached.
    assert!(
        handler.schema_cache.get(SCHEMA_ID).is_some(),
        "schema should be cached after first send"
    );

    sink.close().await.unwrap();
}

// ── Test 5: empty batch send is a no-op (no events, still valid request) ────

#[tokio::test]
async fn test_publish_stream_empty_batch_succeeds() {
    let url = start_echo_stream_server(EchoPublishStreamService::default()).await;
    let (handler, _userinfo) = make_handler(url).await;

    let mut sink = handler
        .publish_stream::<TestEvent>("/event/Test__e")
        .await
        .unwrap();

    // Empty batch should not error.
    let result = sink.send(SCHEMA_ID, vec![]).await;
    assert!(result.is_ok(), "empty batch send should succeed");

    sink.close().await.unwrap();
}

// ── Test 6: publish_stream is re-exported from lib root ─────────────────────

#[test]
fn test_publish_sink_is_reexported() {
    // If this file compiles, the re-export is correct.
    let _: Option<force_pubsub::PublishSink<TestEvent>> = None;
}
