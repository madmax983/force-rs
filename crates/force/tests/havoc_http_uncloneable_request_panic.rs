#![allow(clippy::unwrap_used)]
//! Test for request body cloning panic

use force::auth::{AccessToken, TokenResponse};
use force::http::HttpExecutor;
use reqwest::Method;
use wiremock::MockServer;

#[tokio::test]
async fn test_uncloneable_request_panic() {
    let mock_server = MockServer::start().await;
    let executor = HttpExecutor::new();
    let token = AccessToken::from_response(TokenResponse {
        access_token: secrecy::SecretString::new("test".to_string().into()),
        instance_url: "http://localhost".to_string(),
        token_type: "Bearer".to_string(),
        issued_at: "0".to_string(),
        signature: "test".to_string(),
        expires_in: None,
        refresh_token: None,
    });

    // Create a streaming body which makes the request uncloneable
    let body = reqwest::Body::wrap_stream(futures::stream::iter(vec![Ok::<_, std::io::Error>(
        bytes::Bytes::from("hello"),
    )]));
    let request = reqwest::Client::new()
        .request(
            Method::POST,
            format!("{}/services/data/v60.0/query", mock_server.uri()),
        )
        .body(body)
        .build()
        .unwrap();

    let refresh_token = || async { Ok(token.clone()) };

    let result: Result<reqwest::Response, force::error::ForceError> = executor
        .execute_response(request, &token, refresh_token)
        .await;

    assert!(matches!(
        result.unwrap_err(),
        force::error::ForceError::Http(force::error::HttpError::RequestBuildError(_))
    ));
}
