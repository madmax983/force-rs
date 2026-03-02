//! Data Dictionary Generator.
//!
//! This module provides a utility to generate a Markdown data dictionary
//! for a Salesforce SObject, combining Describe API metadata with optional
//! Field Usage Scanner statistics.
//!
//! This serves as a powerful "Exporter" to document schema directly from the API.

use crate::auth::Authenticator;
use crate::client::ForceClient;
use crate::error::Result;
use crate::experimental::scanner::FieldUsageScanner;
use std::collections::HashMap;

/// Generator for SObject data dictionaries in Markdown format.
#[derive(Debug)]
pub struct DataDictionary<'a, A: Authenticator> {
    client: &'a ForceClient<A>,
}

impl<'a, A: Authenticator> DataDictionary<'a, A> {
    /// Creates a new data dictionary generator.
    ///
    /// # Arguments
    ///
    /// * `client` - The authenticated Force client.
    #[must_use]
    pub fn new(client: &'a ForceClient<A>) -> Self {
        Self { client }
    }

    /// Generates a Markdown data dictionary for the specified SObject.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The API name of the SObject (e.g., "Account").
    /// * `include_usage` - Whether to scan and include field population statistics.
    ///
    /// # Returns
    ///
    /// A String containing the generated Markdown document.
    pub async fn generate(&self, sobject: &str, include_usage: bool) -> Result<String> {
        let describe = self.client.rest().describe(sobject).await?;

        let mut usage_map = HashMap::new();
        if include_usage {
            let scanner = FieldUsageScanner::new(self.client);
            let usages = scanner.scan(sobject).await?;
            for usage in usages {
                usage_map.insert(usage.name.clone(), usage);
            }
        }

        let mut md = String::with_capacity(1024);

        md.push_str(&format!("# Data Dictionary: {}\n", describe.label));
        md.push_str(&format!("**API Name:** `{}`\n", describe.name));
        md.push_str(&format!("**Custom:** {}\n\n", describe.custom));

        md.push_str("## Fields\n\n");

        if include_usage {
            md.push_str("| Label | API Name | Type | Required | Reference To | Populated % |\n");
            md.push_str("|---|---|---|---|---|---|\n");
        } else {
            md.push_str("| Label | API Name | Type | Required | Reference To |\n");
            md.push_str("|---|---|---|---|---|\n");
        }

        let mut fields = describe.fields;
        fields.sort_by(|a, b| a.name.cmp(&b.name));

        for field in fields {
            let required = if !field.nillable && !field.defaulted_on_create {
                "Yes"
            } else {
                "No"
            };

            let type_str = format!("{:?}", field.type_);

            let ref_to = if field.reference_to.is_empty() {
                String::new()
            } else {
                field.reference_to.join(", ")
            };

            if include_usage {
                let usage_str = usage_map
                    .get(&field.name)
                    .map_or_else(|| "N/A".to_string(), |u| format!("{:.1}%", u.percentage));

                md.push_str(&format!(
                    "| {} | `{}` | {} | {} | {} | {} |\n",
                    field.label, field.name, type_str, required, ref_to, usage_str
                ));
            } else {
                md.push_str(&format!(
                    "| {} | `{}` | {} | {} | {} |\n",
                    field.label, field.name, type_str, required, ref_to
                ));
            }
        }

        Ok(md)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::builder;
    use crate::test_support::{MockAuthenticator, Must};
    use serde_json::json;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_generate_dictionary() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        let id_field = json!({
            "name": "Id", "type": "id", "label": "Account ID", "nillable": false,
            "defaultedOnCreate": true, "referenceTo": [],
            // Padding required fields for parsing
            "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
            "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": false,
            "dependentPicklist": false, "deprecatedAndHidden": false, "digits": 0,
            "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": true, "length": 18, "nameField": false, "namePointing": false,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0,
            "queryByDistance": false, "restrictedDelete": false, "restrictedPicklist": false,
            "scale": 0, "soapType": "tns:ID", "sortable": true, "unique": false, "updateable": false,
            "writeRequiresMasterRead": false
        });

        let name_field = json!({
            "name": "Name", "type": "string", "label": "Account Name", "nillable": false,
            "defaultedOnCreate": false, "referenceTo": [],
            // Padding
            "aggregatable": true, "autoNumber": false, "byteLength": 255, "calculated": false,
            "cascadeDelete": false, "caseSensitive": false, "createable": true, "custom": false,
            "dependentPicklist": false, "deprecatedAndHidden": false, "digits": 0,
            "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": false, "length": 255, "nameField": true, "namePointing": false,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0,
            "queryByDistance": false, "restrictedDelete": false, "restrictedPicklist": false,
            "scale": 0, "soapType": "xsd:string", "sortable": true, "unique": false, "updateable": true,
            "writeRequiresMasterRead": false
        });

        let parent_id_field = json!({
            "name": "ParentId", "type": "reference", "label": "Parent Account ID", "nillable": true,
            "defaultedOnCreate": false, "referenceTo": ["Account"],
            // Padding
            "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
            "cascadeDelete": false, "caseSensitive": false, "createable": true, "custom": false,
            "dependentPicklist": false, "deprecatedAndHidden": false, "digits": 0,
            "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": false, "length": 18, "nameField": false, "namePointing": false,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0,
            "queryByDistance": false, "restrictedDelete": false, "restrictedPicklist": false,
            "scale": 0, "soapType": "tns:ID", "sortable": true, "unique": false, "updateable": true,
            "writeRequiresMasterRead": false
        });

        let describe_json = json!({
            "name": "Account", "label": "Account", "custom": false, "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "001", "labelPlural": "Accounts", "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": [id_field, name_field, parent_id_field]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/sobjects/Account/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .mount(&mock_server)
            .await;

        let query_json = json!({
            "totalSize": 1, "done": true,
            "records": [{
                "attributes": { "type": "AggregateResult", "url": "..." },
                "total": 100, "f0": 100, "f1": 95, "f2": 25
            }]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .and(query_param("q", "SELECT COUNT(Id) total, COUNT(Id) f0, COUNT(Name) f1, COUNT(ParentId) f2 FROM Account"))
            .respond_with(ResponseTemplate::new(200).set_body_json(query_json))
            .mount(&mock_server)
            .await;

        let dict = DataDictionary::new(&client);
        let md = dict.generate("Account", true).await.must();

        assert!(md.contains("# Data Dictionary: Account"));
        assert!(md.contains("**API Name:** `Account`"));
        assert!(md.contains("| Account ID | `Id` | Id | No |  | 100.0% |"));
        assert!(md.contains("| Account Name | `Name` | String | Yes |  | 95.0% |"));
        assert!(
            md.contains("| Parent Account ID | `ParentId` | Reference | No | Account | 25.0% |")
        );
    }
}
