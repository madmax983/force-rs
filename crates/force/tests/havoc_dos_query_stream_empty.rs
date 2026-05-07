//! 👺 Havoc: Query Stream Panic/Infinite Loop Mitigation

use force::api::RestOperation;
use force::client::builder;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::error::Result;
use serde::Deserialize;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

#[derive(Debug, Clone)]
struct MockAuthenticator(String);

#[async_trait::async_trait]
impl Authenticator for MockAuthenticator {
    async fn authenticate(&self) -> Result<AccessToken> {
        Ok(AccessToken::from_response(TokenResponse {
            access_token: secrecy::SecretString::new("token".to_string().into()),
            instance_url: self.0.clone(),
            token_type: "Bearer".to_string(),
            issued_at: "1704067200000".to_string(),
            signature: "sig".to_string(),
            expires_in: None,
            refresh_token: None,
        }))
    }
    async fn refresh(&self) -> Result<AccessToken> {
        self.authenticate().await
    }
}

#[derive(Deserialize, Debug)]
struct Dummy {}

#[tokio::test]
async fn test_query_stream_empty_page_loop_dos() {
    let server = MockServer::start().await;

    // 👺 Havoc: Infinite loop of empty pages! `done: false`, but no records!
    // If the stream logic doesn't catch this, it will loop forever.
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "totalSize": 0,
            "done": false,
            "nextRecordsUrl": "/query/next",
            "records": []
        })))
        .mount(&server)
        .await;

    let auth = MockAuthenticator(server.uri());
    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .unwrap_or_else(|_| panic!("Failed to build client"));

    let mut stream = client.rest().query_stream::<Dummy>("SELECT Id FROM Account");

    let res = tokio::time::timeout(std::time::Duration::from_secs(2), stream.next()).await;

    assert!(res.is_err(), "Stream looped infinitely on empty pages!");
}
