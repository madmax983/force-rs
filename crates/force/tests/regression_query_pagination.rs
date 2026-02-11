#![cfg(feature = "rest")]
#![allow(missing_docs)]
#![allow(clippy::unwrap_used)]

use async_trait::async_trait;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::Result;
use force::types::QueryResult;
use serde::{Deserialize, Serialize};
use serde_json::json;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

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
async fn regression_query_more_with_absolute_url() -> Result<()> {
    let mock_server = MockServer::start().await;
    let auth = MockAuthenticator::new("test_token", &mock_server.uri());

    // The absolute URL that Salesforce might return
    let next_url = format!("{}/services/data/v60.0/query/next-page-id", mock_server.uri());

    // Mock first page response with an absolute nextRecordsUrl
    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/query"))
        .and(query_param("q", "SELECT Id, Name FROM Account"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 4,
            "done": false,
            "nextRecordsUrl": next_url,
            "records": [
                {"Id": "001xx0000000001", "Name": "Page1 Record1"},
                {"Id": "001xx0000000002", "Name": "Page1 Record2"}
            ]
        })))
        .mount(&mock_server)
        .await;

    // Mock second page request.
    // IMPORTANT: We expect the client to request EXACTLY the path from the absolute URL,
    // NOT double-prepend the instance URL.
    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/query/next-page-id"))
        .and(header("authorization", "Bearer test_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 4,
            "done": true,
            "records": [
                {"Id": "001xx0000000003", "Name": "Page2 Record1"},
                {"Id": "001xx0000000004", "Name": "Page2 Record2"}
            ]
        })))
        .mount(&mock_server)
        .await;

    let client = builder().authenticate(auth).build().await?;

    let page1: QueryResult<TestAccount> = client.query("SELECT Id, Name FROM Account").await?;

    assert!(page1.next_records_url.is_some());
    let next_records_url = page1.next_records_url.as_ref().unwrap();

    // Verify our test setup is correct: the URL is indeed absolute
    assert!(next_records_url.starts_with("http"));

    // This call should fail if the bug exists (double URL)
    let page2 = client.query_more::<TestAccount>(next_records_url).await?;

    assert_eq!(page2.len(), 2);
    assert_eq!(page2.records[0].name, "Page2 Record1");

    Ok(())
}
