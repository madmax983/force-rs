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
fn generate_dummy_body(describe: &SObjectDescribe, is_update: bool) -> String {
    let mut body = serde_json::Map::new();
    for field in &describe.fields {
        if field.name == "Id" {
            continue;
        }
        let should_include = if is_update {
            field.updateable
        } else {
            field.createable
        };
        if should_include {
            body.insert(
                field.name.clone(),
                json!(format!("{{{{${}}}}}", field.name)),
            );
        }
    }
    serde_json::to_string_pretty(&body).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(feature = "schema")]
fn create_url_object(name: &str, with_id: bool) -> Value {
    let raw = if with_id {
        format!(
            "{{{{_endpoint}}}}/services/data/v60.0/sobjects/{}/{{{{recordId}}}}",
            name
        )
    } else {
        format!("{{{{_endpoint}}}}/services/data/v60.0/sobjects/{}", name)
    };

    let mut path = vec![
        json!("services"),
        json!("data"),
        json!("v60.0"),
        json!("sobjects"),
        json!(name),
    ];
    if with_id {
        path.push(json!("{{recordId}}"));
    }

    json!({
        "raw": raw,
        "host": [
            "{{_endpoint}}"
        ],
        "path": path
    })
}

#[cfg(feature = "schema")]
fn create_request_item(
    name: &str,
    method: &str,
    label: &str,
    body_str: Option<&str>,
    with_id: bool,
) -> Value {
    let mut request = json!({
        "method": method,
        "header": [],
        "url": create_url_object(name, with_id)
    });

    if let Some(body) = body_str {
        request["header"] = json!([
            {
                "key": "Content-Type",
                "value": "application/json"
            }
        ]);
        request["body"] = json!({
            "mode": "raw",
            "raw": body
        });
    }

    json!({
        "name": format!("{} {}", method_to_action(method), label),
        "request": request
    })
}

#[cfg(feature = "schema")]
fn method_to_action(method: &str) -> &'static str {
    match method {
        "POST" => "Create",
        "GET" => "Read",
        "PATCH" => "Update",
        "DELETE" => "Delete",
        _ => "Unknown",
    }
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

    let create_body_str = generate_dummy_body(describe, false);
    let update_body_str = generate_dummy_body(describe, true);

    json!({
        "info": {
            "name": format!("Salesforce REST API - {}", label),
            "description": format!("Generated CRUD operations for {}", name),
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
        },
        "item": [
            create_request_item(name, "POST", &label, Some(&create_body_str), false),
            create_request_item(name, "GET", &label, None, true),
            create_request_item(name, "PATCH", &label, Some(&update_body_str), true),
            create_request_item(name, "DELETE", &label, None, true),
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
