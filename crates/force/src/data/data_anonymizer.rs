//! Data anonymization utility for Salesforce records.

use crate::api::rest::describe::{FieldType, SObjectDescribe};
use crate::types::DynamicSObject;

/// Utility for anonymizing sensitive data in Salesforce records.
#[derive(Debug, Default)]
pub struct DataAnonymizer;

impl DataAnonymizer {
    /// Creates a new data anonymizer.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Anonymizes a record in-place based on its describe metadata.
    pub fn anonymize_record(&self, record: &mut DynamicSObject, describe: &SObjectDescribe) {
        for field in &describe.fields {
            if record.has_field(&field.name) {
                // If it's explicitly encrypted or a known sensitive type, mask it
                if field.encrypted {
                    record.set_field(&field.name, "***-**-****");
                } else {
                    match field.type_ {
                        FieldType::Email => {
                            record.set_field(&field.name, "anonymized@example.com");
                        }
                        FieldType::Phone => {
                            record.set_field(&field.name, "555-0000");
                        }
                        _ => {
                            // Leave other fields untouched
                        }
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
    use crate::test_support::Must;
    use crate::types::{Attributes, SalesforceId};

    fn mock_field(name: &str, type_: FieldType, encrypted: bool) -> FieldDescribe {
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
            encrypted,
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
            nillable: true,
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
    fn test_anonymize_record() {
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
            label: "Account".to_string(),
            label_plural: "Accounts".to_string(),
            layoutable: true,
            mergeable: true,
            mru_enabled: true,
            name: "Account".to_string(),
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
                mock_field("Email", FieldType::Email, false),
                mock_field("Phone", FieldType::Phone, false),
                mock_field("SSN__c", FieldType::String, true), // Encrypted
                mock_field("Name", FieldType::String, false),
            ],
        };

        let dummy_id = SalesforceId::new("001000000000000AAA").must();
        let attrs = Attributes::new("Account", &dummy_id, "v60.0");
        let mut record = DynamicSObject::new(attrs);

        record.set_field("Email", "john.doe@example.com");
        record.set_field("Phone", "555-1234");
        record.set_field("SSN__c", "123-45-678");
        record.set_field("Name", "John Doe");

        let anonymizer = DataAnonymizer::new();
        anonymizer.anonymize_record(&mut record, &describe);

        assert_eq!(
            record.get_field("Email").and_then(|v| v.as_str()),
            Some("anonymized@example.com")
        );
        assert_eq!(
            record.get_field("Phone").and_then(|v| v.as_str()),
            Some("555-0000")
        );
        assert_eq!(
            record.get_field("SSN__c").and_then(|v| v.as_str()),
            Some("***-**-****")
        );
        assert_eq!(
            record.get_field("Name").and_then(|v| v.as_str()),
            Some("John Doe") // Unchanged
        );
    }
}
