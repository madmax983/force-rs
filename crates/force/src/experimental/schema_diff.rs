//! SObject Schema Diff Utility.
//!
//! This module provides a utility to compare the schema of two SObjects
//! or the same SObject across two different environments.
//! It's a useful tool for developers migrating metadata between Orgs
//! or identifying what fields were added/removed in a new package version.

#[cfg(feature = "nova")]
use crate::api::rest::describe::{FieldDescribe, SObjectDescribe};
#[cfg(feature = "nova")]
use std::collections::HashMap;

/// The result of comparing two schemas.
#[cfg(feature = "nova")]
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaDiffResult {
    /// Fields added in the new schema.
    pub added_fields: Vec<FieldDescribe>,
    /// Fields removed in the new schema (just names).
    pub removed_fields: Vec<String>,
    /// Fields whose type changed. (Field name -> (Old Type, New Type))
    pub changed_types: HashMap<String, (String, String)>,
}

/// SObject Schema Diff Utility.
///
/// Use `SchemaDiff::diff` to compare two `SObjectDescribe` objects.
#[cfg(feature = "nova")]
pub struct SchemaDiff;

#[cfg(feature = "nova")]
impl SchemaDiff {
    /// Compares an old SObject schema with a new SObject schema.
    ///
    /// # Arguments
    ///
    /// * `old_schema` - The original SObject describe metadata.
    /// * `new_schema` - The updated SObject describe metadata.
    ///
    /// # Returns
    ///
    /// Returns a `SchemaDiffResult` detailing what was added, removed, or changed.
    #[must_use]
    pub fn diff(old_schema: &SObjectDescribe, new_schema: &SObjectDescribe) -> SchemaDiffResult {
        let mut added_fields = Vec::new();
        let mut removed_fields = Vec::new();
        let mut changed_types = HashMap::new();

        let mut old_fields = HashMap::with_capacity(old_schema.fields.len());
        for field in &old_schema.fields {
            old_fields.insert(field.name.clone(), field);
        }

        let mut new_fields = HashMap::with_capacity(new_schema.fields.len());
        for field in &new_schema.fields {
            new_fields.insert(field.name.clone(), field);
        }

        // Find added and changed fields
        for (name, new_field) in &new_fields {
            if let Some(old_field) = old_fields.get(name) {
                // To compare enums without Eq, we can compare their JSON representation.
                // It's a bit hacky but works for this experimental diff utility.
                let old_type_str = serde_json::to_string(&old_field.type_).unwrap_or_default();
                let new_type_str = serde_json::to_string(&new_field.type_).unwrap_or_default();

                if old_type_str != new_type_str {
                    changed_types.insert(name.clone(), (old_type_str, new_type_str));
                }
            } else {
                added_fields.push((*new_field).clone());
            }
        }

        // Find removed fields
        for name in old_fields.keys() {
            if !new_fields.contains_key(name) {
                removed_fields.push(name.clone());
            }
        }

        SchemaDiffResult {
            added_fields,
            removed_fields,
            changed_types,
        }
    }
}

#[cfg(all(test, feature = "nova"))]
mod tests {
    use super::*;
    use crate::api::rest::describe::FieldType;

    fn dummy_field(name: &str, field_type: FieldType) -> FieldDescribe {
        FieldDescribe {
            aggregatable: false,
            auto_number: false,
            byte_length: 0,
            calculated: false,
            calculated_formula: None,
            cascade_delete: false,
            case_sensitive: false,
            compound_field_name: None,
            controller_name: None,
            createable: false,
            custom: false,
            default_value: None,
            default_value_formula: None,
            defaulted_on_create: false,
            dependent_picklist: false,
            deprecated_and_hidden: false,
            digits: 0,
            display_location_in_decimal: false,
            encrypted: false,
            external_id: false,
            extra_type_info: None,
            filterable: false,
            filtered_lookup_info: None,
            groupable: false,
            high_scale_number: false,
            html_formatted: false,
            id_lookup: false,
            inline_help_text: None,
            label: name.to_string(),
            length: 0,
            mask: None,
            mask_type: None,
            name: name.to_string(),
            name_field: false,
            name_pointing: false,
            nillable: false,
            permissionable: false,
            picklist_values: Some(vec![]),
            polymorphic_foreign_key: false,
            precision: 0,
            query_by_distance: false,
            reference_target_field: None,
            reference_to: vec![],
            relationship_name: None,
            relationship_order: None,
            restricted_delete: false,
            restricted_picklist: false,
            scale: 0,
            search_prefixes_supported: None,
            formula_treat_blanks_as: None,
            soap_type: "xsd:string".to_string(),
            sortable: false,
            type_: field_type,
            unique: false,
            updateable: false,
            write_requires_master_read: false,
        }
    }

    fn dummy_describe(fields: Vec<FieldDescribe>) -> SObjectDescribe {
        SObjectDescribe {
            activateable: false,
            createable: false,
            custom: false,
            custom_setting: false,
            deletable: false,
            deprecated_and_hidden: false,
            feed_enabled: false,
            has_subtypes: false,
            is_subtype: false,
            key_prefix: None,
            label: "Test".to_string(),
            label_plural: "Tests".to_string(),
            layoutable: false,
            mergeable: false,
            mru_enabled: false,
            name: "Test".to_string(),
            queryable: false,
            replicateable: false,
            retrieveable: false,
            searchable: false,
            triggerable: false,
            undeletable: false,
            updateable: false,
            urls: std::collections::HashMap::new(),
            fields,
            child_relationships: vec![],
            record_type_infos: vec![],
        }
    }

    #[test]
    fn test_schema_diff() {
        let f1 = dummy_field("Id", FieldType::Id);
        let f2 = dummy_field("Name", FieldType::String);
        let f3_old = dummy_field("Age", FieldType::Int);
        let f3_new = dummy_field("Age", FieldType::Double);
        let f4 = dummy_field("Active", FieldType::Boolean);

        let old_schema = dummy_describe(vec![f1.clone(), f2.clone(), f3_old.clone()]);
        let new_schema = dummy_describe(vec![f1.clone(), f3_new.clone(), f4.clone()]);

        let diff = SchemaDiff::diff(&old_schema, &new_schema);

        assert_eq!(diff.added_fields.len(), 1);
        assert_eq!(diff.added_fields[0].name, "Active");

        assert_eq!(diff.removed_fields.len(), 1);
        assert_eq!(diff.removed_fields[0], "Name");

        assert_eq!(diff.changed_types.len(), 1);
        let (old_t, new_t) = diff.changed_types.get("Age").unwrap();
        assert_eq!(old_t, "\"int\"");
        assert_eq!(new_t, "\"double\"");
    }
}
