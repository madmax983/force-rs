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
fn build_postman_item(
    label: &str,
    name: &str,
    operation: &str,
    method: &str,
    body: Option<&str>,
    requires_id: bool,
) -> Value {
    let mut url_path = vec![
        json!("services"),
        json!("data"),
        json!("v60.0"),
        json!("sobjects"),
        json!(name),
    ];

    let mut raw_url = format!("{{{{_endpoint}}}}/services/data/v60.0/sobjects/{}", name);

    if requires_id {
        url_path.push(json!("{{recordId}}"));
        raw_url.push_str("/{{recordId}}");
    }

    let mut request = serde_json::Map::new();
    request.insert("method".to_string(), json!(method));

    if let Some(body_str) = body {
        request.insert(
            "header".to_string(),
            json!([{
                "key": "Content-Type",
                "value": "application/json"
            }]),
        );
        request.insert(
            "body".to_string(),
            json!({
                "mode": "raw",
                "raw": body_str
            }),
        );
    } else {
        request.insert("header".to_string(), json!([]));
    }

    request.insert(
        "url".to_string(),
        json!({
            "raw": raw_url,
            "host": [
                "{{_endpoint}}"
            ],
            "path": url_path
        }),
    );

    json!({
        "name": format!("{} {}", operation, label),
        "request": request
    })
}

/// Generates a Postman v2.1.0 Collection for an SObject.
#[cfg(feature = "schema")]
#[must_use]
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
            build_postman_item(&label, name, "Create", "POST", Some(&create_body_str), false),
            build_postman_item(&label, name, "Read", "GET", None, true),
            build_postman_item(&label, name, "Update", "PATCH", Some(&update_body_str), true),
            build_postman_item(&label, name, "Delete", "DELETE", None, true),
        ]
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
