//! CSV template and mock data generator for Bulk API 2.0.
#[cfg(all(feature = "schema", feature = "data_utility"))]
use crate::data::data_faker::generate_mock_record;
#[cfg(feature = "schema")]
use crate::types::describe::SObjectDescribe;
#[cfg(all(feature = "schema", feature = "data_utility"))]
use std::fmt::Write;

/// Generates an empty CSV template with headers for all createable fields.
#[cfg(feature = "schema")]
#[must_use]
pub fn generate_csv_template(describe: &SObjectDescribe) -> String {
    let mut fields: Vec<&_> = describe.fields.iter().filter(|f| f.createable).collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    let mut csv = String::with_capacity(1024);
    let mut first = true;
    for field in fields {
        if !first {
            csv.push(',');
        }
        first = false;
        csv.push_str(&field.name);
    }
    csv
}

/// Generates a CSV file containing mock data for createable fields.
#[cfg(all(feature = "schema", feature = "data_utility"))]
#[must_use]
pub fn generate_mock_csv(describe: &SObjectDescribe, rows: usize) -> String {
    let mut fields: Vec<&_> = describe.fields.iter().filter(|f| f.createable).collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    let mut csv = String::with_capacity(1024 * (rows + 1));
    let mut first = true;
    for field in &fields {
        if !first {
            csv.push(',');
        }
        first = false;
        csv.push_str(&field.name);
    }
    let _ = writeln!(csv);

    for _ in 0..rows {
        let record = generate_mock_record(describe);
        first = true;
        for field in &fields {
            if !first {
                csv.push(',');
            }
            first = false;

            if let Some(val) = record.get_field(&field.name) {
                match val {
                    serde_json::Value::String(s) => {
                        // Escape quotes and wrap in quotes if necessary
                        if s.contains(',') || s.contains('"') || s.contains('\n') {
                            let _ = write!(csv, "\"{}\"", s.replace('"', "\"\""));
                        } else {
                            csv.push_str(s);
                        }
                    }
                    serde_json::Value::Number(n) => {
                        let _ = write!(csv, "{}", n);
                    }
                    serde_json::Value::Bool(b) => {
                        let _ = write!(csv, "{}", b);
                    }
                    serde_json::Value::Null => {}
                    _ => {
                        let _ = write!(csv, "{}", val);
                    }
                }
            }
        }
        let _ = writeln!(csv);
    }

    csv
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::types::describe::{FieldDescribe, FieldType};

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
            ],
        };

        let csv = generate_csv_template(&describe);
        // Should only include createable fields, sorted alphabetically
        assert_eq!(csv.trim(), "IsActive,Name,NumberOfEmployees");
    }

    #[test]
    #[cfg(feature = "data_utility")]
    fn test_generate_mock_csv() {
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
                mock_field("NumberOfEmployees", FieldType::Int, true),
            ],
        };

        let csv = generate_mock_csv(&describe, 2);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 3); // Header + 2 rows
        assert_eq!(lines[0], "Name,NumberOfEmployees");
        assert!(
            lines[1].contains("Mock Name Label")
                || lines[1].contains("mock_string")
                || lines[1].contains("Mock")
        );
        assert!(
            lines[2].contains("Mock Name Label")
                || lines[2].contains("mock_string")
                || lines[2].contains("Mock")
        );
    }
}
