//! Integration tests for actual event delivery through the subscribe stream
//! and reconnection behaviour.
//!
//! These tests exercise the full path: mock gRPC server → subscriber loop →
//! `PubSubEvent` stream consumer.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use common::mock_server::{
    AlwaysErrorService, EventStreamService, ReconnectingStreamService, make_consumer_event,
    make_fetch_response, make_keepalive_response, start_always_error_server,
    start_event_stream_server, start_reconnecting_stream_server, start_userinfo_mock,
};
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::Result as ForceResult;
use force_pubsub::{
    BackoffConfig, PubSubConfig, PubSubError, PubSubEvent, PubSubHandler, ReconnectPolicy,
    ReplayPreset,
};
use futures::StreamExt;
use wiremock::MockServer;

// ─── Test authenticator ────────────────────────────────────────────────────────

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

// ─── Helper: make_handler ──────────────────────────────────────────────────────

/// Returns `(handler, _userinfo_mock)`.  Keep `_userinfo_mock` alive for the
/// duration of the test so the HTTP mock server stays up.
async fn make_handler(
    endpoint: String,
    reconnect_policy: ReconnectPolicy,
) -> (PubSubHandler<TestAuth>, MockServer) {
    let (userinfo_mock, instance_url) = start_userinfo_mock("00Dxx0000001gEREAY").await;
    let auth = TestAuth::new("test-token", &instance_url);
    let client = builder().authenticate(auth).build().await.unwrap();
    let config = PubSubConfig {
        endpoint,
        reconnect_policy,
        batch_size: 10,
    };
    let handler = PubSubHandler::connect(client.session(), config)
        .await
        .unwrap();
    (handler, userinfo_mock)
}

// ─── Avro test helpers ─────────────────────────────────────────────────────────

/// Avro schema used across tests: `{"type":"record","name":"TestEvent","fields":[{"name":"field","type":"string"}]}`
const TEST_SCHEMA_ID: &str = "schema-events-test-001";
const TEST_SCHEMA_JSON: &str =
    r#"{"type":"record","name":"TestEvent","fields":[{"name":"field","type":"string"}]}"#;

/// Encode `{"field": <value>}` as Avro binary using the test schema.
fn encode_field_value(value: &str) -> Vec<u8> {
    let schema = apache_avro::Schema::parse_str(TEST_SCHEMA_JSON).unwrap();
    force_pubsub::encode_avro(&schema, &serde_json::json!({"field": value})).unwrap()
}

// ─── Tests ─────────────────────────────────────────────────────────────────────

/// A subscribe stream that delivers one event should yield `PubSubEvent::Event`
/// with the correct `schema_id`, `replay_id`, and decoded payload.
#[tokio::test]
async fn test_subscribe_yields_events() {
    let replay_id_bytes = vec![0xAA, 0xBB, 0xCC];
    let payload = encode_field_value("hello");

    let event = make_consumer_event(TEST_SCHEMA_ID, replay_id_bytes.clone(), payload);
    let response = make_fetch_response(
        "/event/Test__e",
        vec![event],
        replay_id_bytes.clone(), // latest_replay_id
    );

    let service = EventStreamService {
        topic_schema_id: TEST_SCHEMA_ID.to_string(),
        schema_json: TEST_SCHEMA_JSON.to_string(),
        responses: vec![response],
    };
    let url = start_event_stream_server(service).await;
    let (handler, _userinfo) = make_handler(url, ReconnectPolicy::None).await;

    let mut stream = handler
        .subscribe("/event/Test__e", ReplayPreset::Latest)
        .await
        .unwrap();

    // First item must be PubSubEvent::Event
    let first = stream
        .next()
        .await
        .expect("stream must yield at least one item");
    let event_msg = match first.expect("item must be Ok") {
        PubSubEvent::Event(msg) => msg,
        other => panic!("expected PubSubEvent::Event, got {other:?}"),
    };

    assert_eq!(event_msg.schema_id, TEST_SCHEMA_ID);
    assert_eq!(event_msg.replay_id.as_bytes(), replay_id_bytes.as_slice());

    // The decoded payload must contain {"field": "hello"}
    let field = event_msg.payload["field"]
        .as_str()
        .expect("payload.field must be a string");
    assert_eq!(field, "hello");
}

