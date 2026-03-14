//! CRUD operations for Salesforce REST API.
//!
//! This module provides Create, Read, Update, Delete, and Upsert operations
//! for Salesforce objects.

use crate::error::Result;
use crate::types::SalesforceId;
use crate::types::common::{CreateResponse, DeleteResponse, UpdateResponse, UpsertResponse};
use crate::types::validator::{validate_external_id_field, validate_sobject_name};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};

use super::RestHandler;

/// Custom encode set for External ID values.
/// Preserves safe path characters (-, _, ., ~) as per RFC 3986.
const UPSERT_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

impl<A: crate::auth::Authenticator> RestHandler<A> {
    /// Creates a new record in Salesforce.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The API name of the SObject type (e.g., "Account", "Contact")
    /// * `data` - JSON object containing field values to set
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - Required fields are missing
    /// - Invalid field names or values are provided
    /// - HTTP request fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use serde_json::json;
    ///
    /// let account_data = json!({
    ///     "Name": "Acme Corporation",
    ///     "Industry": "Technology"
    /// });
    ///
    /// let response = client.rest().create("Account", &account_data).await?;
    /// println!("Created account with ID: {}", response.id.must());
    /// ```
    pub async fn create(&self, sobject: &str, data: &serde_json::Value) -> Result<CreateResponse> {
        validate_sobject_name(sobject)?;
        let path = crate::api::path_utils::format_absolute_sobject_path(sobject, None);
        self.execute_post(&path, data, "Create request failed")
            .await
    }

    /// Retrieves a record by its ID.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The API name of the SObject type
    /// * `id` - The Salesforce ID of the record to retrieve
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - Record does not exist (404)
    /// - HTTP request fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let account_id = SalesforceId::new("001xx000003DHP0AAO").must();
    /// let account = client.rest().get("Account", &account_id).await?;
    /// println!("Account name: {}", account["Name"]);
    /// ```
    pub async fn get(&self, sobject: &str, id: &SalesforceId) -> Result<serde_json::Value> {
        validate_sobject_name(sobject)?;
        let path = crate::api::path_utils::format_absolute_sobject_path(sobject, Some(id.as_str()));
        self.execute_get(&path, None, "Get request failed").await
    }

    /// Updates an existing record.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The API name of the SObject type
    /// * `id` - The Salesforce ID of the record to update
    /// * `data` - JSON object containing field values to update
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - Record does not exist (404)
    /// - Invalid field names or values are provided
    /// - HTTP request fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use serde_json::json;
    ///
    /// let updates = json!({
    ///     "Phone": "555-0100",
    ///     "Industry": "Finance"
    /// });
    ///
    /// let account_id = SalesforceId::new("001xx000003DHP0AAO").must();
    /// client.rest().update("Account", &account_id, &updates).await?;
    /// ```
    pub async fn update(
        &self,
        sobject: &str,
        id: &SalesforceId,
        data: &serde_json::Value,
    ) -> Result<UpdateResponse> {
        validate_sobject_name(sobject)?;
        let path = crate::api::path_utils::format_absolute_sobject_path(sobject, Some(id.as_str()));
        self.execute_patch_empty(&path, data, "Update request failed")
            .await?;
        Ok(UpdateResponse::success())
    }

    /// Deletes a record.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The API name of the SObject type
    /// * `id` - The Salesforce ID of the record to delete
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - Record does not exist (404)
    /// - HTTP request fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let account_id = SalesforceId::new("001xx000003DHP0AAO").must();
    /// client.rest().delete("Account", &account_id).await?;
    /// ```
    pub async fn delete(&self, sobject: &str, id: &SalesforceId) -> Result<DeleteResponse> {
        validate_sobject_name(sobject)?;
        let path = crate::api::path_utils::format_absolute_sobject_path(sobject, Some(id.as_str()));
        self.execute_delete_empty(&path, "Delete request failed")
            .await?;
        Ok(DeleteResponse::success())
    }

    /// Upserts a record using an external ID field.
    ///
    /// If a record with the given external ID exists, it will be updated.
    /// Otherwise, a new record will be created.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The API name of the SObject type
    /// * `external_id_field` - The API name of the external ID field
    /// * `external_id_value` - The value of the external ID
    /// * `data` - JSON object containing field values
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - External ID field does not exist or is not marked as external ID
    /// - Required fields are missing (for create)
    /// - HTTP request fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use serde_json::json;
    ///
    /// let account_data = json!({
    ///     "Name": "Acme Corp",
    ///     "Industry": "Technology"
    /// });
    ///
    /// let response = client.rest()
    ///     .upsert("Account", "ExternalId__c", "ACME-001", &account_data)
    ///     .await?;
    ///
    /// if response.created {
    ///     println!("Created new account: {}", response.id);
    /// } else {
    ///     println!("Updated existing account: {}", response.id);
    /// }
    /// ```
    pub async fn upsert(
        &self,
        sobject: &str,
        external_id_field: &str,
        external_id_value: &str,
        data: &serde_json::Value,
    ) -> Result<UpsertResponse> {
        self.upsert_with_retry_class(
            sobject,
            external_id_field,
            external_id_value,
            data,
            crate::http::RequestRetryClass::Mutation,
        )
        .await
    }

