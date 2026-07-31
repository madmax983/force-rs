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

#[cfg(feature = "schema")]
fn build_url(name: &str, with_id: bool) -> Value {
    let raw_url = if with_id {
        format!(
            "{{{{_endpoint}}}}/services/data/v67.0/sobjects/{}/{{{{recordId}}}}",
            name
        )
    } else {
        format!("{{{{_endpoint}}}}/services/data/v67.0/sobjects/{}", name)
    };
    let mut path = vec![
        json!("services"),
        json!("data"),
        json!("v67.0"),
        json!("sobjects"),
        json!(name),
    ];
    if with_id {
        path.push(json!("{{recordId}}"));
    }
    json!({
        "raw": raw_url,
        "host": [
            "{{_endpoint}}"
        ],
        "path": path
    })
}

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
                    "url": build_url(name, false)
                }
            },
            {
                "name": format!("Read {}", label),
                "request": {
                    "method": "GET",
                    "header": [],
                    "url": build_url(name, true)
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
                    "url": build_url(name, true)
                }
            },
            {
                "name": format!("Delete {}", label),
                "request": {
                    "method": "DELETE",
                    "header": [],
                    "url": build_url(name, true)
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

    #[test]
    fn test_build_url() {
        let url = super::build_url("Account", false);
        assert_eq!(
            url["raw"],
            "{{_endpoint}}/services/data/v67.0/sobjects/Account"
        );
        let path = url["path"].as_array().must_msg("path is array");
        assert_eq!(path.len(), 5);
        assert_eq!(path[4], "Account");

        let url_with_id = super::build_url("Account", true);
        assert_eq!(
            url_with_id["raw"],
            "{{_endpoint}}/services/data/v67.0/sobjects/Account/{{recordId}}"
        );
        let path = url_with_id["path"].as_array().must_msg("path is array");
        assert_eq!(path.len(), 6);
        assert_eq!(path[5], "{{recordId}}");
    }
}
