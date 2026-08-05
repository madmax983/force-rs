#[cfg(feature = "schema")]
use crate::types::describe::SObjectDescribe;
use std::fmt::Write;

/// Generates a SOQL query that selects all fields on an SObject.
#[cfg(feature = "schema")]
#[must_use]
pub fn generate_soql_select_all(describe: &SObjectDescribe) -> String {
    generate_soql_with_filter(describe, |_| true)
}

/// Generates a SOQL query that selects only createable fields on an SObject.
#[cfg(feature = "schema")]
#[must_use]
pub fn generate_soql_select_createable(describe: &SObjectDescribe) -> String {
    generate_soql_with_filter(describe, |f| f.createable)
}

/// Generates a SOQL query that selects only updateable fields on an SObject.
#[cfg(feature = "schema")]
#[must_use]
pub fn generate_soql_select_updateable(describe: &SObjectDescribe) -> String {
    generate_soql_with_filter(describe, |f| f.updateable)
}

#[cfg(feature = "schema")]
fn generate_soql_with_filter<F>(describe: &SObjectDescribe, filter: F) -> String
where
    F: Fn(&crate::types::describe::FieldDescribe) -> bool,
{
    let mut soql = String::with_capacity(1024);
    soql.push_str("SELECT ");

    let mut fields: Vec<&_> = describe.fields.iter().filter(|f| filter(f)).collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    let mut first = true;
    for field in fields {
        if !first {
            soql.push_str(", ");
        }
        soql.push_str(&field.name);
        first = false;
    }

    let _ = write!(&mut soql, " FROM {}", describe.name);
    soql
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::types::describe::{FieldDescribe, FieldType};

    fn mock_field(name: &str, createable: bool, updateable: bool) -> FieldDescribe {
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
            type_: FieldType::String,
            unique: false,
            updateable,
            write_requires_master_read: false,
        }
    }

    #[test]
    fn test_generate_soql_select_all() {
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
                mock_field("SystemModstamp", false, false),
                mock_field("Name", true, true),
                mock_field("Id", false, false),
            ],
        };

        assert_eq!(
            generate_soql_select_all(&describe),
            "SELECT Id, Name, SystemModstamp FROM Account"
        );
    }

    #[test]
    fn test_generate_soql_select_createable() {
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
                mock_field("SystemModstamp", false, false),
                mock_field("Name", true, true),
                mock_field("Id", false, false),
            ],
        };

        assert_eq!(
            generate_soql_select_createable(&describe),
            "SELECT Name FROM Account"
        );
    }

    #[test]
    fn test_generate_soql_select_updateable() {
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
                mock_field("Id", false, false),
                mock_field("Name", true, true),
                mock_field("CreatedDate", false, false),
            ],
        };

        assert_eq!(
            generate_soql_select_updateable(&describe),
            "SELECT Name FROM Account"
        );
    }
}
