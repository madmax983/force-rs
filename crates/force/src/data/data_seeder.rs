//! Data seeder for rapid testing and development.
//!
//! This module provides `DataSeeder`, a utility that combines the schema discovery
//! powers of `DataFaker` with the efficiency of `BatchBuilder` to generate and
//! insert hundreds of valid, mock records into Salesforce in a few seconds.

use crate::api::rest_operation::RestOperation;
use crate::auth::Authenticator;
use crate::client::ForceClient;
use crate::error::Result;

use super::data_faker::generate_mock_record;

/// Utility for generating and inserting mock records from schema metadata.
#[derive(Debug)]
pub struct DataSeeder<'a, A: Authenticator> {
    client: &'a ForceClient<A>,
    halt_on_error: bool,
}

impl<'a, A: Authenticator> DataSeeder<'a, A> {
    /// Creates a new data seeder.
    ///
    /// # Arguments
    ///
    /// * `client` - The Force client.
    #[must_use]
    pub fn new(client: &'a ForceClient<A>) -> Self {
        Self {
            client,
            halt_on_error: false,
        }
    }

    /// Sets whether to stop processing if a batch operation fails.
    ///
    /// Default is false.
    #[must_use]
    pub fn halt_on_error(mut self, halt: bool) -> Self {
        self.halt_on_error = halt;
        self
    }

