#![allow(missing_docs)]
#![cfg(feature = "mock")]

use anyhow::Result;
use async_trait::async_trait;
use force::api::RestOperation;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::types::QueryResult;
use serde::Deserialize;
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// Mock authenticator
#[derive(Debug, Clone)]
struct MockAuthenticator {
    instance_url: String,
}

impl MockAuthenticator {
    fn new(instance_url: &str) -> Self {
        Self {
            instance_url: instance_url.to_string(),
        }
    }
}

#[async_trait]
impl Authenticator for MockAuthenticator {
    async fn authenticate(&self) -> force::error::Result<AccessToken> {
        Ok(AccessToken::from_response(TokenResponse {
            access_token: secrecy::SecretString::new("test_token".to_string().into()),
            instance_url: self.instance_url.clone(),
            token_type: "Bearer".to_string(),
            issued_at: "1704067200000".to_string(),
            signature: "sig".to_string(),
            expires_in: None,
            refresh_token: None,
        }))
    }

    async fn refresh(&self) -> force::error::Result<AccessToken> {
        self.authenticate().await
    }
}

#[derive(Deserialize, Debug)]
struct Record {
    #[allow(dead_code)]
    #[serde(rename = "Id")]
    id: String,
}

#[tokio::test]
async fn test_query_more_with_absolute_url() -> Result<()> {
    let mock_server = MockServer::start().await;
    let auth = MockAuthenticator::new(&mock_server.uri());

    // Construct an absolute URL for the next page
    let next_page_path = "/services/data/v67.0/query/next-page";
    let absolute_next_url = format!("{}{}", mock_server.uri(), next_page_path);

    // Mock the next page request
    Mock::given(method("GET"))
        .and(path(next_page_path))
        .and(header("authorization", "Bearer test_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 2,
            "done": true,
            "records": [
                {"Id": "001"}
            ]
        })))
        .mount(&mock_server)
        .await;

    let client = builder().authenticate(auth).build().await?;

    // Call query_more with the absolute URL
    let result: QueryResult<Record> = client.rest().query_more(&absolute_next_url).await?;

    assert_eq!(result.total_size, 2);
    assert!(result.is_done());
    assert_eq!(result.records.len(), 1);

    Ok(())
}
