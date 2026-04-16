//! Data Masking utility for Salesforce.
//!
//! Provides `DataMasker` to obfuscate sensitive data in records according to their field types.

use crate::api::rest::describe::{FieldType, SObjectDescribe};
use serde_json::Value;
use std::collections::HashMap;

/// Utility for masking sensitive fields in records.
pub struct DataMasker;

impl DataMasker {
    /// Masks sensitive fields in the given record based on the SObject schema.
    ///
    /// Modifies the `record` in place.
    pub fn mask_record(record: &mut Value, describe: &SObjectDescribe) {
        let Value::Object(map) = record else {
            return; // Only objects can be masked
        };

        // Pre-compute field types to avoid repeated lookups
        let field_types: HashMap<&str, &FieldType> = describe
            .fields
            .iter()
            .map(|f| (f.name.as_str(), &f.type_))
            .collect();

        for (key, val) in map.iter_mut() {
            if let Some(field_type) = field_types.get(key.as_str()) {
                if val.is_string() {
                    match field_type {
                        FieldType::Email => {
                            *val = Value::String("masked@example.com".to_string());
                        }
                        FieldType::Phone => {
                            *val = Value::String("555-0000".to_string());
                        }
                        FieldType::Url => {
                            *val = Value::String("https://masked.example.com".to_string());
                        }
                        FieldType::Textarea => {
                            *val = Value::String("***".to_string());
                        }
                        _ => {} // Other types are not masked by default
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::rest::describe::FieldDescribe;

    fn mock_field(name: &str, type_: FieldType) -> FieldDescribe {
        FieldDescribe {
            aggregatable: true,
            auto_number: false,
            byte_length: 18,
            calculated: false,
            calculated_formula: None,
            cascade_delete: false,
            case_sensitive: false,
            compound_field_name: None,
            controller_name: None,
            createable: true,
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
            filterable: true,
            filtered_lookup_info: None,
            formula_treat_blanks_as: None,
            groupable: true,
            high_scale_number: false,
            html_formatted: false,
            id_lookup: name == "Id",
            inline_help_text: None,
            label: name.to_string(),
            length: 18,
            mask: None,
            mask_type: None,
            name: name.to_string(),
            name_field: name == "Name",
            name_pointing: false,
            nillable: false,
            permissionable: false,
            picklist_values: None,
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
            soap_type: "xsd:string".to_string(),
            sortable: true,
            type_,
            unique: false,
            updateable: true,
            write_requires_master_read: false,
        }
    }

    #[test]
    fn test_mask_record() {
        let describe = SObjectDescribe {
            activateable: false,
            createable: true,
            custom: false,
            custom_setting: false,
            deletable: true,
            deprecated_and_hidden: false,
            feed_enabled: false,
            has_subtypes: false,
            is_subtype: false,
            key_prefix: Some("001".to_string()),
            label: "Contact".to_string(),
            label_plural: "Contacts".to_string(),
            layoutable: true,
            mergeable: true,
            mru_enabled: true,
            name: "Contact".to_string(),
            queryable: true,
            replicateable: true,
            retrieveable: true,
            searchable: true,
            triggerable: true,
            undeletable: true,
            updateable: true,
            urls: std::collections::HashMap::new(),
            child_relationships: vec![],
            record_type_infos: vec![],
            fields: vec![
                mock_field("Id", FieldType::Id),
                mock_field("Email", FieldType::Email),
                mock_field("Phone", FieldType::Phone),
                mock_field("Website", FieldType::Url),
                mock_field("Description", FieldType::Textarea),
            ],
        };

        let mut record = serde_json::json!({
            "Id": "003000000000000",
            "Email": "john.doe@example.com",
            "Phone": "123-456-7890",
            "Website": "https://example.com",
            "Description": "Secret notes here."
        });

        DataMasker::mask_record(&mut record, &describe);

        assert_eq!(record["Id"], "003000000000000", "Id should not be masked");
        assert_eq!(
            record["Email"], "masked@example.com",
            "Email should be masked"
        );
        assert_eq!(record["Phone"], "555-0000", "Phone should be masked");
        assert_eq!(
            record["Website"], "https://masked.example.com",
            "Url should be masked"
        );
        assert_eq!(record["Description"], "***", "Textarea should be masked");
    }
}