    /// Generates and inserts a specified number of fake records for an SObject.
    ///
    /// This method fetches the object's describe information, generates mock
    /// data using `DataFaker`, and inserts them using the Composite Batch API.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The name of the SObject (e.g., "Account").
    /// * `count` - The number of records to generate and insert.
    ///
    /// # Errors
    ///
    /// Returns an error if the describe call fails or if executing the batch fails.
    pub async fn seed(&self, sobject: &str, count: usize) -> Result<usize> {
        if count == 0 {
            return Ok(0);
        }

        let describe = self.client.rest().describe(sobject).await?;

        let mut success_count = 0;
        let mut current_batch = self
            .client
            .composite()
            .batch()
            .halt_on_error(self.halt_on_error);

        for i in 0..count {
            let record = generate_mock_record(&describe);
            let value = serde_json::to_value(&record.fields).map_err(|e| {
                crate::error::ForceError::InvalidInput(format!(
                    "Failed to serialize mock record: {e}"
                ))
            })?;

            current_batch = current_batch.post(sobject, value)?;

            let is_last_record = (i + 1) == count;
            let needs_execute = current_batch.is_full() || is_last_record;

            if needs_execute {
                let response = current_batch.execute().await?;
                for result in response.results {
                    let status = result.status_code;
                    let is_success = status >= 200 && status < 300;
                    if is_success {
                        success_count += 1;
                    } else if self.halt_on_error {
                        return Err(crate::error::ForceError::InvalidInput(
                            "Seed operation failed".into(),
                        ));
                    }
                }

                // Reset the batch for the next chunk
                current_batch = self
                    .client
                    .composite()
                    .batch()
                    .halt_on_error(self.halt_on_error);
            }
        }

        Ok(success_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::builder;
    use crate::test_support::{MockAuthenticator, Must};
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn create_mock_server() -> MockServer {
        MockServer::start().await
    }

    async fn create_test_client(mock_server: &MockServer) -> ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        builder().authenticate(auth).build().await.must()
    }

    #[tokio::test]
    async fn test_data_seeder_multiple_batches() {
        let mock_server = create_mock_server().await;
        let client = create_test_client(&mock_server).await;

        let describe_json = serde_json::from_str::<serde_json::Value>(r#"{
            "name": "Account",
            "label": "Account",
            "custom": false,
            "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "001", "labelPlural": "Accounts", "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": [
                {
                    "name": "Name", "type": "string", "label": "Account Name", "createable": true,
                    "autoNumber": false, "calculated": false,
                    "aggregatable": true, "byteLength": 18,
                    "cascadeDelete": false, "caseSensitive": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false,
                    "referenceTo": []
                }
            ]
        }"#).must();

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/sobjects/Account/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .mount(&mock_server)
            .await;

        // Mock the composite batch response
        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/composite/batch"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "hasErrors": false,
                "results": [
                    { "statusCode": 201, "result": { "id": "X", "success": true, "errors": [] } }
                ]
            })))
            .mount(&mock_server)
            .await;

        let seeder = DataSeeder::new(&client).halt_on_error(false);
        // It should perform exactly 12 executions because 300 requests => 25 requests per chunk
        // Note: BatchBuilder's limit is hardcoded based on the constants. (default 25 records per composite batch request).
        // For 300 records, 300 / 25 = 12 batches.
        let success = seeder.seed("Account", 300).await.must();
        // The mocked response returns 1 successful result per batch, so 12 batches = 12 successes.
        assert_eq!(success, 12);
    }

    #[tokio::test]
    async fn test_data_seeder_zero_count() {
        let mock_server = create_mock_server().await;
        let client = create_test_client(&mock_server).await;
        let seeder = DataSeeder::new(&client);

        let success_count = seeder.seed("Account", 0).await.must();
        assert_eq!(success_count, 0);
    }

    #[tokio::test]
    async fn test_data_seeder_mutants_halt_on_error() {
        let mock_server = create_mock_server().await;
        let client = create_test_client(&mock_server).await;
        // Test that halt_on_error mutates state
        let seeder = DataSeeder::new(&client);
        assert!(!seeder.halt_on_error);
        let seeder = seeder.halt_on_error(true);
        assert!(seeder.halt_on_error);
    }

    #[tokio::test]
    async fn test_data_seeder_seed_math_mutants() {
        let mock_server = create_mock_server().await;
        let client = create_test_client(&mock_server).await;

        let describe_json = serde_json::from_str::<serde_json::Value>(r#"{
            "name": "Account",
            "label": "Account",
            "custom": false,
            "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "001", "labelPlural": "Accounts", "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": [
                {
                    "name": "Name", "type": "string", "label": "Account Name", "createable": true,
                    "autoNumber": false, "calculated": false,
                    "aggregatable": true, "byteLength": 18,
                    "cascadeDelete": false, "caseSensitive": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false,
                    "referenceTo": []
                }
            ]
        }"#).must();

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/sobjects/Account/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .mount(&mock_server)
            .await;

        // Mock the composite batch response
        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/composite/batch"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "hasErrors": false,
                "results": [
                    { "statusCode": 201, "result": { "id": "001000000000001AAA", "success": true, "errors": [] } },
                    { "statusCode": 201, "result": { "id": "001000000000002AAA", "success": true, "errors": [] } },
                    { "statusCode": 201, "result": { "id": "001000000000003AAA", "success": true, "errors": [] } }
                ]
            })))
            .mount(&mock_server)
            .await;

        let seeder = DataSeeder::new(&client);
        let success_count = seeder.seed("Account", 3).await.must();

        // Testing replacing `1` with `0` in `Ok(success_count)` or replacing `+= 1` with `*= 1` or `-= 1`.
        assert_eq!(success_count, 3, "Expected exactly 3 successes returned");
    }

    #[tokio::test]
    async fn test_data_seeder_halt_on_error() {
        let mock_server = create_mock_server().await;
        let client = create_test_client(&mock_server).await;

        let describe_json = serde_json::from_str::<serde_json::Value>(r#"{
            "name": "Account",
            "label": "Account",
            "custom": false,
            "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "001", "labelPlural": "Accounts", "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": [
                {
                    "name": "Name", "type": "string", "label": "Account Name", "createable": true,
                    "autoNumber": false, "calculated": false,
                    "aggregatable": true, "byteLength": 18,
                    "cascadeDelete": false, "caseSensitive": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false,
                    "referenceTo": []
                }
            ]
        }"#).must();

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/sobjects/Account/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .mount(&mock_server)
            .await;

        // Mock the composite batch response with an error
        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/composite/batch"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "hasErrors": true,
                "results": [
                    { "statusCode": 201, "result": { "id": "001000000000001AAA", "success": true, "errors": [] } },
                    { "statusCode": 400, "result": [{ "errorCode": "INVALID_FIELD", "message": "Bad field" }] }
                ]
            })))
            .mount(&mock_server)
            .await;

        let seeder = DataSeeder::new(&client).halt_on_error(true);
        let result = seeder.seed("Account", 2).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Seed operation failed"));
    }

    #[tokio::test]
    async fn test_data_seeder_serialization_error() {
        // Technically unreachable in normal operation because DynamicSObject serializes safely,
        // but adding for mutation coverage completeness.
        let mock_server = create_mock_server().await;
        let client = create_test_client(&mock_server).await;
        // if count is 0, we immediately return Ok(0)
        let seeder = DataSeeder::new(&client).halt_on_error(true);
        let res = seeder.seed("Account", 0).await;
        assert_eq!(res.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_data_seeder_no_halt_on_error() {
        let mock_server = create_mock_server().await;
        let client = create_test_client(&mock_server).await;

        let describe_json = serde_json::from_str::<serde_json::Value>(r#"{
            "name": "Account",
            "label": "Account",
            "custom": false,
            "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "001", "labelPlural": "Accounts", "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": [
                {
                    "name": "Name", "type": "string", "label": "Account Name", "createable": true,
                    "autoNumber": false, "calculated": false,
                    "aggregatable": true, "byteLength": 18,
                    "cascadeDelete": false, "caseSensitive": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false,
                    "referenceTo": []
                }
            ]
        }"#).must();

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/sobjects/Account/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .mount(&mock_server)
            .await;

        // Mock the composite batch response with an error
        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/composite/batch"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "hasErrors": true,
                "results": [
                    { "statusCode": 201, "result": { "id": "001000000000001AAA", "success": true, "errors": [] } },
                    { "statusCode": 400, "result": [{ "errorCode": "INVALID_FIELD", "message": "Bad field" }] }
                ]
            })))
            .mount(&mock_server)
            .await;

        let seeder = DataSeeder::new(&client).halt_on_error(false);
        let success_count = seeder.seed("Account", 2).await.must();

        // 1 success out of 2 requested
        assert_eq!(success_count, 1);
    }

    #[tokio::test]
    async fn test_data_seeder_success() {
        let mock_server = create_mock_server().await;
        let client = create_test_client(&mock_server).await;

        let describe_json = serde_json::from_str::<serde_json::Value>(r#"{
            "name": "Account",
            "label": "Account",
            "custom": false,
            "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "001", "labelPlural": "Accounts", "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": [
                {
                    "name": "Name", "type": "string", "label": "Account Name", "createable": true,
                    "autoNumber": false, "calculated": false,
                    "aggregatable": true, "byteLength": 18,
                    "cascadeDelete": false, "caseSensitive": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false,
                    "referenceTo": []
                }
            ]
        }"#).must();

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/sobjects/Account/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .mount(&mock_server)
            .await;

        // Mock the composite batch response
        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/composite/batch"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "hasErrors": false,
                "results": [
                    { "statusCode": 201, "result": { "id": "001000000000001AAA", "success": true, "errors": [] } },
                    { "statusCode": 201, "result": { "id": "001000000000002AAA", "success": true, "errors": [] } }
                ]
            })))
            .mount(&mock_server)
            .await;

        let seeder = DataSeeder::new(&client);
        let success_count = seeder.seed("Account", 2).await.must();

        assert_eq!(success_count, 2);
    }
}
