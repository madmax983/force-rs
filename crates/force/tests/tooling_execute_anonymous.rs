//! Integration tests for Tooling API Execute Anonymous.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use async_trait::async_trait;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::Result;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

// Mock authenticator
#[derive(Debug, Clone)]
struct MockAuthenticator {
    token: String,
    instance_url: String,
}

impl MockAuthenticator {
    fn new(token: &str, instance_url: &str) -> Self {
        Self {
            token: token.to_string(),
            instance_url: instance_url.to_string(),
        }
    }
}

#[async_trait]
impl Authenticator for MockAuthenticator {
    async fn authenticate(&self) -> Result<AccessToken> {
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

    async fn refresh(&self) -> Result<AccessToken> {
        self.authenticate().await
    }
}

#[tokio::test]
async fn test_execute_anonymous_success() {
    let mock_server = MockServer::start().await;
    let auth = MockAuthenticator::new("token", &mock_server.uri());
    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .expect("failed to build client");

    let script = "System.debug('Hello');";
    let encoded_script = "System.debug('Hello');"; // reqwest handles encoding, wiremock sees decoded

    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/tooling/executeAnonymous"))
        .and(query_param("anonymousBody", encoded_script))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "line": -1,
            "column": -1,
            "compiled": true,
            "success": true,
            "compileProblem": null,
            "exceptionMessage": null,
            "exceptionStackTrace": null
        })))
        .mount(&mock_server)
        .await;

    let result = client
        .tooling()
        .execute_anonymous(script)
        .await
        .expect("API call failed");

    assert!(result.is_success());
    assert!(result.compiled);
    assert!(result.error_message().is_none());
}

#[tokio::test]
async fn test_execute_anonymous_compile_error() {
    let mock_server = MockServer::start().await;
    let auth = MockAuthenticator::new("token", &mock_server.uri());
    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .expect("failed to build client");

    let script = "Invalid Apex";

    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/tooling/executeAnonymous"))
        .and(query_param("anonymousBody", script))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "line": 1,
            "column": 1,
            "compiled": false,
            "success": false,
            "compileProblem": "Unexpected token",
            "exceptionMessage": null,
            "exceptionStackTrace": null
        })))
        .mount(&mock_server)
        .await;

    let result = client
        .tooling()
        .execute_anonymous(script)
        .await
        .expect("API call failed");

    assert!(!result.is_success());
    assert!(!result.compiled);
    assert_eq!(result.error_message(), Some("Unexpected token"));
    assert_eq!(result.line, 1);
}

#[tokio::test]
async fn test_execute_anonymous_runtime_exception() {
    let mock_server = MockServer::start().await;
    let auth = MockAuthenticator::new("token", &mock_server.uri());
    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .expect("failed to build client");

    let script = "throw new Exception('Boom');";

    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/tooling/executeAnonymous"))
        .and(query_param("anonymousBody", script))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "line": 1,
            "column": 1,
            "compiled": true,
            "success": false,
            "compileProblem": null,
            "exceptionMessage": "Boom",
            "exceptionStackTrace": "Class.test: line 1"
        })))
        .mount(&mock_server)
        .await;

    let result = client
        .tooling()
        .execute_anonymous(script)
        .await
        .expect("API call failed");

    assert!(!result.is_success());
    assert!(result.compiled);
    assert_eq!(result.error_message(), Some("Boom"));
    assert_eq!(result.exception_stack_trace, Some("Class.test: line 1".to_string()));
}
