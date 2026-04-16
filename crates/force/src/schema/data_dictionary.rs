//! Data Dictionary Generator.
//!
//! This module provides a utility to generate Markdown data dictionaries
//! for Salesforce SObjects, optionally including field usage statistics.

use crate::api::rest_operation::RestOperation;
use crate::auth::Authenticator;
use crate::client::ForceClient;
use crate::error::Result;
use std::collections::HashMap;
use std::fmt::Write;

use super::scanner::FieldUsageScanner;

/// Generates a Data Dictionary for an SObject.
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
            usage_map.reserve(usages.len());
            for usage in usages {
                // ⚡ Bolt: Moving `usage.name` directly into the map avoids an unnecessary `.clone()` allocation,
                // and storing only the `f64` percentage reduces memory usage vs storing the entire struct.
                usage_map.insert(usage.name, usage.percentage);
            }
        }

        let mut md = String::with_capacity(1024);

        // ⚡ Bolt: Use `writeln!` directly to the `md` buffer instead of `format!` and `push_str`
        // to avoid intermediate String heap allocations for the header and each table row.
        let _ = writeln!(md, "# Data Dictionary: {}", describe.label);
        let _ = writeln!(md, "**API Name:** `{}`", describe.name);
        let _ = writeln!(md, "**Custom:** {}", describe.custom);
        md.push('\n');

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

            let _ = write!(
                md,
                "| {} | `{}` | {:?} | {} | ",
                field.label, field.name, field.type_, required
            );

            // ⚡ Bolt: Write reference_to directly without allocating a `join(", ")` String.
            let mut first = true;
            for r in &field.reference_to {
                if !first {
                    md.push_str(", ");
                }
                md.push_str(r);
                first = false;
            }

            if include_usage {
                if let Some(percentage) = usage_map.get(&field.name) {
                    let _ = writeln!(md, " | {:.1}% |", percentage);
                } else {
                    md.push_str(" | N/A |\n");
                }
            } else {
                md.push_str(" |\n");
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
    async fn test_generate_dictionary_without_usage() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        let describe_json = serde_json::from_str::<serde_json::Value>(r#"{
            "name": "Account",
            "label": "Account Object",
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
                    "name": "Id", "type": "id", "label": "Account ID", "createable": false,
                    "autoNumber": false, "calculated": false, "custom": false, "nillable": false,
                    "defaultedOnCreate": true, "referenceTo": [],
                    "aggregatable": true, "byteLength": 18,
                    "cascadeDelete": false, "caseSensitive": false,
                    "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 18, "nameField": false, "namePointing": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:ID",
                    "sortable": true, "unique": true, "updateable": false, "writeRequiresMasterRead": false
                },
                {
                    "name": "Name", "type": "string", "label": "Account Name", "createable": true,
                    "autoNumber": false, "calculated": false, "custom": false, "nillable": false,
                    "defaultedOnCreate": false, "referenceTo": [],
                    "aggregatable": true, "byteLength": 255,
                    "cascadeDelete": false, "caseSensitive": false,
                    "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 255, "nameField": true, "namePointing": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:string",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false
                },
                {
                    "name": "ParentId", "type": "reference", "label": "Parent Account", "createable": true,
                    "autoNumber": false, "calculated": false, "custom": false, "nillable": true,
                    "defaultedOnCreate": false, "referenceTo": ["Account"],
                    "aggregatable": true, "byteLength": 18,
                    "cascadeDelete": false, "caseSensitive": false,
                    "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 18, "nameField": false, "namePointing": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:ID",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false
                }
            ]
        }"#).must();

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/sobjects/Account/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .mount(&mock_server)
            .await;

        let dict = DataDictionary::new(&client);
        let md = dict.generate("Account", false).await.must();

        assert!(md.contains("# Data Dictionary: Account Object"));
        assert!(md.contains("**API Name:** `Account`"));
        assert!(md.contains("**Custom:** false"));

        // Table header without usage
        assert!(md.contains("| Label | API Name | Type | Required | Reference To |"));

        // Field rows
        assert!(md.contains("| Account ID | `Id` | Id | No |  |")); // DefaultedOnCreate = true -> Required = No
        assert!(md.contains("| Account Name | `Name` | String | Yes |  |"));
        assert!(md.contains("| Parent Account | `ParentId` | Reference | No | Account |"));
    }

    #[tokio::test]
    async fn test_generate_dictionary_with_usage() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        // 1. Describe Mock
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
                    "autoNumber": false, "calculated": false, "custom": false, "nillable": false,
                    "defaultedOnCreate": false, "referenceTo": [],
                    "aggregatable": true, "byteLength": 255,
                    "cascadeDelete": false, "caseSensitive": false,
                    "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 255, "nameField": true, "namePointing": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:string",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false
                },
                {
                    "name": "Website", "type": "url", "label": "Website", "createable": true,
                    "autoNumber": false, "calculated": false, "custom": false, "nillable": true,
                    "defaultedOnCreate": false, "referenceTo": [],
                    "aggregatable": true, "byteLength": 255,
                    "cascadeDelete": false, "caseSensitive": false,
                    "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 255, "nameField": false, "namePointing": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:string",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false
                }
            ]
        }"#).must();

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/sobjects/Account/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .mount(&mock_server)
            .await;

        // 2. Query Mock (from Scanner)
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .and(query_param(
                "q",
                "SELECT COUNT(Id) total, COUNT(Name) f0, COUNT(Website) f1 FROM Account",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 1,
                "done": true,
                "records": [
                    {
                        "attributes": {"type": "AggregateResult"},
                        "total": 100,
                        "f0": 100, // Name 100%
                        "f1": 25   // Website 25%
                    }
                ]
            })))
            .mount(&mock_server)
            .await;

        let dict = DataDictionary::new(&client);
        let md = dict.generate("Account", true).await.must();

        // Table header with usage
        assert!(md.contains("| Label | API Name | Type | Required | Reference To | Populated % |"));

        // Field rows
        assert!(md.contains("| Account Name | `Name` | String | Yes |  | 100.0% |"));
        assert!(md.contains("| Website | `Website` | Url | No |  | 25.0% |"));
    }
}
