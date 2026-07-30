//! CSV template generator for Salesforce SObject Describe metadata.

#[cfg(feature = "schema")]
use crate::types::describe::SObjectDescribe;

/// Preview utility to generate a CSV template from SObject describe metadata.
///
/// Generates a CSV template header from an SObject describe result.
/// Only includes createable fields to make it useful for data imports.
#[cfg(feature = "schema")]
pub fn generate_csv_template(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 32);
    write_csv_template(&mut out, describe);
    out
}

/// Writes a CSV template header from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_csv_template(out: &mut String, describe: &SObjectDescribe) {
    let mut fields: Vec<&_> = describe.fields.iter().filter(|f| f.createable).collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for (i, field) in fields.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&field.name);
    }
    out.push('\n');
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::types::describe::{FieldDescribe, FieldType};

    fn mock_field(
        name: &str,
        type_: FieldType,
        nillable: bool,
        updateable: bool,
        createable: bool,
    ) -> FieldDescribe {
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
            nillable,
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
            updateable,
            write_requires_master_read: false,
        }
    }

    #[test]
    fn test_csv_template_generator() {
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
                mock_field("Id", FieldType::Id, false, false, false), // Not createable
                mock_field("Name", FieldType::String, false, true, true),
                mock_field("NumberOfEmployees", FieldType::Int, true, true, true),
                mock_field("IsActive", FieldType::Boolean, true, true, true),
                mock_field("SystemField", FieldType::String, false, false, false), // Not createable
            ],
        };

        let csv = generate_csv_template(&describe);
        assert_eq!(csv, "IsActive,Name,NumberOfEmployees\n");
    }
}
