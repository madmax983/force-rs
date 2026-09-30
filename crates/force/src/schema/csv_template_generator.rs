//! CSV Template Generator

use crate::types::describe::SObjectDescribe;

/// Generates a CSV header template for the given SObjectDescribe based on its createable fields.
#[cfg(feature = "schema")]
pub fn generate_csv_template(describe: &SObjectDescribe) -> String {
    let mut fields: Vec<&str> = describe
        .fields
        .iter()
        .filter(|f| f.createable)
        .map(|f| f.name.as_str())
        .collect();

    fields.sort_unstable();

    let mut result = String::with_capacity(fields.iter().map(|s| s.len() + 1).sum());
    let mut first = true;
    for &field in &fields {
        if !first {
            result.push(',');
        }
        result.push_str(field);
        first = false;
    }
    result.push('\n');
    result
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::types::describe::{FieldDescribe, FieldType, SObjectDescribe};

    fn mock_field(name: &str, type_: FieldType, createable: bool) -> FieldDescribe {
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
            createable,
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
            updateable: createable,
            write_requires_master_read: false,
        }
    }

    #[test]
    fn test_generate_csv_template() {
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
                mock_field("Id", FieldType::Id, false),
                mock_field("Name", FieldType::String, true),
                mock_field("IsActive", FieldType::Boolean, true),
                mock_field("NumberOfEmployees", FieldType::Int, true),
                mock_field("NonCreateable", FieldType::String, false),
            ],
        };

        let result = generate_csv_template(&describe);

        // Assert that Id and NonCreateable are not present (not createable)
        // Assert that the fields are sorted correctly
        assert_eq!(result, "IsActive,Name,NumberOfEmployees\n");
    }
}
