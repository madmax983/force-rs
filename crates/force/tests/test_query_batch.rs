//! Integration tests for `QueryBatch`.
#![cfg(feature = "composite")]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use force::api::composite::BatchOp;
use async_trait::async_trait;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::Result;
use serde::Deserialize;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
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
            access_token: secrecy::SecretString::new(self.token.clone().into()),
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

#[derive(Debug, Deserialize, Clone)]
struct Account {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
}

#[tokio::test]
async fn test_query_batch_pagination_and_update() {
    let mock_server = MockServer::start().await;
    let auth = MyMockAuthenticator {
        token: "token".to_string(),
        instance_url: mock_server.uri(),
    };

    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .expect("client build failed");

    // 1. Mock Query Page 1 (2 records)
    // We will update the first one and delete the second one.
    // nextRecordsUrl points to page 2.
    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/query"))
        .and(query_param("q", "SELECT Id, Name FROM Account"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 4,
            "done": false,
            "nextRecordsUrl": "/services/data/v60.0/query/page2",
            "records": [
                {"Id": "001000000000001", "Name": "Account 1"},
                {"Id": "001000000000002", "Name": "Account 2"}
            ]
        })))
        .mount(&mock_server)
        .await;

    // 2. Mock Query Page 2 (2 records)
    // We will ignore the first one and create a child for the second one.
    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/query/page2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 4,
            "done": true,
            "records": [
                {"Id": "001000000000003", "Name": "Account 3"},
                {"Id": "001000000000004", "Name": "Account 4"}
            ]
        })))
        .mount(&mock_server)
        .await;

    // 3. Mock Batch Request
    // We expect one batch request since we have < 25 operations.
    // Ops: Update(Acc1), Delete(Acc2), Create(Contact for Acc4).
    Mock::given(method("POST"))
        .and(path("/services/data/v60.0/composite/batch"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "hasErrors": false,
            "results": [
                {"statusCode": 204, "result": null}, // Update
                {"statusCode": 204, "result": null}, // Delete
                {"statusCode": 201, "result": {"id": "003..."}} // Create
            ]
        })))
        .mount(&mock_server)
        .await;

    let processor = client.composite().query_batch("SELECT Id, Name FROM Account");

    let stats = processor
        .run(|record: Account| match record.name.as_str() {
            "Account 1" => Some(BatchOp::Update(
                "Account".to_string(),
                record.id,
                json!({"Name": "Updated 1"}),
            )),
            "Account 2" => Some(BatchOp::Delete("Account".to_string(), record.id)),
            "Account 4" => Some(BatchOp::Create(
                "Contact".to_string(),
                json!({"AccountId": record.id, "LastName": "Contact"}),
            )),
            _ => None,
        })
        .await
        .expect("run failed");

    assert_eq!(stats.records_processed, 4);
    assert_eq!(stats.ops_succeeded, 3);
    assert_eq!(stats.ops_failed, 0);
}

#[tokio::test]
async fn test_query_batch_multiple_batches() {
    let mock_server = MockServer::start().await;
    let auth = MyMockAuthenticator {
        token: "token".to_string(),
        instance_url: mock_server.uri(),
    };
    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .expect("client build failed");

    // 1. Mock Query returning 26 records
    let mut records = Vec::new();
    for i in 0..26 {
        records.push(json!({
            "Id": format!("001{:012}", i),
            "Name": format!("Account {}", i)
        }));
    }

    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 26,
            "done": true,
            "records": records
        })))
        .mount(&mock_server)
        .await;

    // 2. Mock Batch Requests

    // Batch 1 (25 items, contains first ID)
    let mut results1 = Vec::new();
    for _ in 0..25 {
        results1.push(json!({"statusCode": 200, "result": null}));
    }

    Mock::given(method("POST"))
        .and(path("/services/data/v60.0/composite/batch"))
        .and(wiremock::matchers::body_string_contains("001000000000000"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "hasErrors": false,
            "results": results1
        })))
        .mount(&mock_server)
        .await;

    // Batch 2 (Specific, 1 item, ID 25)
    // Mounted LAST, checked FIRST
    Mock::given(method("POST"))
        .and(path("/services/data/v60.0/composite/batch"))
        .and(wiremock::matchers::body_string_contains("001000000000025"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "hasErrors": false,
            "results": [{"statusCode": 200, "result": null}]
        })))
        .mount(&mock_server)
        .await;

    let processor = client.composite().query_batch("SELECT Id, Name FROM Account");

    let stats = processor
        .run(|record: Account| Some(BatchOp::Delete("Account".to_string(), record.id)))
        .await
        .expect("run failed");

    assert_eq!(stats.records_processed, 26);
    assert_eq!(stats.ops_succeeded, 26);
}

#[tokio::test]
async fn test_query_batch_halt_on_error() {
    let mock_server = MockServer::start().await;
    let auth = MyMockAuthenticator {
        token: "token".to_string(),
        instance_url: mock_server.uri(),
    };
    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .expect("client build failed");

    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 1,
            "done": true,
            "records": [{"Id": "001000000000001", "Name": "Account 1"}]
        })))
        .mount(&mock_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/services/data/v60.0/composite/batch"))
        .and(wiremock::matchers::body_string_contains(
            "haltOnError\":true",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "hasErrors": true,
            "results": [{"statusCode": 400, "result": null}]
        })))
        .mount(&mock_server)
        .await;

    let processor = client.composite().query_batch("SELECT Id, Name FROM Account").halt_on_error(true);

    let stats = processor
        .run(|record: Account| Some(BatchOp::Delete("Account".to_string(), record.id)))
        .await
        .expect("run failed");

    assert_eq!(stats.ops_failed, 1);
    assert_eq!(stats.ops_succeeded, 0);
}

