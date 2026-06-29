//! CSV Data Dictionary Generator.
//!
//! This module provides a utility to generate a CSV data dictionary
//! for a Salesforce SObject based on the Describe API metadata.

use crate::api::rest_operation::RestOperation;
use crate::auth::Authenticator;
use crate::client::ForceClient;
use crate::error::Result;
use std::fmt::Write;

/// Generator for SObject data dictionaries in CSV format.
#[derive(Debug)]
pub struct CsvDictionary<'a, A: Authenticator> {
    client: &'a ForceClient<A>,
}

impl<'a, A: Authenticator> CsvDictionary<'a, A> {
    /// Creates a new CSV data dictionary generator.
    #[must_use]
    pub fn new(client: &'a ForceClient<A>) -> Self {
        Self { client }
    }

    /// Generates a CSV data dictionary for the specified SObject.
    pub async fn generate(&self, sobject: &str) -> Result<String> {
        let describe = self.client.rest().describe(sobject).await?;

        let mut csv = String::with_capacity(1024);
        csv.push_str("API Name,Type,Createable,Reference To\n");

        let mut fields = describe.fields;
        fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

        for field in fields {
            let createable_str = if field.createable { "Yes" } else { "No" };
            let ref_to = field.reference_to.join(", ");
            let ref_to_str = if ref_to.is_empty() {
                String::new()
            } else {
                format!("\"{}\"", ref_to)
            };

            let _ = writeln!(
                csv,
                "\"{}\",\"{:?}\",\"{}\",{}",
                field.name, field.type_, createable_str, ref_to_str
            );
        }

        Ok(csv)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::ForceClientBuilder;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_generate_csv() {
        let mock_server = MockServer::start().await;
        let describe_json = json!({
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
                    "name": "Id", "type": "id", "label": "Record ID", "createable": false,
                    "autoNumber": false, "calculated": false, "aggregatable": true, "byteLength": 18,
                    "cascadeDelete": false, "caseSensitive": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false,
                    "referenceTo": []
                },
                {
                    "name": "Name", "type": "string", "label": "Account Name", "createable": true,
                    "autoNumber": false, "calculated": false, "aggregatable": true, "byteLength": 255,
                    "cascadeDelete": false, "caseSensitive": false, "custom": false,
                    "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 255, "nameField": true, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false,
                    "referenceTo": []
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/sobjects/Account/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .mount(&mock_server)
            .await;

        let auth = MockAuthenticator::new("token", &mock_server.uri());
        let client = ForceClientBuilder::new()
            .authenticate(auth)
            .build()
            .await
            .must();
        let dict = CsvDictionary::new(&client);

        let csv = dict.generate("Account").await.must();
        assert!(csv.contains("API Name,Type,Createable,Reference To"));
        assert!(csv.contains("\"Id\",\"Id\",\"No\","));
        assert!(csv.contains("\"Name\",\"String\",\"Yes\","));
    }
}