/// A `FetchResponse` with an empty `events` vec (keepalive) should yield
/// `PubSubEvent::KeepAlive`.
#[tokio::test]
async fn test_subscribe_yields_keepalive() {
    let keepalive_replay_id = vec![0x01, 0x02];
    let response = make_keepalive_response("/event/Test__e", keepalive_replay_id);

    let service = EventStreamService {
        topic_schema_id: TEST_SCHEMA_ID.to_string(),
        schema_json: TEST_SCHEMA_JSON.to_string(),
        responses: vec![response],
    };
    let url = start_event_stream_server(service).await;
    let (handler, _userinfo) = make_handler(url, ReconnectPolicy::None).await;

    let mut stream = handler
        .subscribe("/event/Test__e", ReplayPreset::Latest)
        .await
        .unwrap();

    // First item must be KeepAlive
    let first = stream
        .next()
        .await
        .expect("stream must yield at least one item");
    let event = first.expect("item must be Ok");
    assert!(
        matches!(event, PubSubEvent::KeepAlive),
        "expected KeepAlive, got {event:?}"
    );
}

/// After a transport error the subscriber should:
/// 1. Yield the two events from the first connection.
/// 2. Yield a `PubSubEvent::Reconnected` with the last `replay_id` seen.
/// 3. Yield the one event from the second connection.
/// 4. Then end cleanly.
#[tokio::test]
async fn test_subscribe_reconnects_on_disconnect() {
    // First connection: 2 events then Status::unavailable
    let replay_a = vec![0x01];
    let replay_b = vec![0x02];
    let replay_c = vec![0x03];

    let event_a = make_consumer_event(TEST_SCHEMA_ID, replay_a.clone(), encode_field_value("a"));
    let event_b = make_consumer_event(TEST_SCHEMA_ID, replay_b.clone(), encode_field_value("b"));
    let resp1 = make_fetch_response(
        "/event/Test__e",
        vec![event_a, event_b],
        replay_b.clone(), // latest_replay_id after first batch
    );

    // Second connection: 1 event then clean close
    let event_c = make_consumer_event(TEST_SCHEMA_ID, replay_c.clone(), encode_field_value("c"));
    let resp2 = make_fetch_response("/event/Test__e", vec![event_c], replay_c.clone());

    let connection_count = Arc::new(AtomicU32::new(0));
    let service = ReconnectingStreamService {
        topic_schema_id: TEST_SCHEMA_ID.to_string(),
        schema_json: TEST_SCHEMA_JSON.to_string(),
        first_responses: vec![resp1],
        second_responses: vec![resp2],
        connection_count: Arc::clone(&connection_count),
    };
    let url = start_reconnecting_stream_server(service).await;
    let (handler, _userinfo) = make_handler(
        url,
        ReconnectPolicy::Auto {
            max_retries: 3,
            backoff: BackoffConfig {
                initial_delay: Duration::from_millis(10),
                max_delay: Duration::from_millis(50),
                multiplier: 1.0,
            },
        },
    )
    .await;

    let mut stream = handler
        .subscribe("/event/Test__e", ReplayPreset::Latest)
        .await
        .unwrap();

    let start_time = Instant::now();

    // Collect up to 10 items (guards against infinite reconnect loop)
    let mut items: Vec<_> = Vec::new();
    loop {
        match tokio::time::timeout(Duration::from_secs(5), stream.next()).await {
            Ok(Some(item)) => {
                // Stop accumulating after we see the second-connection event (replay_c)
                // or we hit a non-recoverable error.
                let is_last = match &item {
                    Ok(PubSubEvent::Event(msg)) => msg.replay_id.as_bytes() == replay_c.as_slice(),
                    Err(_) => true,
                    _ => false,
                };
                items.push(item);
                if is_last || items.len() >= 10 {
                    break;
                }
            }
            Ok(None) => break,
            Err(e) => panic!("stream timed out waiting for items: {e}"),
        }
    }

    // We expect at least 4 items: event_a, event_b, Reconnected, event_c
    assert!(
        items.len() >= 4,
        "expected at least 4 items, got {}",
        items.len()
    );

    // item 0: Event with field="a"
    let msg_a = match items[0].as_ref().expect("item 0 must be Ok") {
        PubSubEvent::Event(m) => m,
        other => panic!("item 0: expected Event, got {other:?}"),
    };
    assert_eq!(msg_a.payload["field"].as_str().unwrap(), "a");

    // item 1: Event with field="b"
    let msg_b = match items[1].as_ref().expect("item 1 must be Ok") {
        PubSubEvent::Event(m) => m,
        other => panic!("item 1: expected Event, got {other:?}"),
    };
    assert_eq!(msg_b.payload["field"].as_str().unwrap(), "b");

    // item 2: Reconnected, replay_id must match the last replay seen (replay_b)
    match items[2].as_ref().expect("item 2 must be Ok") {
        PubSubEvent::Reconnected { replay_id, attempt } => {
            assert_eq!(
                replay_id.as_bytes(),
                replay_b.as_slice(),
                "reconnect replay_id must be the last seen replay_id"
            );
            assert_eq!(*attempt, 1, "first reconnect attempt must be 1");
        }
        other => panic!("item 2: expected Reconnected, got {other:?}"),
    }

    let elapsed = start_time.elapsed();
    // delay_for(0) with initial=10ms, multiplier=1.0 is 10ms. Elapsed must be at least 10ms.
    // Time on CI could be much larger, so we just verify it waited the minimum backoff.
    assert!(
        elapsed >= Duration::from_millis(10),
        "backoff delay not respected, elapsed: {elapsed:?}"
    );

    // item 3: Event with field="c" (from second connection)
    let msg_c = match items[3].as_ref().expect("item 3 must be Ok") {
        PubSubEvent::Event(m) => m,
        other => panic!("item 3: expected Event, got {other:?}"),
    };
    assert_eq!(msg_c.payload["field"].as_str().unwrap(), "c");
    assert_eq!(msg_c.replay_id.as_bytes(), replay_c.as_slice());
}

