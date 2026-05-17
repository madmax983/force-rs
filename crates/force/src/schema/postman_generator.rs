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
    // ⚡ Bolt: Use a string slice (`&str`) for the label instead of an owned `String` to avoid
    // an unnecessary `.clone()` heap allocation, since it's only used for formatting later.
    let label = if describe.label.is_empty() {
        name.as_str()
    } else {
        describe.label.as_str()
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
