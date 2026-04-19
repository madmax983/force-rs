//! Salesforce Schema Diff Utility.
//!
//! This module provides the `compare_schemas` utility function to compare two `SObjectDescribe`
//! payloads. It identifies added fields, removed fields, and fields that have
//! changed types. This is useful for tracking schema evolution over time.
//!
//! # Example
//!
//! ```no_run
//! # use force::api::RestOperation;
//! # use force::client::ForceClientBuilder;
//! # use force::schema::compare_schemas;
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
//! let diff = compare_schemas(&describe_v1, &describe_v2);
//!
//! println!("Added fields: {:?}", diff.added.len());
//! println!("Removed fields: {:?}", diff.removed.len());
//! println!("Changed fields: {:?}", diff.changed.len());
//! # Ok(())
//! # }
//! ```

use crate::api::rest::describe::{FieldDescribe, FieldType, SObjectDescribe};
use std::collections::HashMap;

/// Represents a change in a field's definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldChange {
    /// The name of the field.
    pub name: String,
    /// The old type of the field.
    pub old_type: FieldType,
    /// The new type of the field.
    pub new_type: FieldType,
}

/// The result of comparing two schema definitions.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SchemaDiffResult {
    /// Fields that were added in the new schema.
    pub added: Vec<FieldDescribe>,
    /// Fields that were removed in the new schema.
    pub removed: Vec<FieldDescribe>,
    /// Fields whose types have changed.
    pub changed: Vec<FieldChange>,
}

impl SchemaDiffResult {
    /// Returns true if there are no differences between the schemas.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }
}

/// Compares two `SObjectDescribe` payloads and returns a `SchemaDiffResult`.
///
/// # Arguments
///
/// * `old_schema` - The original SObject describe payload.
/// * `new_schema` - The updated SObject describe payload.
///
/// # Returns
///
/// A `SchemaDiffResult` containing added, removed, and changed fields.
#[must_use]
pub fn compare_schemas(
    old_schema: &SObjectDescribe,
    new_schema: &SObjectDescribe,
) -> SchemaDiffResult {
    let mut result = SchemaDiffResult::default();

    // ⚡ Bolt: Use .as_str() directly in the map instead of doing a heap allocation (.clone())
    let mut old_fields: HashMap<&str, &FieldDescribe> =
        HashMap::with_capacity(old_schema.fields.len());
    for field in &old_schema.fields {
        old_fields.insert(field.name.as_str(), field);
    }

    // Find added and changed fields
    // ⚡ Bolt: Use new_field.name.as_str() instead of doing a heap allocation (.clone())
    for new_field in &new_schema.fields {
        if let Some(old_field) = old_fields.remove(new_field.name.as_str()) {
            if old_field.type_ != new_field.type_ {
                result.changed.push(FieldChange {
                    name: new_field.name.clone(),
                    old_type: old_field.type_.clone(),
                    new_type: new_field.type_.clone(),
                });
            }
        } else {
            result.added.push(new_field.clone());
        }
    }

    // Find removed fields
    // ⚡ Bolt: Using `extend` automatically pre-allocates the exact capacity needed from the iterator's size hint, preventing multiple vector reallocations.
    result.removed.extend(old_fields.into_values().cloned());

    // Sort to ensure deterministic output
    result
        .added
        .sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));
    result
        .removed
        .sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));
    result
        .changed
        .sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    result
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
    fn test_schema_diff_no_changes() {
        let old_schema = create_mock_describe(&json!([
            mock_field("Id", "id"),
            mock_field("Name", "string")
        ]));

        let new_schema = create_mock_describe(&json!([
            mock_field("Id", "id"),
            mock_field("Name", "string")
        ]));

        let diff = compare_schemas(&old_schema, &new_schema);

        assert!(diff.is_empty());
        assert_eq!(diff.added.len(), 0);
        assert_eq!(diff.removed.len(), 0);
        assert_eq!(diff.changed.len(), 0);
    }

    #[test]
    fn test_schema_diff_added_fields() {
        let old_schema = create_mock_describe(&json!([mock_field("Id", "id")]));

        let new_schema = create_mock_describe(&json!([
            mock_field("Id", "id"),
            mock_field("Name", "string"),
            mock_field("Website", "url")
        ]));

        let diff = compare_schemas(&old_schema, &new_schema);

        assert!(!diff.is_empty());
        assert_eq!(diff.added.len(), 2);
        assert_eq!(diff.removed.len(), 0);
        assert_eq!(diff.changed.len(), 0);

        assert_eq!(diff.added[0].name, "Name");
        assert_eq!(diff.added[1].name, "Website");
    }

    #[test]
    fn test_schema_diff_removed_fields() {
        let old_schema = create_mock_describe(&json!([
            mock_field("Id", "id"),
            mock_field("Name", "string"),
            mock_field("Website", "url")
        ]));

        let new_schema = create_mock_describe(&json!([mock_field("Id", "id")]));

        let diff = compare_schemas(&old_schema, &new_schema);

        assert!(!diff.is_empty());
        assert_eq!(diff.added.len(), 0);
        assert_eq!(diff.removed.len(), 2);
        assert_eq!(diff.changed.len(), 0);

        assert_eq!(diff.removed[0].name, "Name");
        assert_eq!(diff.removed[1].name, "Website");
    }

    #[test]
    fn test_schema_diff_changed_fields() {
        let old_schema =
            create_mock_describe(&json!([mock_field("Id", "id"), mock_field("Age", "int")]));

        let new_schema = create_mock_describe(&json!([
            mock_field("Id", "id"),
            mock_field("Age", "double")
        ]));

        let diff = compare_schemas(&old_schema, &new_schema);

        assert!(!diff.is_empty());
        assert_eq!(diff.added.len(), 0);
        assert_eq!(diff.removed.len(), 0);
        assert_eq!(diff.changed.len(), 1);

        assert_eq!(diff.changed[0].name, "Age");
        assert_eq!(diff.changed[0].old_type, FieldType::Int);
        assert_eq!(diff.changed[0].new_type, FieldType::Double);
    }
}
