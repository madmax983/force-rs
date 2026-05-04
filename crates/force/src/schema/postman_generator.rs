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

/// Generates a Postman Collection mapping CRUD operations for an SObject.
///
/// Builds a Postman Collection v2.1.0 JSON object containing standard
/// POST (Create), GET (Read), PATCH (Update), and DELETE operations.
/// It automatically populates request bodies for Create and Update based
/// on the specific field-level `createable` and `updateable` flags of the SObject.
///
/// # Arguments
///
/// * `describe` - The SObject description metadata.
///
/// # Returns
///
/// A `serde_json::Value` representing the generated Postman collection.
#[must_use]
#[cfg(feature = "schema")]
pub fn generate_postman_collection(describe: &SObjectDescribe) -> Value {
    let name = &describe.name;
    let label = if describe.label.is_empty() {
        name.clone()
    } else {
        describe.label.clone()
    };

    let create_body_str = build_request_body(describe, true);
    let update_body_str = build_request_body(describe, false);

    json!({
        "info": {
            "name": format!("Salesforce REST API - {}", label),
            "description": format!("Generated CRUD operations for {}", name),
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
        },
        "item": [
            build_create_request(&label, name, &create_body_str),
            build_read_request(&label, name),
            build_update_request(&label, name, &update_body_str),
            build_delete_request(&label, name)
        ]
    })
}

#[cfg(feature = "schema")]
fn build_request_body(describe: &SObjectDescribe, is_create: bool) -> String {
    let mut body = serde_json::Map::new();

    for field in &describe.fields {
        let include_field = if is_create {
            field.createable
        } else {
            field.updateable
        };
        if include_field && field.name != "Id" {
            body.insert(
                field.name.clone(),
                json!(format!("{{{{${}}}}}", field.name)),
            );
        }
    }

    serde_json::to_string_pretty(&body).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(feature = "schema")]
fn build_create_request(label: &str, name: &str, body_str: &str) -> Value {
    json!({
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
                "raw": body_str
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
    })
}

#[cfg(feature = "schema")]
fn build_read_request(label: &str, name: &str) -> Value {
    json!({
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
    })
}

#[cfg(feature = "schema")]
fn build_update_request(label: &str, name: &str, body_str: &str) -> Value {
    json!({
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
                "raw": body_str
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
    })
}

#[cfg(feature = "schema")]
fn build_delete_request(label: &str, name: &str) -> Value {
    json!({
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
    })
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_support::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder, MustMsg};
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