/// When the subscribe stream always errors and `max_retries` is exhausted, the
/// stream must eventually yield `Err(PubSubError::ReconnectFailed { .. })` and
/// terminate — no infinite loop.
#[tokio::test]
async fn test_subscribe_exhausts_retries_returns_error() {
    let service = AlwaysErrorService::default();
    let url = start_always_error_server(service).await;
    let (handler, _userinfo) = make_handler(
        url,
        ReconnectPolicy::Auto {
            max_retries: 2,
            backoff: BackoffConfig {
                initial_delay: Duration::from_millis(10),
                max_delay: Duration::from_millis(50),
                multiplier: 2.0,
            },
        },
    )
    .await;

    let mut stream = handler
        .subscribe("/event/Test__e", ReplayPreset::Latest)
        .await
        .unwrap();

    let start_time = Instant::now();

    // Drain items until we see ReconnectFailed or the stream ends.
    let mut reconnect_failed_seen = false;
    let mut count = 0usize;
    let mut reconnect_events = 0;
    loop {
        match tokio::time::timeout(Duration::from_secs(5), stream.next()).await {
            Ok(Some(item)) => {
                count += 1;
                if let Ok(PubSubEvent::Reconnected { .. }) = &item {
                    reconnect_events += 1;
                }
                if let Err(PubSubError::ReconnectFailed { attempts, .. }) = &item {
                    // attempts == max_retries + 1 due to the increment before the check
                    assert!(*attempts > 0, "attempts must be > 0");
                    assert_eq!(*attempts, 3, "expected 3 attempts (max_retries 2 + 1)");
                    assert_eq!(
                        reconnect_events, 2,
                        "should have seen exactly 2 reconnect events"
                    );
                    let elapsed = start_time.elapsed();
                    // We expect total delay to be at least 30ms (10ms + 20ms).
                    assert!(
                        elapsed >= Duration::from_millis(30),
                        "backoff delay too short, elapsed: {elapsed:?}"
                    );
                    reconnect_failed_seen = true;
                    break;
                }
                assert!(count <= 20, "stream did not terminate after 20 items");
            }
            Ok(None) => break, // stream ended without an error (also acceptable)
            Err(e) => panic!("stream timed out — possible infinite reconnect loop: {e}"),
        }
    }

    assert!(
        reconnect_failed_seen,
        "expected PubSubError::ReconnectFailed to be yielded"
    );
}
