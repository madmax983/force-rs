//! Go struct generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
#[cfg(feature = "schema")]
use std::fmt::Write;

/// Generates a Go struct definition from an SObject describe result.
#[cfg(feature = "schema")]
pub fn generate_go_struct(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_go_struct(&mut out, describe);
    out
}

/// Writes a Go struct definition from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_go_struct(out: &mut String, describe: &SObjectDescribe) {
    let struct_name = pascal_case(&describe.name);
    let _ = writeln!(
        out,
        "// {} represents the Salesforce object: {}",
        struct_name, describe.label
    );
    let _ = writeln!(out, "type {} struct {{", struct_name);

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in fields {
        let go_type = map_type(&field.type_);
        let type_prefix = if field.nillable { "*" } else { "" };

        let mut field_name = pascal_case(&field.name);
        if field_name == "Id" {
            field_name = "ID".to_string();
        }

        let omit_empty = if field.nillable { ",omitempty" } else { "" };

        let _ = writeln!(
            out,
            "\t{} {}{} `json:\"{}{}\"` // {}",
            field_name, type_prefix, go_type, field.name, omit_empty, field.label
        );
    }

    out.push_str("}\n");
}

/// Converts a string to PascalCase for Go exported fields.
#[cfg(feature = "schema")]
fn pascal_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = true;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

/// Maps a Salesforce `FieldType` to a Go type.
#[cfg(feature = "schema")]
fn map_type(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Boolean => "bool",
        FieldType::Int => "int64",
        FieldType::Double | FieldType::Currency | FieldType::Percent => "float64",
        _ => "string",
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::types::describe::FieldDescribe;

    fn mock_field(name: &str, type_: FieldType, nillable: bool, updateable: bool) -> FieldDescribe {
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
    fn test_go_generator() {
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
                mock_field("Id", FieldType::Id, false, false),
                mock_field("Name", FieldType::String, false, true),
                mock_field("NumberOfEmployees", FieldType::Int, true, true),
                mock_field("IsActive", FieldType::Boolean, true, true),
                mock_field("Custom_Field__c", FieldType::String, true, true),
            ],
        };

        let go_code = generate_go_struct(&describe);

        let expected = "// Account represents the Salesforce object: Account\n\
type Account struct {\n\
\tID string `json:\"Id\"` // Id\n\
\tCustomFieldC *string `json:\"Custom_Field__c,omitempty\"` // Custom_Field__c\n\
\tIsActive *bool `json:\"IsActive,omitempty\"` // IsActive\n\
\tName string `json:\"Name\"` // Name\n\
\tNumberOfEmployees *int64 `json:\"NumberOfEmployees,omitempty\"` // NumberOfEmployees\n\
}\n";
        assert_eq!(go_code, expected);
    }
}
