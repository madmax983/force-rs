//! Havoc token leak test.
#![cfg(feature = "rest")]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use async_trait::async_trait;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::Result;
use force::types::QueryResult;
use serde::Deserialize;
use serde_json::json;
use wiremock::matchers::{header, method, path};
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

#[tokio::test]
async fn test_havoc_token_leak_via_absolute_url() {
    // 1. Setup "Salesforce" server
    let salesforce = MockServer::start().await;
    let auth = MyMockAuthenticator {
        token: "sensitive_token".to_string(),
        instance_url: salesforce.uri(),
    };

    // 2. Setup "Attacker" server
    let attacker = MockServer::start().await;
    let attacker_url = format!("{}/capture", attacker.uri());

    // 3. Salesforce returns a response pointing to Attacker
    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 2,
            "done": false,
            // POINTING TO ATTACKER!
            "nextRecordsUrl": attacker_url,
            "records": [
                {"Id": "001"}
            ]
        })))
        .mount(&salesforce)
        .await;

    // 4. Attacker expects to receive the sensitive token
    Mock::given(method("GET"))
        .and(path("/capture"))
        .and(header("Authorization", "Bearer sensitive_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 2,
            "done": true,
            "records": [
                {"Id": "002"}
            ]
        })))
        .expect(0) // MUST NOT be called (leak prevented)
        .mount(&attacker)
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

    let next_url = result.next_records_url.as_ref().expect("no next url");

    // 6. Execute query_more - this should fail with a security error
    let result_more: Result<QueryResult<TestAccount>> =
        client.rest().query_more::<TestAccount>(next_url).await;

    match result_more {
        Ok(_) => panic!("query_more should have failed! Token leak detected!"),
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("Security Error"),
                "Expected security error, got: {msg}"
            );
            assert!(
                msg.contains("nextRecordsUrl origin"),
                "Expected origin mismatch details, got: {msg}"
            );
        }
    }
}

#[tokio::test]
async fn test_havoc_token_leak_via_relative_url() {
    // 1. Setup "Salesforce" server
    let salesforce = MockServer::start().await;
    let auth = MyMockAuthenticator {
        token: "sensitive_token".to_string(),
        instance_url: salesforce.uri(),
    };

    // 2. Setup "Attacker" server
    let attacker = MockServer::start().await;

    // 3. Salesforce returns a response pointing to Attacker using a relative payload
    // A relative URL starting with `@` will become `https://instance_url@attacker_domain/capture`
    // when concatenated, making the instance URL the username and the attacker domain the host.
    // e.g. `https://127.0.0.1:1234@127.0.0.1:4567/capture`

    // We construct the payload to point to the attacker's host and port
    // E.g., @127.0.0.1:1234/capture
    let attacker_relative = format!("@{}", attacker.uri().strip_prefix("http://").unwrap());
    let attacker_capture = format!("{attacker_relative}/capture");

    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 2,
            "done": false,
            // POINTING TO ATTACKER!
            "nextRecordsUrl": attacker_capture,
            "records": [
                {"Id": "001"}
            ]
        })))
        .mount(&salesforce)
        .await;

    // 4. Attacker expects to receive the sensitive token
    Mock::given(method("GET"))
        .and(path("/capture"))
        .and(header("Authorization", "Bearer sensitive_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 2,
            "done": true,
            "records": [
                {"Id": "002"}
            ]
        })))
        .expect(0) // MUST NOT be called (leak prevented)
        .mount(&attacker)
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

    let next_url = result.next_records_url.as_ref().expect("no next url");

    // 6. Execute query_more - this should fail with a security error
    let result_more: Result<QueryResult<TestAccount>> =
        client.rest().query_more::<TestAccount>(next_url).await;

    match result_more {
        Ok(_) => panic!("query_more should have failed! Token leak detected!"),
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("Security Error"),
                "Expected security error, got: {msg}"
            );
            assert!(
                msg.contains("relative nextRecordsUrl must begin with a forward slash"),
                "Expected relative URL mismatch details, got: {msg}"
            );
        }
    }
}
