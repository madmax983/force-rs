//! Data Dictionary Generator.
//!
//! This module provides a utility to generate a Markdown data dictionary
//! for a Salesforce SObject, combining Describe API metadata with optional
//! Field Usage Scanner statistics.
//!
//! This serves as a powerful "Exporter" to document schema directly from the API.

use crate::api::rest_operation::RestOperation;
use crate::auth::Authenticator;
use crate::client::ForceClient;
use crate::error::Result;
use std::collections::HashMap;
use std::fmt::Write;

use super::scanner::scan_field_usage;

/// Generator for SObject data dictionaries in Markdown format.

/// Generates a Markdown data dictionary for the specified SObject.
///
/// # Arguments
///
/// * `client` - The authenticated Force client.
/// * `sobject` - The API name of the SObject (e.g., "Account").
/// * `include_usage` - Whether to scan and include field population statistics.
///
/// # Returns
///
/// A String containing the generated Markdown document.
pub async fn generate_data_dictionary<A: Authenticator>(
    client: &ForceClient<A>,
    sobject: &str,
    include_usage: bool,
) -> Result<String> {
    let describe = client.rest().describe(sobject).await?;

    let mut usage_map = HashMap::new();
    if include_usage {
        let usages = scan_field_usage(client, sobject).await?;
        for usage in usages {
            usage_map.insert(usage.name.clone(), usage);
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
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

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
            if let Some(usage) = usage_map.get(&field.name) {
                let _ = writeln!(md, " | {:.1}% |", usage.percentage);
            } else {
                md.push_str(" | N/A |\n");
            }
        } else {
            md.push_str(" |\n");
        }
    }

    Ok(md)
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
        let auth = MockAuthenticator::new("token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        let id_field = json!({
            "name": "Id", "type": "id", "label": "Account ID", "nillable": false,
            "defaultedOnCreate": true, "referenceTo": [],
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
            "defaultedOnCreate": false, "referenceTo": ["Account", "Contact"],
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

        let describe_json = json!({
            "name": "Account", "label": "Account", "custom": true, "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "001", "labelPlural": "Accounts", "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": [id_field, name_field]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/sobjects/Account/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .mount(&mock_server)
            .await;

        let md = generate_data_dictionary(&client, "Account", false)
            .await
            .must();

        assert!(md.contains("# Data Dictionary: Account"));
        assert!(md.contains("**Custom:** true"));
        // Without usage, should not have Populated % column
        assert!(!md.contains("Populated %"));
        // Check the table has 5 columns, not 6
        assert!(md.contains("| Label | API Name | Type | Required | Reference To |"));
        // Check that multiple referenceTo values are comma-separated
        assert!(md.contains("Account, Contact"));
    }

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

        let md = generate_data_dictionary(&client, "Account", true)
            .await
            .must();

        assert!(md.contains("# Data Dictionary: Account"));
        assert!(md.contains("**API Name:** `Account`"));
        assert!(md.contains("| Account ID | `Id` | Id | No |  | 100.0% |"));
        assert!(md.contains("| Account Name | `Name` | String | Yes |  | 95.0% |"));
        assert!(
            md.contains("| Parent Account ID | `ParentId` | Reference | No | Account | 25.0% |")
        );
    }
}
