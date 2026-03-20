//! Data seeder for rapid testing and development.
//!
//! This module provides `DataSeeder`, a utility that combines the schema discovery
//! powers of `DataFaker` with the efficiency of `BatchBuilder` to generate and
//! insert hundreds of valid, mock records into Salesforce in a few seconds.

use crate::auth::Authenticator;
use crate::client::ForceClient;
use crate::error::Result;
use crate::experimental::data_faker::generate_mock_record;
use serde_json::Value;

/// Mass operations processor using SOQL and Composite Batch API.
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
        let mut current_batch = self.client.composite().batch().halt_on_error(self.halt_on_error);

        for i in 0..count {
            let record = generate_mock_record(&describe);
            let value = serde_json::to_value(&record.fields).unwrap_or(Value::Null);

            current_batch = current_batch.post(sobject, value)?;

            if current_batch.is_full() || i == count - 1 {
                let response = current_batch.execute().await?;
                for result in response.results {
                    if result.status_code >= 200 && result.status_code < 300 {
                        success_count += 1;
                    } else if self.halt_on_error {
                        return Err(crate::error::ForceError::InvalidInput("Seed operation failed".into()));
                    }
                }

                // Reset the batch for the next chunk
                current_batch = self.client.composite().batch().halt_on_error(self.halt_on_error);
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
