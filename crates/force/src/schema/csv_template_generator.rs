//! CSV Template Generator.
//!
//! This module provides a utility to generate CSV templates for Salesforce Bulk API operations
//! based on SObject Describe metadata.

#[cfg(all(feature = "schema", feature = "bulk"))]
use crate::schema::mock_data_generator::generate_mock_data;
#[cfg(all(feature = "schema", feature = "bulk"))]
use crate::types::describe::SObjectDescribe;

/// Generates a CSV template for a given SObject.
///
/// # Arguments
///
/// * `describe` - The SObject describe result.
/// * `include_mock_data` - Whether to include a row of mock data.
#[cfg(all(feature = "schema", feature = "bulk"))]
pub fn generate_csv_template(
    describe: &SObjectDescribe,
    include_mock_data: bool,
) -> Result<String, csv::Error> {
    let mut wtr = csv::Writer::from_writer(vec![]);

    let mut fields: Vec<&_> = describe.fields.iter().filter(|f| f.createable).collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    let headers: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
    wtr.write_record(&headers)?;

    if include_mock_data {
        let mock = generate_mock_data(describe);
        if let serde_json::Value::Object(map) = mock {
            let row: Vec<String> = fields
                .iter()
                .map(|f| match map.get(&f.name) {
                    Some(serde_json::Value::String(s)) => s.clone(),
                    Some(serde_json::Value::Number(n)) => n.to_string(),
                    Some(serde_json::Value::Bool(b)) => b.to_string(),
                    _ => String::new(),
                })
                .collect();
            wtr.write_record(&row)?;
        }
    }

    wtr.flush()?;
    Ok(String::from_utf8(wtr.into_inner().unwrap_or_default()).unwrap_or_default())
}

#[cfg(test)]
#[cfg(all(feature = "schema", feature = "bulk"))]
mod tests {
    use super::*;
    use crate::types::describe::{FieldDescribe, FieldType};

    fn default_field() -> FieldDescribe {
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
            createable: false,
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
            id_lookup: false,
            inline_help_text: None,
            label: "Default".to_string(),
            length: 0,
            mask: None,
            mask_type: None,
            name: "Default".to_string(),
            name_field: false,
            name_pointing: false,
            nillable: false,
            permissionable: true,
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
            search_prefixes_supported: Some(false),
            soap_type: "xsd:string".to_string(),
            sortable: true,
            type_: FieldType::String,
            unique: false,
            updateable: true,
            write_requires_master_read: false,
        }
    }

    fn mock_describe() -> SObjectDescribe {
        SObjectDescribe {
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
                FieldDescribe {
                    createable: false,
                    label: "Account ID".to_string(),
                    name: "Id".to_string(),
                    id_lookup: true,
                    type_: FieldType::Id,
                    ..default_field()
                },
                FieldDescribe {
                    createable: true,
                    label: "Account Name".to_string(),
                    name: "Name".to_string(),
                    name_field: true,
                    type_: FieldType::String,
                    ..default_field()
                },
                FieldDescribe {
                    createable: true,
                    label: "Active".to_string(),
                    name: "IsActive".to_string(),
                    type_: FieldType::Boolean,
                    ..default_field()
                },
            ],
        }
    }

    #[test]
    fn test_generate_csv_template_headers_only() -> anyhow::Result<()> {
        let describe = mock_describe();
        let csv = generate_csv_template(&describe, false)?;
        assert_eq!(csv, "IsActive,Name\n");
        Ok(())
    }

    #[test]
    fn test_generate_csv_template_with_mock_data() -> anyhow::Result<()> {
        let describe = mock_describe();
        let csv = generate_csv_template(&describe, true)?;
        assert_eq!(csv, "IsActive,Name\ntrue,mock_string\n");
        Ok(())
    }
}