#[tokio::test]
async fn test_query_batch_empty_results() {
    let mock_server = MockServer::start().await;
    let auth = MyMockAuthenticator {
        token: "token".to_string(),
        instance_url: mock_server.uri(),
    };
    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .expect("client build failed");

    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 0,
            "done": true,
            "records": []
        })))
        .mount(&mock_server)
        .await;

    let processor = client.composite().query_batch("SELECT Id, Name FROM Account");

    let stats = processor
        .run(|record: Account| Some(BatchOp::Delete("Account".to_string(), record.id)))
        .await
        .expect("run failed");

    assert_eq!(stats.records_processed, 0);
    assert_eq!(stats.ops_succeeded, 0);
    assert_eq!(stats.ops_failed, 0);
}

#[tokio::test]
async fn test_query_batch_mixed_results() {
    let mock_server = MockServer::start().await;
    let auth = MyMockAuthenticator {
        token: "token".to_string(),
        instance_url: mock_server.uri(),
    };
    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .expect("client build failed");

    // 1. Mock Query with 2 records
    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 2,
            "done": true,
            "records": [
                {"Id": "001000000000001", "Name": "Account 1"},
                {"Id": "001000000000002", "Name": "Account 2"}
            ]
        })))
        .mount(&mock_server)
        .await;

    // 2. Mock Batch Request with 1 success, 1 failure
    Mock::given(method("POST"))
        .and(path("/services/data/v60.0/composite/batch"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "hasErrors": true,
            "results": [
                {"statusCode": 204, "result": null},
                {"statusCode": 400, "result": {"errorCode": "BAD_REQUEST", "message": "Bad"}}
            ]
        })))
        .mount(&mock_server)
        .await;

    let processor = client.composite().query_batch("SELECT Id, Name FROM Account");

    let stats = processor
        .run(|record: Account| Some(BatchOp::Delete("Account".to_string(), record.id)))
        .await
        .expect("run failed");

    assert_eq!(stats.records_processed, 2);
    assert_eq!(stats.ops_succeeded, 1);
    assert_eq!(stats.ops_failed, 1);
}

#[tokio::test]
async fn test_query_batch_halt_on_error_false() {
    let mock_server = MockServer::start().await;
    let auth = MyMockAuthenticator {
        token: "token".to_string(),
        instance_url: mock_server.uri(),
    };
    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .expect("client build failed");

    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 1,
            "done": true,
            "records": [{"Id": "001000000000001", "Name": "Account 1"}]
        })))
        .mount(&mock_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/services/data/v60.0/composite/batch"))
        .and(wiremock::matchers::body_string_contains(
            "haltOnError\":false",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "hasErrors": true,
            "results": [{"statusCode": 400, "result": null}]
        })))
        .mount(&mock_server)
        .await;

    let processor = client.composite().query_batch("SELECT Id, Name FROM Account").halt_on_error(false);

    let stats = processor
        .run(|record: Account| Some(BatchOp::Delete("Account".to_string(), record.id)))
        .await
        .expect("run failed");

    assert_eq!(stats.ops_failed, 1);
    assert_eq!(stats.ops_succeeded, 0);
}

#[tokio::test]
async fn test_query_batch_returns_default_stats_if_empty() {
    let mock_server = MockServer::start().await;
    let auth = MyMockAuthenticator {
        token: "token".to_string(),
        instance_url: mock_server.uri(),
    };
    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .expect("client build failed");

    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 0,
            "done": true,
            "records": []
        })))
        .mount(&mock_server)
        .await;

    let processor = client.composite().query_batch("SELECT Id, Name FROM Account");

    let stats = processor
        .run(|_record: Account| None)
        .await
        .expect("run failed");

    assert_eq!(stats.records_processed, 0);
    assert_eq!(stats.ops_succeeded, 0);
    assert_eq!(stats.ops_failed, 0);
}

#[tokio::test]
async fn test_query_batch_ops_counts() {
    let mock_server = MockServer::start().await;
    let auth = MyMockAuthenticator {
        token: "token".to_string(),
        instance_url: mock_server.uri(),
    };
    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .expect("client build failed");

    // 1. Mock Query
    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 2,
            "done": true,
            "records": [
                {"Id": "001000000000001", "Name": "Account 1"},
                {"Id": "001000000000002", "Name": "Account 2"}
            ]
        })))
        .mount(&mock_server)
        .await;

    // 2. Mock Batch Request with 1 success, 1 failure
    Mock::given(method("POST"))
        .and(path("/services/data/v60.0/composite/batch"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "hasErrors": true,
            "results": [
                {"statusCode": 204, "result": null},
                {"statusCode": 400, "result": {"errorCode": "BAD_REQUEST", "message": "Bad"}}
            ]
        })))
        .mount(&mock_server)
        .await;

    let processor = client.composite().query_batch("SELECT Id, Name FROM Account");

    let stats = processor
        .run(|record: Account| {
            Some(BatchOp::Update(
                "Account".to_string(),
                record.id,
                json!({"Name": "Updated"}),
            ))
        })
        .await
        .expect("run failed");

    assert_eq!(stats.records_processed, 2);
    assert_eq!(stats.ops_succeeded, 1);
    assert_eq!(stats.ops_failed, 1);
}
