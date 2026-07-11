//! CRUD operations for Salesforce REST API.
//!
//! CRUD, upsert, and upsert-idempotent operations are provided by the
//! [`RestOperation`](crate::api::rest_operation::RestOperation) trait, which
//! `RestHandler` implements. This module retains the integration tests that
//! exercise those operations through the handler.
#[cfg(test)]
mod tests {
    use crate::api::rest_operation::RestOperation;
    use crate::client::builder;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;
    use crate::types::SalesforceId;

    use serde_json::json;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // RED PHASE - Write failing tests first

    // === CREATE TESTS ===

    #[tokio::test]
    async fn test_create_success() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("POST"))
            .and(path("/services/data/v67.0/sobjects/Account"))
            .and(header("Authorization", "Bearer test_token"))
            .and(body_json(json!({"Name": "Test Account"})))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "id": "001xx000003DHP0AAO",
                "success": true,
                "errors": []
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let response = rest
            .create("Account", &json!({"Name": "Test Account"}))
            .await
            .must();

        assert!(response.is_success());
        assert_eq!(response.id.must().as_str(), "001xx000003DHP0AAO");
        assert!(response.errors.is_empty());
    }

    #[tokio::test]
    async fn test_create_missing_required_field() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("POST"))
            .and(path("/services/data/v67.0/sobjects/Account"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!([{
                "message": "Required fields are missing: [Name]",
                "statusCode": "REQUIRED_FIELD_MISSING",
                "fields": ["Name"]
            }])))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let result = rest.create("Account", &json!({})).await;

        let Err(err) = result else {
            panic!("Expected Err")
        };
        assert!(err.to_string().contains("Required fields are missing"));
    }

    #[tokio::test]
    async fn test_create_invalid_field() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("POST"))
            .and(path("/services/data/v67.0/sobjects/Account"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!([{
                "message": "No such column 'InvalidField' on sobject of type Account",
                "statusCode": "INVALID_FIELD",
                "fields": ["InvalidField"]
            }])))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let result = rest
            .create("Account", &json!({"InvalidField": "value"}))
            .await;

        let Err(err) = result else {
            panic!("Expected Err")
        };
        assert!(err.to_string().contains("No such column 'InvalidField'"));
    }

    // === GET TESTS ===

    #[tokio::test]
    async fn test_get_success() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/sobjects/Contact/003xx000004TmiQAAS"))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "attributes": {"type": "Contact", "url": "/services/data/v67.0/sobjects/Contact/003xx000004TmiQAAS"},
                "Id": "003xx000004TmiQAAS",
                "FirstName": "John",
                "LastName": "Doe",
                "Email": "john.doe@example.com"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let id = SalesforceId::new("003xx000004TmiQAAS").must();
        let record = rest.get("Contact", &id).await.must();

        assert_eq!(record["Id"], "003xx000004TmiQAAS");
        assert_eq!(record["FirstName"], "John");
        assert_eq!(record["LastName"], "Doe");
    }

    #[tokio::test]
    async fn test_get_not_found() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v67.0/sobjects/Contact/003000000000001",
            ))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!([{
                "message": "Provided external ID field does not exist or is not accessible",
                "statusCode": "NOT_FOUND"
            }])))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let id = SalesforceId::new("003000000000001").must();
        let result = rest.get("Contact", &id).await;

        let Err(err) = result else {
            panic!("Expected Err")
        };
        assert!(
            err.to_string()
                .contains("Provided external ID field does not exist or is not accessible")
        );
    }

    // === UPDATE TESTS ===

    #[tokio::test]
    async fn test_update_success() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("PATCH"))
            .and(path(
                "/services/data/v67.0/sobjects/Account/001xx000003DHP0AAO",
            ))
            .and(header("Authorization", "Bearer test_token"))
            .and(body_json(json!({"Phone": "555-0100"})))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let id = SalesforceId::new("001xx000003DHP0AAO").must();
        let response = rest
            .update("Account", &id, &json!({"Phone": "555-0100"}))
            .await
            .must();

        assert!(response.is_success());
        assert!(response.errors.is_empty());
    }

    #[tokio::test]
    async fn test_update_not_found() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("PATCH"))
            .and(path(
                "/services/data/v67.0/sobjects/Account/001000000000002",
            ))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!([{
                "message": "Entity is deleted",
                "statusCode": "ENTITY_IS_DELETED"
            }])))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let id = SalesforceId::new("001000000000002").must();
        let result = rest
            .update("Account", &id, &json!({"Phone": "555-0100"}))
            .await;

        let Err(err) = result else {
            panic!("Expected Err")
        };
        assert!(err.to_string().contains("Entity is deleted"));
    }

    #[tokio::test]
    async fn test_update_invalid_field() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("PATCH"))
            .and(path(
                "/services/data/v67.0/sobjects/Account/001xx000003DHP0AAO",
            ))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!([{
                "message": "No such column 'BadField' on sobject of type Account",
                "statusCode": "INVALID_FIELD",
                "fields": ["BadField"]
            }])))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let id = SalesforceId::new("001xx000003DHP0AAO").must();
        let result = rest
            .update("Account", &id, &json!({"BadField": "value"}))
            .await;

        let Err(err) = result else {
            panic!("Expected Err")
        };
        assert!(err.to_string().contains("No such column 'BadField'"));
    }

    // === DELETE TESTS ===

    #[tokio::test]
    async fn test_delete_success() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("DELETE"))
            .and(path(
                "/services/data/v67.0/sobjects/Account/001xx000003DHP0AAO",
            ))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let id = SalesforceId::new("001xx000003DHP0AAO").must();
        let response = rest.delete("Account", &id).await.must();

        assert!(response.is_success());
        assert!(response.errors.is_empty());
    }

    #[tokio::test]
    async fn test_delete_not_found() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("DELETE"))
            .and(path(
                "/services/data/v67.0/sobjects/Account/001000000000003",
            ))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!([{
                "message": "Entity is deleted",
                "statusCode": "ENTITY_IS_DELETED"
            }])))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let id = SalesforceId::new("001000000000003").must();
        let result = rest.delete("Account", &id).await;

        let Err(err) = result else {
            panic!("Expected Err")
        };
        assert!(err.to_string().contains("Entity is deleted"));
    }

    // === UPSERT TESTS ===

    #[tokio::test]
    async fn test_upsert_create() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("PATCH"))
            .and(path(
                "/services/data/v67.0/sobjects/Account/ExternalId__c/ACME-001",
            ))
            .and(header("Authorization", "Bearer test_token"))
            .and(body_json(json!({"Name": "Acme Corp"})))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "id": "001xx000003DHP0AAO",
                "success": true,
                "created": true,
                "errors": []
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let response = rest
            .upsert(
                "Account",
                "ExternalId__c",
                "ACME-001",
                &json!({"Name": "Acme Corp"}),
            )
            .await
            .must();

        assert!(response.is_success());
        assert!(response.is_created());
        assert_eq!(response.id.as_str(), "001xx000003DHP0AAO");
    }

    #[tokio::test]
    async fn test_upsert_success_other_status() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        // Testing the `_ if response.status().is_success()` match arm directly
        Mock::given(method("PATCH"))
            .and(path(
                "/services/data/v67.0/sobjects/Account/ExternalId__c/ACME-002",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "001xx000003DHP0AAO",
                "success": true,
                "created": false,
                "errors": []
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let response = rest
            .upsert(
                "Account",
                "ExternalId__c",
                "ACME-002",
                &json!({"Name": "Acme Corp 2"}),
            )
            .await
            .must();

        assert!(response.is_success());
        assert!(!response.is_created());
        assert_eq!(response.id.as_str(), "001xx000003DHP0AAO");
    }

    #[tokio::test]
    async fn test_upsert_does_not_retry_on_503_by_default() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("PATCH"))
            .and(path(
                "/services/data/v67.0/sobjects/Account/ExternalId__c/ACME-001",
            ))
            .respond_with(ResponseTemplate::new(503).set_body_string("temporary outage"))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let result = rest
            .upsert(
                "Account",
                "ExternalId__c",
                "ACME-001",
                &json!({"Name": "Acme Corp"}),
            )
            .await;

        let Err(err) = result else {
            panic!("Expected Err")
        };
        assert!(err.to_string().contains("temporary outage"));
    }

    #[tokio::test]
    async fn test_upsert_idempotent_retries_on_503() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("PATCH"))
            .and(path(
                "/services/data/v67.0/sobjects/Account/ExternalId__c/ACME-001",
            ))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("PATCH"))
            .and(path(
                "/services/data/v67.0/sobjects/Account/ExternalId__c/ACME-001",
            ))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "id": "001xx000003DHP0AAO",
                "success": true,
                "created": true,
                "errors": []
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let response = rest
            .upsert_idempotent(
                "Account",
                "ExternalId__c",
                "ACME-001",
                &json!({"Name": "Acme Corp"}),
            )
            .await
            .must();

        assert!(response.created);
        assert_eq!(response.id.as_str(), "001xx000003DHP0AAO");
    }

    #[tokio::test]
    async fn test_upsert_update() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("PATCH"))
            .and(path(
                "/services/data/v67.0/sobjects/Account/ExternalId__c/ACME-001",
            ))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let result = rest
            .upsert(
                "Account",
                "ExternalId__c",
                "ACME-001",
                &json!({"Name": "Acme Corp"}),
            )
            .await;

        // Note: 204 response doesn't include the ID, so this will fail
        // We need to handle this case in the implementation
        assert!(matches!(
            result,
            Err(crate::error::ForceError::NotImplemented(_))
        ));
    }

    #[tokio::test]
    async fn test_upsert_invalid_external_id_field() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("PATCH"))
            .and(path(
                "/services/data/v67.0/sobjects/Account/BadField__c/VALUE",
            ))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!([{
                "message": "Provided external ID field does not exist or is not accessible",
                "statusCode": "INVALID_FIELD"
            }])))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let result = rest
            .upsert("Account", "BadField__c", "VALUE", &json!({"Name": "Test"}))
            .await;

        let Err(err) = result else {
            panic!("Expected Err")
        };
        assert!(
            err.to_string()
                .contains("Provided external ID field does not exist or is not accessible")
        );
    }
}
