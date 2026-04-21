//! TypeScript interface generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Preview utility to generate TypeScript interfaces from SObject describe metadata.
#[cfg(feature = "schema")]
/// Generates a TypeScript interface definition from an SObject describe result.
pub fn generate_typescript_interface(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_typescript_interface(&mut out, describe);
    out
}

/// Writes a TypeScript interface definition from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_typescript_interface(out: &mut String, describe: &SObjectDescribe) {
    out.push_str("/**\n");
    let _ = writeln!(out, " * {}", describe.label);
    out.push_str(" */\n");
    let _ = writeln!(out, "export interface {} {{", describe.name);

    // Sort fields alphabetically, Id first
    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in fields {
        out.push_str("  /**\n");
        let _ = writeln!(out, "   * {}", field.label);
        if let Some(help) = &field.inline_help_text {
            let _ = writeln!(out, "   * {}", help);
        }
        if !field.updateable {
            out.push_str("   * @readonly\n");
        }
        out.push_str("   */\n");

        let ts_type = map_type(&field.type_);
        let optional = if field.nillable { "?" } else { "" };

        let _ = writeln!(out, "  {}{}: {};", field.name, optional, ts_type);
    }

    out.push_str("}\n");
}

/// Maps a Salesforce `FieldType` to a TypeScript type.
fn map_type(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Boolean => "boolean",
        FieldType::Int | FieldType::Double | FieldType::Currency | FieldType::Percent => "number",
        // Typically serialized as ISO 8601 strings
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
    fn test_ts_generator() {
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

        let ts = generate_typescript_interface(&describe);

        let expected = "/**\n * Account\n */\nexport interface Account {\n  /**\n   * Id\n   * @readonly\n   */\n  Id: string;\n  /**\n   * IsActive\n   */\n  IsActive?: boolean;\n  /**\n   * Name\n   */\n  Name: string;\n  /**\n   * NumberOfEmployees\n   */\n  NumberOfEmployees?: number;\n}\n";
        assert_eq!(ts, expected);
    }
}
