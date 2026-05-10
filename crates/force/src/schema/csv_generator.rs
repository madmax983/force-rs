//! CSV Template Generator.
//!
//! Generates a CSV template for bulk data operations based on an SObject describe metadata.
//! This is extremely useful when preparing CSV files for the Bulk API.

#[cfg(feature = "schema")]
use crate::types::describe::SObjectDescribe;
#[cfg(feature = "schema")]
use std::fmt::Write;

/// Generates a CSV template string for the given SObject describe metadata.
///
/// Outputs a header row containing all `createable` fields.
/// If `include_mock_row` is true, it also appends a second row with mock data
/// generated from the field types.
#[cfg(feature = "schema")]
#[must_use]
pub fn generate_csv_template(describe: &SObjectDescribe, include_mock_row: bool) -> String {
    let mut out = String::with_capacity(1024);
    write_csv_template(&mut out, describe, include_mock_row);
    out
}

/// Writes a CSV template directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_csv_template(out: &mut String, describe: &SObjectDescribe, include_mock_row: bool) {
    let mut createable_fields: Vec<&_> = describe.fields.iter().filter(|f| f.createable).collect();
    createable_fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    // Headers
    let mut first = true;
    for field in &createable_fields {
        if !first {
            out.push(',');
        }
        first = false;
        out.push_str(&field.name);
    }
    out.push('\n');

    // Mock Data Row
    if include_mock_row {
        let mock_data = crate::schema::generate_mock_data(describe);
        let mut first = true;
        if let Some(obj) = mock_data.as_object() {
            for field in &createable_fields {
                if !first {
                    out.push(',');
                }
                first = false;

                if let Some(val) = obj.get(&field.name) {
                    match val {
                        serde_json::Value::String(s) => {
                            // Basic CSV escaping
                            if s.contains(',') || s.contains('"') || s.contains('\n') {
                                let _ = write!(out, "\"{}\"", s.replace('"', "\"\""));
                            } else {
                                out.push_str(s);
                            }
                        }
                        serde_json::Value::Number(n) => {
                            out.push_str(&n.to_string());
                        }
                        serde_json::Value::Bool(b) => {
                            out.push_str(if *b { "true" } else { "false" });
                        }
                        serde_json::Value::Null => {}
                        _ => {
                            let s = val.to_string();
                            if s.contains(',') || s.contains('"') || s.contains('\n') {
                                let _ = write!(out, "\"{}\"", s.replace('"', "\"\""));
                            } else {
                                out.push_str(&s);
                            }
                        }
                    }
                }
            }
        }
        out.push('\n');
    }
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
    fn test_generate_csv_template_headers_only() {
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
            ],
        };

        let csv = generate_csv_template(&describe, false);
        assert_eq!(csv, "IsActive,Name,NumberOfEmployees\n");
    }

    #[test]
    fn test_generate_csv_template_with_mock_row() {
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
            ],
        };

        let csv = generate_csv_template(&describe, true);
        assert_eq!(
            csv,
            "IsActive,Name,NumberOfEmployees\ntrue,mock_string,42\n"
        );
    }
}
