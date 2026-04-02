use crate::api::rest::describe::{FieldType, SObjectDescribe};
use serde_json::{Map, Value};

/// Generates mock data for an SObjectDescribe based on its createable fields.
#[cfg(feature = "schema")]
pub fn generate_mock_data(describe: &SObjectDescribe) -> Value {
    let mut map = Map::new();

    for field in &describe.fields {
        if !field.createable {
            continue;
        }

        let mock_val = match field.type_ {
            FieldType::String | FieldType::Textarea | FieldType::Email | FieldType::Phone | FieldType::Url => Value::String("mock_string".to_string()),
            FieldType::Boolean => Value::Bool(true),
            FieldType::Int => Value::Number(serde_json::Number::from(42)),
            FieldType::Double | FieldType::Currency | FieldType::Percent => {
                Value::Number(serde_json::Number::from_f64(42.0).unwrap())
            }
            FieldType::Date => Value::String("2023-01-01".to_string()),
            FieldType::Datetime => Value::String("2023-01-01T00:00:00Z".to_string()),
            FieldType::Id | FieldType::Reference => Value::String("001000000000000AAA".to_string()),
            _ => Value::Null,
        };

        map.insert(field.name.clone(), mock_val);
    }

    Value::Object(map)
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::api::rest::describe::{FieldType, SObjectDescribe, FieldDescribe};

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
    fn test_generate_mock_data() {
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

        let result = generate_mock_data(&describe);

        // Assert that the result is an object
        let obj = result.as_object().expect("Expected JSON Object");

        // Assert that Id is not present (since it's not createable)
        assert!(!obj.contains_key("Id"));

        // Assert that createable fields are present with correct mock values
        assert_eq!(obj.get("Name").unwrap().as_str().unwrap(), "mock_string");
        assert_eq!(obj.get("IsActive").unwrap().as_bool().unwrap(), true);
        assert_eq!(obj.get("NumberOfEmployees").unwrap().as_i64().unwrap(), 42);
    }
}
