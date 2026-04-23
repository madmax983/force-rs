//! Integration tests for `PubSubHandler` — `GetTopic` and `GetSchema` RPCs.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::significant_drop_tightening)]
#![allow(clippy::redundant_clone)]

mod common;

use async_trait::async_trait;
use common::mock_server::{MockPubSubService, start_mock_server, start_userinfo_mock};
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::Result as ForceResult;
use force_pubsub::{PubSubConfig, PubSubError, PubSubHandler};
use std::time::Duration;
use tokio::net::TcpListener;
use wiremock::MockServer;

/// Minimal mock authenticator for handler tests.
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
        ..PubSubConfig::default()
    };
    let handler = PubSubHandler::connect(client.session(), config)
        .await
        .unwrap();
    (handler, userinfo_mock)
}

#[tokio::test]
async fn test_get_topic_returns_info() {
    let url = start_mock_server(MockPubSubService::default()).await;
    let (handler, _userinfo) = make_handler(url).await;

    let info = handler.get_topic("/event/MyEvent__e").await.unwrap();
    assert_eq!(info.topic_name, "/event/MyEvent__e");
    assert!(info.can_subscribe);
    assert_eq!(info.schema_id, "schema-test-001");
}

#[tokio::test]
async fn test_get_schema_returns_info() {
    let url = start_mock_server(MockPubSubService::default()).await;
    let (handler, _userinfo) = make_handler(url).await;

    let info = handler.get_schema("schema-test-001").await.unwrap();
    assert_eq!(info.schema_id, "schema-test-001");
    assert!(info.schema_json.contains("TestEvent"));
}

#[tokio::test]
async fn test_get_schema_not_found_returns_error() {
    let url = start_mock_server(MockPubSubService::default()).await;
    let (handler, _userinfo) = make_handler(url).await;

    let result = handler.get_schema("nonexistent-schema").await;
    let Err(err) = result else {
        panic!("Expected an error");
    };
    assert!(matches!(err, PubSubError::Transport(_)));
}

#[tokio::test]
async fn test_handler_is_cloneable() {
    let url = start_mock_server(MockPubSubService::default()).await;
    let (handler, _userinfo) = make_handler(url).await;
    let _cloned = handler.clone();
}

#[tokio::test]
async fn test_connect_rejects_invalid_batch_size_zero() {
    let url = start_mock_server(MockPubSubService::default()).await;
    let auth = TestAuth::new("test-token", "https://test.salesforce.com");
    let client = builder().authenticate(auth).build().await.unwrap();
    let config = PubSubConfig {
        endpoint: url,
        batch_size: 0,
        ..PubSubConfig::default()
    };
    let result = PubSubHandler::connect(client.session(), config).await;
    let Err(err) = result else {
        panic!("Expected an error");
    };

    assert!(matches!(err, PubSubError::Config(_)));
}

#[tokio::test]
async fn test_connect_rejects_invalid_batch_size_over_100() {
    let url = start_mock_server(MockPubSubService::default()).await;
    let auth = TestAuth::new("test-token", "https://test.salesforce.com");
    let client = builder().authenticate(auth).build().await.unwrap();
    let config = PubSubConfig {
        endpoint: url,
        batch_size: 101,
        ..PubSubConfig::default()
    };
    let result = PubSubHandler::connect(client.session(), config).await;
    let Err(err) = result else {
        panic!("Expected an error");
    };

    assert!(matches!(err, PubSubError::Config(_)));
}

#[tokio::test]
async fn test_connect_https_endpoint_uses_tls_connector() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let _accept_task = tokio::spawn(async move {
        if let Ok((socket, _peer)) = listener.accept().await {
            tokio::time::sleep(Duration::from_millis(200)).await;
            drop(socket);
        }
    });

    let auth = TestAuth::new("test-token", "https://test.salesforce.com");
    let client = builder().authenticate(auth).build().await.unwrap();
    let config = PubSubConfig {
        endpoint: format!("https://{addr}"),
        ..PubSubConfig::default()
    };

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        PubSubHandler::connect(client.session(), config),
    )
    .await
    .expect("connect should fail fast against the local non-TLS socket");

    let Err(err) = result else {
        panic!("Expected an error");
    };
    let error = format!("{err:?}");

    assert!(
        !error.contains("Connecting to HTTPS without TLS enabled"),
        "HTTPS endpoints must be configured with tonic TLS support, got: {error}",
    );
}

#[tokio::test]
async fn test_get_topic_can_publish_is_true() {
    let url = start_mock_server(MockPubSubService::default()).await;
    let (handler, _userinfo) = make_handler(url).await;

    let info = handler.get_topic("/event/MyEvent__e").await.unwrap();
    assert!(info.can_publish);
}
