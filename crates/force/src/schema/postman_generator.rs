//! Postman Collection Generator.
//!
//! This module provides a utility to generate a Postman v2.1.0 Collection
//! from a Salesforce SObjectDescribe metadata payload. It creates standard
//! CRUD (Create, Read, Update, Delete) operations with pre-filled variables
//! based on the object's schema.

#[cfg(feature = "schema")]
use crate::api::rest::describe::SObjectDescribe;
#[cfg(feature = "schema")]
use serde_json::{Value, json};

/// Generates a Postman v2.1.0 Collection for an SObject.
#[cfg(feature = "schema")]
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn generate_postman_collection(describe: &SObjectDescribe) -> Value {
    let name = &describe.name;
    let label = if describe.label.is_empty() {
        name.clone()
    } else {
        describe.label.clone()
    };

    // Build dummy json body for create/update using fields that are createable/updateable
    let mut create_body = serde_json::Map::new();
    let mut update_body = serde_json::Map::new();

    for field in &describe.fields {
        if field.createable && field.name != "Id" {
            create_body.insert(
                field.name.clone(),
                json!(format!("{{{{${}}}}}", field.name)),
            );
        }
        if field.updateable && field.name != "Id" {
            update_body.insert(
                field.name.clone(),
                json!(format!("{{{{${}}}}}", field.name)),
            );
        }
    }

    let create_body_str =
        serde_json::to_string_pretty(&create_body).unwrap_or_else(|_| "{}".to_string());
    let update_body_str =
        serde_json::to_string_pretty(&update_body).unwrap_or_else(|_| "{}".to_string());

    json!({
        "info": {
            "name": format!("Salesforce REST API - {}", label),
            "description": format!("Generated CRUD operations for {}", name),
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
        },
        "item": [
            {
                "name": format!("Create {}", label),
                "request": {
                    "method": "POST",
                    "header": [
                        {
                            "key": "Content-Type",
                            "value": "application/json"
                        }
                    ],
                    "body": {
                        "mode": "raw",
                        "raw": create_body_str
                    },
                    "url": {
                        "raw": format!("{{{{_endpoint}}}}/services/data/v60.0/sobjects/{}", name),
                        "host": [
                            "{{_endpoint}}"
                        ],
                        "path": [
                            "services",
                            "data",
                            "v60.0",
                            "sobjects",
                            name
                        ]
                    }
                }
            },
            {
                "name": format!("Read {}", label),
                "request": {
                    "method": "GET",
                    "header": [],
                    "url": {
                        "raw": format!("{{{{_endpoint}}}}/services/data/v60.0/sobjects/{}/{{{{recordId}}}}", name),
                        "host": [
                            "{{_endpoint}}"
                        ],
                        "path": [
                            "services",
                            "data",
                            "v60.0",
                            "sobjects",
                            name,
                            "{{recordId}}"
                        ]
                    }
                }
            },
            {
                "name": format!("Update {}", label),
                "request": {
                    "method": "PATCH",
                    "header": [
                        {
                            "key": "Content-Type",
                            "value": "application/json"
                        }
                    ],
                    "body": {
                        "mode": "raw",
                        "raw": update_body_str
                    },
                    "url": {
                        "raw": format!("{{{{_endpoint}}}}/services/data/v60.0/sobjects/{}/{{{{recordId}}}}", name),
                        "host": [
                            "{{_endpoint}}"
                        ],
                        "path": [
                            "services",
                            "data",
                            "v60.0",
                            "sobjects",
                            name,
                            "{{recordId}}"
                        ]
                    }
                }
            },
            {
                "name": format!("Delete {}", label),
                "request": {
                    "method": "DELETE",
                    "header": [],
                    "url": {
                        "raw": format!("{{{{_endpoint}}}}/services/data/v60.0/sobjects/{}/{{{{recordId}}}}", name),
                        "host": [
                            "{{_endpoint}}"
                        ],
                        "path": [
                            "services",
                            "data",
                            "v60.0",
                            "sobjects",
                            name,
                            "{{recordId}}"
                        ]
                    }
                }
            }
        ]
    })
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::api::rest::describe::{FieldDescribe, FieldType};

    fn mock_field(
        name: &str,
        type_: FieldType,
        createable: bool,
        updateable: bool,
    ) -> FieldDescribe {
        FieldDescribe {
            aggregatable: true,
            auto_number: false,
            byte_length: 255,
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
            length: 255,
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
            updateable,
            write_requires_master_read: false,
        }
    }

    #[test]
    fn test_generate_postman_collection() {
        use crate::test_support::MustMsg;
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
                mock_field("Id", FieldType::Id, true, true),
                mock_field("Name", FieldType::String, true, true),
                mock_field("Industry", FieldType::Picklist, true, true),
                mock_field("CreateOnly", FieldType::String, true, false),
                mock_field("UpdateOnly", FieldType::String, false, true),
            ],
        };

        let collection = generate_postman_collection(&describe);

        assert_eq!(collection["info"]["name"], "Salesforce REST API - Account");
        assert_eq!(
            collection["info"]["schema"],
            "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
        );

        let items = collection["item"]
            .as_array()
            .must_msg("item should be an array");
        assert_eq!(items.len(), 4, "Should have 4 CRUD operations");

        // Check Create Request
        let create_item = &items[0];
        assert_eq!(create_item["name"], "Create Account");
        assert_eq!(create_item["request"]["method"], "POST");
        let create_raw = create_item["request"]["body"]["raw"]
            .as_str()
            .must_msg("raw body should be string");
        assert!(
            !create_raw.contains("\"Id\""),
            "Id should not be createable"
        );
        assert!(create_raw.contains("\"Name\""), "Name should be createable");
        assert!(
            create_raw.contains("\"CreateOnly\""),
            "CreateOnly should be createable"
        );
        assert!(
            !create_raw.contains("\"UpdateOnly\""),
            "UpdateOnly should not be createable"
        );

        // Check Update Request
        let update_item = &items[2];
        assert_eq!(update_item["name"], "Update Account");
        assert_eq!(update_item["request"]["method"], "PATCH");
        let update_raw = update_item["request"]["body"]["raw"]
            .as_str()
            .must_msg("raw body should be string");
        assert!(
            !update_raw.contains("\"Id\""),
            "Id should not be updateable"
        );
        assert!(update_raw.contains("\"Name\""), "Name should be updateable");
        assert!(
            !update_raw.contains("\"CreateOnly\""),
            "CreateOnly should not be updateable"
        );
        assert!(
            update_raw.contains("\"UpdateOnly\""),
            "UpdateOnly should be updateable"
        );
    }
}