    /// Upserts an SObject by external ID with idempotent retry semantics.
    ///
    /// This method is intended for writes that are safe to retry when transient
    /// infrastructure errors occur (for example 503). It routes through the HTTP
    /// executor's `IdempotentMutation` retry class.
    pub async fn upsert_idempotent(
        &self,
        sobject: &str,
        external_id_field: &str,
        external_id_value: &str,
        data: &serde_json::Value,
    ) -> Result<UpsertResponse> {
        self.upsert_with_retry_class(
            sobject,
            external_id_field,
            external_id_value,
            data,
            crate::http::RequestRetryClass::IdempotentMutation,
        )
        .await
    }

    async fn upsert_with_retry_class(
        &self,
        sobject: &str,
        external_id_field: &str,
        external_id_value: &str,
        data: &serde_json::Value,
        retry_class: crate::http::RequestRetryClass,
    ) -> Result<UpsertResponse> {
        validate_sobject_name(sobject)?;
        validate_external_id_field(external_id_field)?;

        let encoded_value = utf8_percent_encode(external_id_value, UPSERT_ENCODE_SET).to_string();

        let url = format!(
            "{}/sobjects/{}/{}/{}",
            self.base_url().await?,
            sobject,
            external_id_field,
            encoded_value
        );
        let request = self
            .inner
            .patch(&url)
            .json(data)
            .build()
            .map_err(crate::error::HttpError::from)?;
        let response = self
            .inner
            .execute_request_with_retry_class(request, retry_class)
            .await?;

        match response.status().as_u16() {
            204 => {
                // 204 No Content means an existing record was updated
                // But we don't have the ID from the response, this is a known limitation
                Err(crate::error::ForceError::NotImplemented(
                    "Upsert update (204) response does not include record ID - use query to retrieve".to_string()
                ))
            }
            _ if response.status().is_success() => {
                // Success codes (like 201 Created or 200 OK) - try to parse as upsert response
                response
                    .json::<UpsertResponse>()
                    .await
                    .map_err(|e| crate::error::HttpError::from(e).into())
            }
            _ => Err(crate::http::response_to_force_error(response, "Upsert request failed").await),
        }
    }
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::client::builder;
    use crate::test_support::{MockAuthenticator, Must};

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
            .and(path("/services/data/v60.0/sobjects/Account"))
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
            .and(path("/services/data/v60.0/sobjects/Account"))
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

        let err = result.unwrap_err();
        assert!(err.to_string().contains("Required fields are missing"));
    }

    #[tokio::test]
    async fn test_create_invalid_field() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/sobjects/Account"))
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

        let err = result.unwrap_err();
        assert!(err.to_string().contains("No such column 'InvalidField'"));
    }

    // === GET TESTS ===

    #[tokio::test]
    async fn test_get_success() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/sobjects/Contact/003xx000004TmiQAAS"))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "attributes": {"type": "Contact", "url": "/services/data/v60.0/sobjects/Contact/003xx000004TmiQAAS"},
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
                "/services/data/v60.0/sobjects/Contact/003000000000001",
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

        let err = result.unwrap_err();
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
                "/services/data/v60.0/sobjects/Account/001xx000003DHP0AAO",
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
                "/services/data/v60.0/sobjects/Account/001000000000002",
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

        let err = result.unwrap_err();
        assert!(err.to_string().contains("Entity is deleted"));
    }

    #[tokio::test]
    async fn test_update_invalid_field() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("PATCH"))
            .and(path(
                "/services/data/v60.0/sobjects/Account/001xx000003DHP0AAO",
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

        let err = result.unwrap_err();
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
                "/services/data/v60.0/sobjects/Account/001xx000003DHP0AAO",
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
                "/services/data/v60.0/sobjects/Account/001000000000003",
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

        let err = result.unwrap_err();
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
                "/services/data/v60.0/sobjects/Account/ExternalId__c/ACME-001",
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
                "/services/data/v60.0/sobjects/Account/ExternalId__c/ACME-002",
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
                "/services/data/v60.0/sobjects/Account/ExternalId__c/ACME-001",
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

        let err = result.unwrap_err();
        assert!(err.to_string().contains("temporary outage"));
    }

    #[tokio::test]
    async fn test_upsert_idempotent_retries_on_503() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("PATCH"))
            .and(path(
                "/services/data/v60.0/sobjects/Account/ExternalId__c/ACME-001",
            ))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("PATCH"))
            .and(path(
                "/services/data/v60.0/sobjects/Account/ExternalId__c/ACME-001",
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
                "/services/data/v60.0/sobjects/Account/ExternalId__c/ACME-001",
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
                "/services/data/v60.0/sobjects/Account/BadField__c/VALUE",
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

        let err = result.unwrap_err();
        assert!(
            err.to_string()
                .contains("Provided external ID field does not exist or is not accessible")
        );
    }
}
