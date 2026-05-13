//! Record cloner for Salesforce.
//!
//! Provides `RecordCloner`, a utility for seamlessly cloning Salesforce data,
//! optionally applying data masking for sensitive fields.

use crate::api::rest_operation::RestOperation;
use crate::auth::Authenticator;
use crate::client::ForceClient;
use crate::data::DataMasker;
use crate::error::Result;
use crate::types::DynamicSObject;
use crate::types::common::CreateResponse;

/// Utility for cloning records, optionally masking sensitive fields.
#[derive(Debug, Clone)]
pub struct RecordCloner<'a, A: Authenticator> {
    client: &'a ForceClient<A>,
    mask_pii: bool,
}

impl<'a, A: Authenticator> RecordCloner<'a, A> {
    /// Creates a new record cloner.
    ///
    /// # Arguments
    ///
    /// * `client` - The Force client.
    #[must_use]
    pub fn new(client: &'a ForceClient<A>) -> Self {
        Self {
            client,
            mask_pii: false,
        }
    }

    /// Sets whether to mask sensitive fields when cloning.
    ///
    /// If enabled, this will fetch the schema describe to automatically
    /// redact PII fields (like Email, Phone, Encrypted) using `DataMasker`.
    #[must_use]
    pub fn mask_pii(mut self, mask: bool) -> Self {
        self.mask_pii = mask;
        self
    }

    /// Clones a record by its ID, creating a new record in Salesforce.
    ///
    /// This method will:
    /// 1. Fetch the record from Salesforce.
    /// 2. If `mask_pii` is true, fetch the object schema and mask sensitive fields.
    /// 3. Remove non-createable or system fields (like Id).
    /// 4. Create the new cloned record in Salesforce.
    ///
    /// # Arguments
    ///
    /// * `sobject_name` - The API name of the SObject (e.g., "Contact").
    /// * `id` - The ID of the record to clone.
    ///
    /// # Returns
    ///
    /// Returns the `CreateResponse` from the API containing the new ID.
    pub async fn clone_record(&self, sobject_name: &str, id: &str) -> Result<CreateResponse> {
        // 1. Fetch all fields by first querying describe, or just by getting the record.
        // The easiest way is to get the record directly, but we need all fields.
        let describe = self.client.rest().describe(sobject_name).await?;

        let mut fields_to_query = Vec::new();
        for field in &describe.fields {
            // we only query fields we have permission to read
            if !field.deprecated_and_hidden {
                fields_to_query.push(field.name.clone());
            }
        }

        // Ensure we always have Id
        if !fields_to_query.contains(&"Id".to_string()) {
            fields_to_query.push("Id".to_string());
        }

        let soql = format!(
            "SELECT {} FROM {} WHERE Id = '{}' LIMIT 1",
            fields_to_query.join(", "),
            sobject_name,
            id
        );

        let mut query_result = self.client.rest().query::<DynamicSObject>(&soql).await?;

        if query_result.records.is_empty() {
            return Err(crate::error::ForceError::InvalidInput(format!(
                "Record {} not found",
                id
            )));
        }

        let mut record = query_result.records.remove(0);

        // 2. Filter out non-createable fields before masking.
        // We do this to avoid inserting fields that we shouldn't.
        let mut createable_fields = serde_json::Map::new();
        for (key, value) in record.fields {
            if let Some(field_desc) = describe
                .fields
                .iter()
                .find(|f| f.name.eq_ignore_ascii_case(&key))
            {
                if field_desc.createable {
                    createable_fields.insert(key, value);
                }
            }
        }

        record.fields = createable_fields;

        // 3. Mask PII if requested.
        if self.mask_pii {
            let masker = DataMasker::new(&describe);
            masker.mask_record(&mut record);
        }

        // 4. Create the new cloned record.
        let value = serde_json::Value::Object(record.fields);
        self.client.rest().create(sobject_name, &value).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::ForceClientBuilder;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;
    use serde_json::json;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn test_clone_record_with_masking() {
        let mock_server = MockServer::start().await;

        // 1. Mock Describe Response
        let describe_json: serde_json::Value = serde_json::from_str(r#"{
            "name": "Contact",
            "label": "Contact",
            "custom": false,
            "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "003", "labelPlural": "Contacts", "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": [
                {
                    "name": "Id", "type": "id", "label": "Id", "createable": false,
                    "autoNumber": false, "calculated": false,
                    "aggregatable": true, "byteLength": 18,
                    "cascadeDelete": false, "caseSensitive": false, "custom": false,
                    "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false,
                    "referenceTo": []
                },
                {
                    "name": "Name", "type": "string", "label": "Name", "createable": false,
                    "autoNumber": false, "calculated": false,
                    "aggregatable": true, "byteLength": 18,
                    "cascadeDelete": false, "caseSensitive": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 18, "nameField": true, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false,
                    "referenceTo": []
                },
                {
                    "name": "FirstName", "type": "string", "label": "First Name", "createable": true,
                    "autoNumber": false, "calculated": false,
                    "aggregatable": true, "byteLength": 18,
                    "cascadeDelete": false, "caseSensitive": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false,
                    "referenceTo": []
                },
                {
                    "name": "LastName", "type": "string", "label": "Last Name", "createable": true,
                    "autoNumber": false, "calculated": false,
                    "aggregatable": true, "byteLength": 18,
                    "cascadeDelete": false, "caseSensitive": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false,
                    "referenceTo": []
                },
                {
                    "name": "Email", "type": "email", "label": "Email", "createable": true,
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
            .and(path("/services/data/v60.0/sobjects/Contact/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .mount(&mock_server)
            .await;

        // 2. Mock Query Response
        let query_response = json!({
            "totalSize": 1,
            "done": true,
            "records": [
                {
                    "attributes": {"type": "Contact", "url": "/services/data/v60.0/sobjects/Contact/003xx000000001AAA"},
                    "Id": "003xx000000001AAA",
                    "Name": "Jane Doe",
                    "FirstName": "Jane",
                    "LastName": "Doe",
                    "Email": "jane.doe@example.com"
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            // SOQL query string contains spaces which get URL encoded, we just match the path to simplify
            // or we could use `query_param` if we precisely construct the URL encoded string.
            .respond_with(ResponseTemplate::new(200).set_body_json(query_response))
            .mount(&mock_server)
            .await;

        // 3. Mock Create Response
        let create_request_body = json!({
            "FirstName": "Jane",
            "LastName": "Doe",
            "Email": "***@***.***"
        });

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/sobjects/Contact"))
            .and(body_json(create_request_body))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "id": "003xx000000002BBB",
                "success": true,
                "errors": []
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let auth = MockAuthenticator::new("token", &mock_server.uri());
        let client = ForceClientBuilder::new()
            .authenticate(auth)
            .build()
            .await
            .must();

        let cloner = RecordCloner::new(&client).mask_pii(true);
        let result = cloner
            .clone_record("Contact", "003xx000000001AAA")
            .await
            .must();

        assert!(result.is_success());
        assert_eq!(result.id.must().as_str(), "003xx000000002BBB");
    }
}
