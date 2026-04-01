//! Havoc test for dangerous `has_more` query pagination state.
#![cfg(feature = "rest")]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use async_trait::async_trait;
use force::api::rest_operation::RestOperation;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::Result;
use force::types::QueryResult;
use serde::Deserialize;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[derive(Debug)]
struct MyMockAuthenticator {
    token: String,
    instance_url: String,
}

#[async_trait]
impl Authenticator for MyMockAuthenticator {
    async fn authenticate(&self) -> Result<AccessToken> {
        Ok(AccessToken::from_response(TokenResponse {
            access_token: self.token.clone(),
            instance_url: self.instance_url.clone(),
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
struct TestAccount {
    #[serde(rename = "Id")]
    _id: String,
}

/// In external tests, we must define our own Must trait since the internal test_support one is not public.
pub trait Must<T> {
    /// Safe unwrap equivalent with custom message for tests.
    fn must_msg(self, msg: &str) -> T;
}

impl<T> Must<T> for Option<T> {
    fn must_msg(self, msg: &str) -> T {
        self.expect(msg)
    }
}

#[tokio::test]
async fn test_havoc_query_pagination_panic() {
    // 1. Setup Mock Server
    let salesforce = MockServer::start().await;
    let auth = MyMockAuthenticator {
        token: "sensitive_token".to_string(),
        instance_url: salesforce.uri(),
    };

    // 3. Salesforce returns a dangerous response: done=false but no nextRecordsUrl
    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 2,
            "done": false,
            "records": [
                {"Id": "001"}
            ]
        })))
        .mount(&salesforce)
        .await;

    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .expect("client build failed");

    // 5. Execute initial query
    let result: QueryResult<TestAccount> = client
        .rest()
        .query("SELECT Id FROM Account")
        .await
        .expect("query failed");

    // 6. Verify we reached the dangerous state
    if result.has_more() {
        // If has_more is true, users will try to unwrap the URL
        // With the bug, has_more() is true, next_records_url is None
        // The .must() call will panic with "Option was None"
        let _next_url = result.next_records_url.as_ref().must_msg("Option was None");
    } else {
        // If the fix is in place, has_more() is false, we reach here and don't panic
        // but the test expects a panic, so it will fail when fixed!
        // To handle this, we should change the test structure after fixing,
        // but right now it will panic and satisfy the Red Phase.
    }
}
