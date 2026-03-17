//! Integration tests for the unary publish RPC.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

mod common;

use async_trait::async_trait;
use common::mock_server::{start_mock_server, MockPubSubService};
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::Result as ForceResult;
use force_pubsub::{PubSubConfig, PubSubError, PubSubHandler, ReconnectPolicy};

const SCHEMA_JSON: &str =
    r#"{"type":"record","name":"TestEvent","fields":[{"name":"id","type":"string"}]}"#;
const SCHEMA_ID: &str = "schema-test-001";

/// Minimal mock authenticator for publisher tests.
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

async fn make_handler(endpoint: String) -> PubSubHandler<TestAuth> {
    let auth = TestAuth::new("test-token", "https://test.salesforce.com");
    let client = builder().authenticate(auth).build().await.unwrap();
    let config = PubSubConfig {
        endpoint,
        reconnect_policy: ReconnectPolicy::None,
        ..PubSubConfig::default()
    };
    PubSubHandler::connect(client.session(), config).await.unwrap()
}

#[tokio::test]
async fn test_publish_returns_response() {
    let url = start_mock_server(MockPubSubService::default()).await;
    let handler = make_handler(url).await;

    // Pre-populate schema cache so publish can encode events
    handler
        .schema_cache
        .parse_and_insert(SCHEMA_ID.to_string(), SCHEMA_JSON)
        .unwrap();

    let payload = serde_json::json!({"id": "evt-001"});
    let resp = handler
        .publish(SCHEMA_ID, "/event/Test__e", vec![payload])
        .await
        .unwrap();
    assert_eq!(resp.topic_name, "test");
}

#[tokio::test]
async fn test_publish_empty_events_succeeds() {
    let url = start_mock_server(MockPubSubService::default()).await;
    let handler = make_handler(url).await;
    handler
        .schema_cache
        .parse_and_insert(SCHEMA_ID.to_string(), SCHEMA_JSON)
        .unwrap();

    let resp = handler
        .publish::<serde_json::Value>(SCHEMA_ID, "/event/Test__e", vec![])
        .await
        .unwrap();
    assert!(resp.all_succeeded());
}

#[tokio::test]
async fn test_publish_schema_not_in_cache_returns_error() {
    let url = start_mock_server(MockPubSubService::default()).await;
    let handler = make_handler(url).await;
    // schema_cache is empty — don't insert anything

    let payload = serde_json::json!({"id": "evt-001"});
    let result = handler
        .publish(SCHEMA_ID, "/event/Test__e", vec![payload])
        .await;
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        PubSubError::SchemaNotFound { .. }
    ));
}
