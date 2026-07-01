//! Go struct generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Preview utility to generate Go structs from SObject describe metadata.
#[cfg(feature = "schema")]
/// Generates a Go struct definition from an SObject describe result.
///
/// This will generate a struct with `json` tags to match the
/// Salesforce API field names, and map the Salesforce types to appropriate
/// Go types.
pub fn generate_go_struct(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_go_struct(&mut out, describe);
    out
}

/// Writes a Go struct definition from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_go_struct(out: &mut String, describe: &SObjectDescribe) {
    let _ = writeln!(
        out,
        "// {} represents a Salesforce SObject.",
        describe.label
    );
    out.push_str("type ");
    write_pascal_case(out, &describe.name);
    out.push_str(" struct {\n");

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| {
        if a.name == "Id" {
            std::cmp::Ordering::Less
        } else if b.name == "Id" {
            std::cmp::Ordering::Greater
        } else {
            a.name.cmp(&b.name)
        }
    });

    for field in fields {
        let _ = writeln!(out, "\t// {}", field.label);
        let go_type = map_type(&field.type_);

        let mut field_name = String::new();
        write_pascal_case(&mut field_name, &field.name);

        if field.nillable || field.defaulted_on_create || !field.createable {
            let _ = writeln!(
                out,
                "\t{} *{} `json:\"{},omitempty\"`",
                field_name, go_type, field.name
            );
        } else {
            let _ = writeln!(
                out,
                "\t{} {} `json:\"{}\"`",
                field_name, go_type, field.name
            );
        }
    }

    out.push_str("}\n");
}

#[cfg(feature = "schema")]
fn write_pascal_case(out: &mut String, s: &str) {
    let mut capitalize_next = true;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            out.extend(c.to_uppercase());
            capitalize_next = false;
        } else {
            out.push(c);
        }
    }
}

/// Maps a Salesforce `FieldType` to a Go type.
#[cfg(feature = "schema")]
fn map_type(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Id
        | FieldType::Reference
        | FieldType::String
        | FieldType::Textarea
        | FieldType::Email
        | FieldType::Phone
        | FieldType::Url
        | FieldType::Picklist
        | FieldType::Multipicklist
        | FieldType::Combobox
        | FieldType::Date
        | FieldType::Datetime
        | FieldType::Time => "string", // Or time.Time, but string is safer for raw JSON
        FieldType::Boolean => "bool",
        FieldType::Int => "int",
        FieldType::Double | FieldType::Currency | FieldType::Percent => "float64",
        _ => "interface{}", // Fallback
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::types::describe::FieldDescribe;

    fn mock_field(name: &str, type_: FieldType, nillable: bool, createable: bool) -> FieldDescribe {
        FieldDescribe {
            aggregatable: true,
            auto_number: false,
            byte_length: 0,
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
            length: 0,
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
            updateable: true,
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
            ],
        };

        let go_struct = generate_go_struct(&describe);

        let expected = "// Account represents a Salesforce SObject.\ntype Account struct {\n\t// Id\n\tId *string `json:\"Id,omitempty\"`\n\t// IsActive\n\tIsActive *bool `json:\"IsActive,omitempty\"`\n\t// Name\n\tName string `json:\"Name\"`\n\t// NumberOfEmployees\n\tNumberOfEmployees *int `json:\"NumberOfEmployees,omitempty\"`\n}\n";
        assert_eq!(go_struct, expected);
    }
}
