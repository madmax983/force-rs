#[cfg(feature = "schema")]
use crate::types::describe::SObjectDescribe;
use std::fmt::Write;

/// Generates a dbt staging model SQL query for an SObject.
#[cfg(feature = "schema")]
pub fn generate_dbt_staging_model(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(1024);
    let table_name = describe.name.to_lowercase();

    let _ = writeln!(out, "with source as (");
    let _ = writeln!(
        out,
        "    select * from {{{{ source('salesforce', '{}') }}}}",
        table_name
    );
    let _ = writeln!(out, "),");
    let _ = writeln!(out, "renamed as (");
    let _ = writeln!(out, "    select");

    for (i, field) in describe.fields.iter().enumerate() {
        let field_name_lower = field.name.to_lowercase();
        let comma = if i < describe.fields.len() - 1 {
            ","
        } else {
            ""
        };
        let mut snake_case_name = String::with_capacity(field.name.len() + 5);
        let chars = field.name.chars();
        for c in chars {
            if c.is_uppercase() {
                if !snake_case_name.is_empty() {
                    snake_case_name.push('_');
                }
                snake_case_name.push(c.to_ascii_lowercase());
            } else {
                snake_case_name.push(c);
            }
        }

        if field_name_lower == snake_case_name {
            let _ = writeln!(out, "        {}{} ", field_name_lower, comma);
        } else {
            let _ = writeln!(
                out,
                "        {} as {}{} ",
                field_name_lower, snake_case_name, comma
            );
        }
    }

    let _ = writeln!(out, "    from source");
    let _ = writeln!(out, ")");
    let _ = writeln!(out, "select * from renamed");

    out
}

/// Generates a dbt source.yml definition for an SObject.
#[cfg(feature = "schema")]
pub fn generate_dbt_source_yml(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(1024);
    let table_name = describe.name.to_lowercase();

    let _ = writeln!(out, "version: 2");
    let _ = writeln!(out);
    let _ = writeln!(out, "sources:");
    let _ = writeln!(out, "  - name: salesforce");
    let _ = writeln!(out, "    tables:");
    let _ = writeln!(out, "      - name: {}", table_name);
    let _ = writeln!(out, "        columns:");

    for field in &describe.fields {
        let field_name_lower = field.name.to_lowercase();
        let _ = writeln!(out, "          - name: {}", field_name_lower);
        if let Some(label) = &field.inline_help_text {
            let _ = writeln!(
                out,
                "            description: \"{}\"",
                label.replace('"', "")
            );
        } else {
            let _ = writeln!(
                out,
                "            description: \"{}\"",
                field.label.replace('"', "")
            );
        }
    }

    out
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::types::describe::{FieldDescribe, FieldType, SObjectDescribe};

    fn mock_field(name: &str, type_: FieldType) -> FieldDescribe {
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

    fn create_mock_describe() -> SObjectDescribe {
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
                mock_field("Id", FieldType::Id),
                mock_field("Name", FieldType::String),
                mock_field("CreatedDate", FieldType::Datetime),
            ],
        }
    }

    #[test]
    fn test_generate_dbt_staging_model() {
        let describe = create_mock_describe();
        let sql = generate_dbt_staging_model(&describe);

        assert!(sql.contains("with source as ("));
        assert!(sql.contains("select * from {{ source('salesforce', 'account') }}"));
        assert!(sql.contains("renamed as ("));
        assert!(sql.contains("id,"));
        assert!(sql.contains("name,"));
        assert!(sql.contains("created_date"));
        assert!(sql.contains("from source"));
    }

    #[test]
    fn test_generate_dbt_source_yml() {
        let describe = create_mock_describe();
        let yml = generate_dbt_source_yml(&describe);

        assert!(yml.contains("version: 2"));
        assert!(yml.contains("sources:"));
        assert!(yml.contains("- name: salesforce"));
        assert!(yml.contains("tables:"));
        assert!(yml.contains("- name: account"));
    }
}
