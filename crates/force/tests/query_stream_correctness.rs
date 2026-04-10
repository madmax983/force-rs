#![allow(missing_docs)]
#![allow(clippy::unwrap_used)]
#![cfg(feature = "rest")]

use async_trait::async_trait;
use force::api::RestOperation;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// Mock authenticator for testing
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TestAccount {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
}

#[tokio::test]
async fn test_query_stream_empty_middle_page_repro() {
    let mock_server = MockServer::start().await;
    let auth = MockAuthenticator::new("test_token", &mock_server.uri());

    // First page: returns 1 record, done=false
    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 3,
            "done": false,
            "nextRecordsUrl": "/services/data/v60.0/query/page2",
            "records": [
                {"Id": "001", "Name": "A"}
            ]
        })))
        .mount(&mock_server)
        .await;

    // Second page: returns 0 records, done=false (empty page)
    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/query/page2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 3,
            "done": false,
            "nextRecordsUrl": "/services/data/v60.0/query/page3",
            "records": []
        })))
        .mount(&mock_server)
        .await;

    // Third page: returns 1 record, done=true
    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/query/page3"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 3,
            "done": true,
            "records": [
                {"Id": "002", "Name": "B"}
            ]
        })))
        .mount(&mock_server)
        .await;

    let client = builder().authenticate(auth).build().await.unwrap();
    let mut stream = client
        .rest()
        .query_stream::<TestAccount>("SELECT Id, Name FROM Account");

    // Should get first record
    let r1 = stream.next().await.unwrap().unwrap();
    assert_eq!(r1.name, "A");

    // Should automatically skip empty page and get second record
    let r2 = stream.next().await.unwrap().unwrap();
    assert_eq!(r2.name, "B");

    assert!(stream.next().await.unwrap().is_none());
}
