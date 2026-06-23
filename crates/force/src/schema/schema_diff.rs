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
//! # async fn main() -> force::error::Result<()> {
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

use crate::types::describe::{FieldDescribe, FieldType, SObjectDescribe};

/// Represents a change in a field's definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldChange<'a> {
    /// The name of the field.
    pub name: &'a str,
    /// The old type of the field.
    pub old_type: &'a FieldType,
    /// The new type of the field.
    pub new_type: &'a FieldType,
}

/// The result of comparing two schema definitions.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SchemaDiffResult<'a> {
    /// Fields that were added in the new schema.
    pub added: Vec<&'a FieldDescribe>,
    /// Fields that were removed in the new schema.
    pub removed: Vec<&'a FieldDescribe>,
    /// Fields whose types have changed.
    pub changed: Vec<FieldChange<'a>>,
}

impl SchemaDiffResult<'_> {
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
/// ⚡ Bolt: Uses borrowed references tied to the original describe payloads instead
/// of deep cloning string vectors and objects.
#[must_use]
pub fn compare_schemas<'a>(
    old_schema: &'a SObjectDescribe,
    new_schema: &'a SObjectDescribe,
) -> SchemaDiffResult<'a> {
    let mut result = SchemaDiffResult::default();

    // ⚡ Bolt: Sort fields and use an O(N) linear merge to avoid allocating a `HashMap`.
    // This removes the hashing overhead and map allocations entirely.
    let mut old_fields: Vec<&'a FieldDescribe> = old_schema.fields.iter().collect();
    let mut new_fields: Vec<&'a FieldDescribe> = new_schema.fields.iter().collect();

    old_fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));
    new_fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    let mut old_iter = old_fields.into_iter();
    let mut new_iter = new_fields.into_iter();

    let mut current_old = old_iter.next();
    let mut current_new = new_iter.next();

    while let (Some(old), Some(new)) = (current_old, current_new) {
        match crate::schema::cmp_field_names(&old.name, &new.name) {
            std::cmp::Ordering::Less => {
                result.removed.push(old);
                current_old = old_iter.next();
            }
            std::cmp::Ordering::Greater => {
                result.added.push(new);
                current_new = new_iter.next();
            }
            std::cmp::Ordering::Equal => {
                if old.type_ != new.type_ {
                    result.changed.push(FieldChange {
                        name: new.name.as_str(),
                        old_type: &old.type_,
                        new_type: &new.type_,
                    });
                }
                current_old = old_iter.next();
                current_new = new_iter.next();
            }
        }
    }

    // Drain remaining old fields as removed
    if let Some(old) = current_old {
        result.removed.push(old);
        result.removed.extend(old_iter);
    }

    // Drain remaining new fields as added
    if let Some(new) = current_new {
        result.added.push(new);
        result.added.extend(new_iter);
    }

    // We do not need to sort the result vectors since we traversed in sorted order.
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;
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
            "fields": fields_json
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
        assert_eq!(diff.changed[0].old_type, &FieldType::Int);
        assert_eq!(diff.changed[0].new_type, &FieldType::Double);
    }
}
