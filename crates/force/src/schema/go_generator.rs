//! Golang struct generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates a Golang struct definition from an SObject describe result.
#[cfg(feature = "schema")]
pub fn generate_go_struct(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_go_struct(&mut out, describe);
    out
}

/// Writes a Golang struct definition from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_go_struct(out: &mut String, describe: &SObjectDescribe) {
    let _ = writeln!(out, "type {} struct {{", describe.name);

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in fields {
        let go_type = map_type(&field.type_);
        let pointer = if field.nillable { "*" } else { "" };

        let mut go_field_name = field.name.clone();
        if let Some(c) = go_field_name.chars().next() {
            go_field_name.replace_range(..c.len_utf8(), &c.to_uppercase().to_string());
        }

        let _ = writeln!(
            out,
            "\t{} {}{} `json:\"{}\"`",
            go_field_name, pointer, go_type, field.name
        );
    }

    out.push_str("}\n");
}

/// Maps a Salesforce `FieldType` to a Golang primitive.
#[cfg(feature = "schema")]
fn map_type(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Boolean => "bool",
        FieldType::Int => "int",
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
                mock_field("customField__c", FieldType::String, true, true),
            ],
        };
        let go_code = generate_go_struct(&describe);
        let expected = "type Account struct {\n\tId string `json:\"Id\"`\n\tIsActive *bool `json:\"IsActive\"`\n\tName string `json:\"Name\"`\n\tNumberOfEmployees *int `json:\"NumberOfEmployees\"`\n\tCustomField__c *string `json:\"customField__c\"`\n}\n";
        assert_eq!(go_code, expected);
    }
}
