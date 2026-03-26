//! Integration tests for the Salesforce REST apply lane.

use async_trait::async_trait;
use force::{
    auth::{AccessToken, Authenticator, TokenResponse},
    client::{ForceClient, builder},
    error::Result as ForceResult,
    types::SalesforceId,
};
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_json, header, method, path},
};

use force_sync::apply::salesforce::{ApplyError, RestApplyResult, SalesforceApplier};

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

async fn test_client(mock_server: &MockServer) -> ForceClient<MockAuthenticator> {
    builder()
        .authenticate(MockAuthenticator::new("test_token", &mock_server.uri()))
        .build()
        .await
        .unwrap_or_else(|error| panic!("unexpected client build error: {error}"))
}

fn salesforce_id(id: &str) -> SalesforceId {
    SalesforceId::new(id)
        .unwrap_or_else(|error| panic!("unexpected Salesforce ID construction error: {error}"))
}

#[tokio::test]
async fn apply_rest_upsert_create_returns_created_id() {
    let mock_server = MockServer::start().await;
    let client = test_client(&mock_server).await;
    let applier = SalesforceApplier::new(client);

    Mock::given(method("PATCH"))
        .and(path(
            "/services/data/v60.0/sobjects/Account/ExternalId__c/ACME-001",
        ))
        .and(header("Authorization", "Bearer test_token"))
        .and(body_json(json!({"Name": "Acme Corp"})))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "001000000000001AAA",
            "success": true,
            "created": true,
            "errors": []
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let result = applier
        .apply_rest_upsert(
            "Account",
            "ExternalId__c",
            "ACME-001",
            &json!({"Name": "Acme Corp"}),
        )
        .await
        .unwrap_or_else(|error| panic!("unexpected apply error: {error}"));

    assert_eq!(
        result,
        RestApplyResult {
            salesforce_id: Some(salesforce_id("001000000000001AAA")),
            created: true,
        }
    );
}

#[tokio::test]
async fn apply_rest_upsert_update_204_returns_success_without_id() {
    let mock_server = MockServer::start().await;
    let client = test_client(&mock_server).await;
    let applier = SalesforceApplier::new(client);

    Mock::given(method("PATCH"))
        .and(path(
            "/services/data/v60.0/sobjects/Account/ExternalId__c/ACME-002",
        ))
        .and(header("Authorization", "Bearer test_token"))
        .and(body_json(json!({"Name": "Acme Updated"})))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock_server)
        .await;

    let result = applier
        .apply_rest_upsert(
            "Account",
            "ExternalId__c",
            "ACME-002",
            &json!({"Name": "Acme Updated"}),
        )
        .await
        .unwrap_or_else(|error| panic!("unexpected apply error: {error}"));

    assert_eq!(
        result,
        RestApplyResult {
            salesforce_id: None,
            created: false,
        }
    );
}

#[tokio::test]
async fn apply_rest_delete_uses_salesforce_id() {
    let mock_server = MockServer::start().await;
    let client = test_client(&mock_server).await;
    let applier = SalesforceApplier::new(client);
    let record_id = salesforce_id("001000000000001AAA");

    Mock::given(method("DELETE"))
        .and(path(
            "/services/data/v60.0/sobjects/Account/001000000000001AAA",
        ))
        .and(header("Authorization", "Bearer test_token"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock_server)
        .await;

    applier
        .apply_rest_delete("Account", &record_id)
        .await
        .unwrap_or_else(|error| panic!("unexpected delete error: {error}"));
}

#[tokio::test]
async fn transient_rest_upsert_failure_is_retryable() {
    let mock_server = MockServer::start().await;
    let client = test_client(&mock_server).await;
    let applier = SalesforceApplier::new(client);

    Mock::given(method("PATCH"))
        .and(path(
            "/services/data/v60.0/sobjects/Account/ExternalId__c/ACME-503",
        ))
        .respond_with(ResponseTemplate::new(503).set_body_string("temporary outage"))
        .mount(&mock_server)
        .await;

    let result = applier
        .apply_rest_upsert(
            "Account",
            "ExternalId__c",
            "ACME-503",
            &json!({"Name": "Acme Retry"}),
        )
        .await;

    let Err(error) = result else {
        panic!("expected retryable apply error");
    };

    assert!(matches!(error, ApplyError::Retryable(_)));
    assert!(error.to_string().contains("temporary outage"));
}
