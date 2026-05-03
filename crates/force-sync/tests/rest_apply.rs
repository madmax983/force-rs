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
    matchers::{body_json, header, method, path, query_param},
};

use force_sync::{ApplyError, RestApplyResult, SalesforceApplier};

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
            access_token: secrecy::SecretString::new(self.token.clone().into()),
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
async fn apply_rest_upsert_update_204_fails_when_lookup_finds_no_row() {
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

    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/query"))
        .and(query_param(
            "q",
            "SELECT Id FROM Account WHERE ExternalId__c = 'ACME-002' LIMIT 1",
        ))
        .and(header("Authorization", "Bearer test_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 0,
            "done": true,
            "records": []
        })))
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
        .await;

    let Err(error) = result else {
        panic!("expected 204 follow-up lookup failure");
    };

    assert!(matches!(error, ApplyError::Permanent(_)));
    assert!(
        error
            .to_string()
            .contains("follow-up lookup did not return")
    );
}

#[tokio::test]
async fn apply_rest_upsert_update_204_resolves_salesforce_id_when_link_missing() {
    let mock_server = MockServer::start().await;
    let client = test_client(&mock_server).await;
    let applier = SalesforceApplier::new(client);

    Mock::given(method("PATCH"))
        .and(path(
            "/services/data/v60.0/sobjects/Account/ExternalId__c/ACME-003",
        ))
        .and(header("Authorization", "Bearer test_token"))
        .and(body_json(json!({"Name": "Acme Updated"})))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/query"))
        .and(query_param(
            "q",
            "SELECT Id FROM Account WHERE ExternalId__c = 'ACME-003' LIMIT 1",
        ))
        .and(header("Authorization", "Bearer test_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 1,
            "done": true,
            "records": [{
                "attributes": {
                    "type": "Account",
                    "url": "/services/data/v60.0/sobjects/Account/001000000000003AAA"
                },
                "Id": "001000000000003AAA"
            }]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let result = applier
        .apply_rest_upsert(
            "Account",
            "ExternalId__c",
            "ACME-003",
            &json!({"Name": "Acme Updated"}),
        )
        .await
        .unwrap_or_else(|error| panic!("unexpected apply error: {error}"));

    assert_eq!(
        result,
        RestApplyResult {
            salesforce_id: Some(salesforce_id("001000000000003AAA")),
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
async fn apply_rest_delete_404_is_idempotent_success() {
    let mock_server = MockServer::start().await;
    let client = test_client(&mock_server).await;
    let applier = SalesforceApplier::new(client);
    let record_id = salesforce_id("001000000000004AAA");

    Mock::given(method("DELETE"))
        .and(path(
            "/services/data/v60.0/sobjects/Account/001000000000004AAA",
        ))
        .and(header("Authorization", "Bearer test_token"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
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

#[tokio::test]
async fn apply_rest_delete_server_error_is_retryable() {
    let mock_server = MockServer::start().await;
    let client = test_client(&mock_server).await;
    let applier = SalesforceApplier::new(client);
    let record_id = salesforce_id("001000000000005AAA");

    Mock::given(method("DELETE"))
        .and(path(
            "/services/data/v60.0/sobjects/Account/001000000000005AAA",
        ))
        .and(header("Authorization", "Bearer test_token"))
        .respond_with(ResponseTemplate::new(503).set_body_string("service unavailable"))
        .mount(&mock_server)
        .await;

    let result = applier.apply_rest_delete("Account", &record_id).await;

    let Err(error) = result else {
        panic!("expected retryable delete error");
    };

    assert!(matches!(error, ApplyError::Retryable(_)));
}

#[tokio::test]
async fn apply_rest_delete_bad_request_is_permanent() {
    let mock_server = MockServer::start().await;
    let client = test_client(&mock_server).await;
    let applier = SalesforceApplier::new(client);
    let record_id = salesforce_id("001000000000006AAA");

    Mock::given(method("DELETE"))
        .and(path(
            "/services/data/v60.0/sobjects/Account/001000000000006AAA",
        ))
        .and(header("Authorization", "Bearer test_token"))
        .respond_with(ResponseTemplate::new(400).set_body_string("bad request"))
        .mount(&mock_server)
        .await;

    let result = applier.apply_rest_delete("Account", &record_id).await;

    let Err(error) = result else {
        panic!("expected permanent delete error");
    };

    assert!(matches!(error, ApplyError::Permanent(_)));
}

#[tokio::test]
async fn apply_rest_upsert_permanent_400_classified_correctly() {
    let mock_server = MockServer::start().await;
    let client = test_client(&mock_server).await;
    let applier = SalesforceApplier::new(client);

    Mock::given(method("PATCH"))
        .and(path(
            "/services/data/v60.0/sobjects/Account/ExternalId__c/ACME-400",
        ))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!([{
            "message": "FIELD_INTEGRITY_EXCEPTION",
            "errorCode": "FIELD_INTEGRITY_EXCEPTION",
            "fields": ["Name"]
        }])))
        .mount(&mock_server)
        .await;

    let result = applier
        .apply_rest_upsert(
            "Account",
            "ExternalId__c",
            "ACME-400",
            &json!({"Name": null}),
        )
        .await;

    let Err(error) = result else {
        panic!("expected permanent apply error for 400");
    };

    assert!(matches!(error, ApplyError::Permanent(_)));
}

#[tokio::test]
async fn apply_rest_upsert_update_204_fails_when_id_field_is_null_in_query_result() {
    let mock_server = MockServer::start().await;
    let client = test_client(&mock_server).await;
    let applier = SalesforceApplier::new(client);

    // Upsert returns 204 (update, no body)
    Mock::given(method("PATCH"))
        .and(path(
            "/services/data/v60.0/sobjects/Account/ExternalId__c/ACME-NULL",
        ))
        .and(header("Authorization", "Bearer test_token"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock_server)
        .await;

    // Follow-up query returns a record but the Id field is null
    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/query"))
        .and(query_param(
            "q",
            "SELECT Id FROM Account WHERE ExternalId__c = 'ACME-NULL' LIMIT 1",
        ))
        .and(header("Authorization", "Bearer test_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 1,
            "done": true,
            "records": [{
                "attributes": {
                    "type": "Account",
                    "url": "/services/data/v60.0/sobjects/Account/001000000000099AAA"
                },
                "Id": null
            }]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let result = applier
        .apply_rest_upsert(
            "Account",
            "ExternalId__c",
            "ACME-NULL",
            &json!({"Name": "Test"}),
        )
        .await;

    let Err(error) = result else {
        panic!("expected permanent error for null Id field");
    };

    assert!(
        matches!(error, ApplyError::Permanent(_)),
        "expected Permanent error, got: {error}"
    );
    let error_text = error.to_string();
    assert!(
        error_text.contains("follow-up lookup did not return")
            || error_text.contains("invalid Salesforce"),
        "unexpected error message: {error_text}"
    );
}
