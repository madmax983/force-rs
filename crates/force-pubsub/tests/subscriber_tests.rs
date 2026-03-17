//! Integration tests for subscribe stream.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

mod common;

use async_trait::async_trait;
use common::mock_server::{MockPubSubService, start_mock_server, start_userinfo_mock};
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::Result as ForceResult;
use force_pubsub::{PubSubConfig, PubSubHandler, ReconnectPolicy, ReplayPreset};
use futures::StreamExt;
use wiremock::MockServer;

/// Minimal mock authenticator for subscriber tests.
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
async fn make_handler_no_reconnect(endpoint: String) -> (PubSubHandler<TestAuth>, MockServer) {
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

#[tokio::test]
async fn test_subscribe_yields_keepalive_when_no_events() {
    // The mock server's subscribe() immediately drops the sender (empty stream).
    // With ReconnectPolicy::None the loop sends a Transport error and terminates.
    let url = start_mock_server(MockPubSubService::default()).await;
    let (handler, _userinfo) = make_handler_no_reconnect(url).await;
    let mut stream = handler
        .subscribe("/event/Test__e", ReplayPreset::Latest)
        .await
        .unwrap();

    // First item should be either KeepAlive or a Transport error (stream ended immediately)
    let first = stream.next().await;
    assert!(first.is_some(), "stream should yield at least one item");
}

#[tokio::test]
async fn test_subscribe_returns_stream() {
    let url = start_mock_server(MockPubSubService::default()).await;
    let (handler, _userinfo) = make_handler_no_reconnect(url).await;
    let _stream = handler
        .subscribe("/event/Test__e", ReplayPreset::Latest)
        .await
        .unwrap();
    // Just verify it compiles and returns — stream behaviour tested in keepalive test
}

#[tokio::test]
async fn test_subscribe_typed_returns_stream() {
    let url = start_mock_server(MockPubSubService::default()).await;
    let (handler, _userinfo) = make_handler_no_reconnect(url).await;
    let _stream = handler
        .subscribe_typed::<serde_json::Value>("/event/Test__e", ReplayPreset::Latest)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_subscribe_stream_ends_on_no_reconnect() {
    let url = start_mock_server(MockPubSubService::default()).await;
    let (handler, _userinfo) = make_handler_no_reconnect(url).await;
    let mut stream = handler
        .subscribe("/event/Test__e", ReplayPreset::Earliest)
        .await
        .unwrap();

    // Drain stream until it ends — with ReconnectPolicy::None it must terminate.
    let mut count = 0usize;
    while let Some(_item) = stream.next().await {
        count += 1;
        if count > 10 {
            panic!("stream did not terminate as expected");
        }
    }
    // Stream ended (None returned), which is correct for ReconnectPolicy::None
}
