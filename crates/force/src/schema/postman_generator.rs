//! Postman Collection Generator.
//!
//! This module provides a utility to generate a Postman v2.1.0 Collection
//! from a Salesforce SObjectDescribe metadata payload. It creates standard
//! CRUD (Create, Read, Update, Delete) operations with pre-filled variables
//! based on the object's schema.

#[cfg(feature = "schema")]
use crate::types::describe::SObjectDescribe;
#[cfg(feature = "schema")]
use serde_json::{Value, json};

/// Generates a Postman v2.1.0 Collection for an SObject.
#[cfg(feature = "schema")]
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn generate_postman_collection(describe: &SObjectDescribe) -> Value {
    let name = &describe.name;
    let label = if describe.label.is_empty() {
        name
    } else {
        &describe.label
    };

    // Build dummy json body for create/update using fields that are createable/updateable
    let mut create_body = serde_json::Map::new();
    let mut update_body = serde_json::Map::new();

    for field in &describe.fields {
        if field.createable && field.name != "Id" {
            // ⚡ Bolt: Eliminate `format!` heap allocations by pre-allocating String with capacity and using push_str
            let mut val = String::with_capacity(5 + field.name.len());
            val.push_str("{{$");
            val.push_str(&field.name);
            val.push_str("}}");
            create_body.insert(field.name.clone(), json!(val));
        }
        if field.updateable && field.name != "Id" {
            // ⚡ Bolt: Eliminate `format!` heap allocations by pre-allocating String with capacity and using push_str
            let mut val = String::with_capacity(5 + field.name.len());
            val.push_str("{{$");
            val.push_str(&field.name);
            val.push_str("}}");
            update_body.insert(field.name.clone(), json!(val));
        }
    }

    let create_body_str =
        serde_json::to_string_pretty(&create_body).unwrap_or_else(|_| "{}".to_string());
    let update_body_str =
        serde_json::to_string_pretty(&update_body).unwrap_or_else(|_| "{}".to_string());

    // ⚡ Bolt: Eliminate `format!` heap allocations for large Postman templates by pre-allocating Strings
    let mut info_name = String::with_capacity(22 + label.len());
    info_name.push_str("Salesforce REST API - ");
    info_name.push_str(label);

    let mut info_desc = String::with_capacity(31 + name.len());
    info_desc.push_str("Generated CRUD operations for ");
    info_desc.push_str(name);

    let mut create_name = String::with_capacity(7 + label.len());
    create_name.push_str("Create ");
    create_name.push_str(label);

    let mut read_name = String::with_capacity(5 + label.len());
    read_name.push_str("Read ");
    read_name.push_str(label);

    let mut update_name = String::with_capacity(7 + label.len());
    update_name.push_str("Update ");
    update_name.push_str(label);

    let mut delete_name = String::with_capacity(7 + label.len());
    delete_name.push_str("Delete ");
    delete_name.push_str(label);

    let mut url_collection = String::with_capacity(40 + name.len());
    url_collection.push_str("{{_endpoint}}/services/data/v60.0/sobjects/");
    url_collection.push_str(name);

    let mut url_record = String::with_capacity(53 + name.len());
    url_record.push_str("{{_endpoint}}/services/data/v60.0/sobjects/");
    url_record.push_str(name);
    url_record.push_str("/{{recordId}}");

    json!({
        "info": {
            "name": info_name,
            "description": info_desc,
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
        },
        "item": [
            {
                "name": create_name,
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
                        "raw": url_collection,
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
                "name": read_name,
                "request": {
                    "method": "GET",
                    "header": [],
                    "url": {
                        "raw": url_record.clone(),
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
                "name": update_name,
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
                        "raw": url_record.clone(),
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
                "name": delete_name,
                "request": {
                    "method": "DELETE",
                    "header": [],
                    "url": {
                        "raw": url_record,
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
    use crate::test_utils::mock_describe::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};
    use crate::test_utils::must::MustMsg;
    use crate::types::describe::FieldType;

    #[test]
    fn test_generate_postman_collection() {
        let describe = MockSObjectDescribeBuilder::new("Account")
            .field(
                MockFieldDescribeBuilder::new("Id", FieldType::Id)
                    .createable(true)
                    .updateable(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("Name", FieldType::String)
                    .createable(true)
                    .updateable(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("Industry", FieldType::Picklist)
                    .createable(true)
                    .updateable(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("CreateOnly", FieldType::String)
                    .createable(true)
                    .updateable(false)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("UpdateOnly", FieldType::String)
                    .createable(false)
                    .updateable(true)
                    .build(),
            )
            .build();

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
