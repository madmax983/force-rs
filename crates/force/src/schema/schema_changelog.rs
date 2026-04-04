//! Schema Changelog Generator.
//!
//! This module provides the `generate_changelog` utility function to create a human-readable
//! Markdown changelog by comparing two `SObjectDescribe` payloads using `compare_schemas`.
//!
//! # Example
//!
//! ```no_run
//! # use force::api::rest_operation::RestOperation;
//! # use force::client::ForceClientBuilder;
//! # use force::schema::generate_changelog;
//! # use force::auth::ClientCredentials;
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! # let auth = ClientCredentials::new("id", "secret", "url");
//! # let client = ForceClientBuilder::new().authenticate(auth).build().await?;
//! // Assume we have two versions of a describe payload
//! let describe_v1 = client.rest().describe("Account").await?;
//! // ... time passes, schema changes ...
//! let describe_v2 = client.rest().describe("Account").await?;
//!
//! let md_changelog = generate_changelog(&describe_v1, &describe_v2);
//! println!("{}", md_changelog);
//! # Ok(())
//! # }
//! ```

use crate::api::rest::describe::SObjectDescribe;

use super::schema_diff::compare_schemas;

/// Generates a Markdown changelog comparing an old and new schema describe.
///
/// # Arguments
///
/// * `old_schema` - The original SObject describe payload.
/// * `new_schema` - The updated SObject describe payload.
///
/// # Returns
///
/// A String containing the generated Markdown changelog.
#[must_use]
pub fn generate_changelog(old_schema: &SObjectDescribe, new_schema: &SObjectDescribe) -> String {
    let mut md = String::with_capacity(1024);
    write_changelog(&mut md, old_schema, new_schema);
    md
}

/// Writes a Markdown-formatted changelog between two schema versions into the provided buffer.
pub fn write_changelog(
    md: &mut String,
    old_schema: &SObjectDescribe,
    new_schema: &SObjectDescribe,
) {
    use std::fmt::Write;

    let diff = compare_schemas(old_schema, new_schema);

    let _ = writeln!(md, "# Schema Changelog: {}", new_schema.label);
    let _ = writeln!(md, "**API Name:** `{}`\n", new_schema.name);

    if diff.is_empty() {
        md.push_str("No changes detected.\n");
        return;
    }

    if !diff.added_fields.is_empty() {
        md.push_str("## Added Fields\n\n");
        md.push_str("| Label | API Name | Type |\n");
        md.push_str("|---|---|---|\n");
        for field in &diff.added_fields {
            let _ = writeln!(
                md,
                "| {} | `{}` | {:?} |",
                field.label, field.name, field.type_
            );
        }
        md.push('\n');
    }

    if !diff.removed_fields.is_empty() {
        md.push_str("## Removed Fields\n\n");
        md.push_str("| Label | API Name | Type |\n");
        md.push_str("|---|---|---|\n");
        for field in &diff.removed_fields {
            let _ = writeln!(
                md,
                "| {} | `{}` | {:?} |",
                field.label, field.name, field.type_
            );
        }
        md.push('\n');
    }

    if !diff.changed_fields.is_empty() {
        md.push_str("## Changed Fields\n\n");
        md.push_str("| API Name | Old Type | New Type |\n");
        md.push_str("|---|---|---|\n");
        for field in &diff.changed_fields {
            let _ = writeln!(
                md,
                "| `{}` | {:?} | {:?} |",
                field.name, field.old_type, field.new_type
            );
        }
        md.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Must;
    use serde_json::json;

    fn create_mock_describe(fields_json: &serde_json::Value) -> SObjectDescribe {
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
            "fields": fields_json.clone()
        });
        serde_json::from_value(describe_json).must()
    }

    fn mock_field(name: &str, field_type: &str) -> serde_json::Value {
        json!({
            "name": name,
            "type": field_type,
            "label": format!("{} Label", name),
            "referenceTo": [],
            "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
            "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": false,
            "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:ID",
            "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false
        })
    }

    #[test]
    fn test_schema_changelog_generator_no_changes() {
        let schema_v1 = create_mock_describe(&json!([
            mock_field("Id", "id"),
            mock_field("Name", "string")
        ]));

        let md = generate_changelog(&schema_v1, &schema_v1);

        assert!(md.contains("# Schema Changelog: Account"));
        assert!(md.contains("No changes detected."));
    }

    #[test]
    fn test_schema_changelog_generator_with_changes() {
        let schema_v1 = create_mock_describe(&json!([
            mock_field("Id", "id"),
            mock_field("Name", "string"),
            mock_field("OldField", "string")
        ]));

        let schema_v2 = create_mock_describe(&json!([
            mock_field("Id", "id"),
            mock_field("Name", "textarea"), // Changed type
            mock_field("NewField", "int")   // Added field
        ]));

        let md = generate_changelog(&schema_v1, &schema_v2);

        assert!(md.contains("# Schema Changelog: Account"));
        assert!(md.contains("**API Name:** `Account`"));

        assert!(md.contains("## Added Fields"));
        assert!(md.contains("| NewField Label | `NewField` | Int |"));

        assert!(md.contains("## Removed Fields"));
        assert!(md.contains("| OldField Label | `OldField` | String |"));

        assert!(md.contains("## Changed Fields"));
        assert!(md.contains("| `Name` | String | Textarea |"));
    }
}
