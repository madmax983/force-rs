//! JSON Schema Generator for Salesforce SObject Describe metadata.
//!
//! Generates JSON Schema draft-07 schemas based on Salesforce object schemas.

#[cfg(feature = "schema")]
use crate::types::describe::{FieldDescribe, FieldType, SObjectDescribe};
#[cfg(feature = "schema")]
use serde_json::{Value, json};

/// Generates a JSON Schema draft-07 definition for a given SObject.
#[cfg(feature = "schema")]
pub fn generate_json_schema(describe: &SObjectDescribe) -> Value {
    let mut properties = serde_json::Map::new();

    // Find required fields
    let required_fields: Vec<Value> = describe
        .fields
        .iter()
        .filter(|f| {
            !f.nillable
                && !f.defaulted_on_create
                && f.createable
                && f.type_ != FieldType::Id
                && f.type_ != FieldType::Boolean
        })
        .map(|f| Value::String(f.name.clone()))
        .collect();

    for field in &describe.fields {
        properties.insert(field.name.clone(), generate_field_schema(field));
    }

    let mut schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": describe.name,
        "type": "object",
        "properties": properties,
    });

    if let Some(obj) = schema.as_object_mut() {
        if !describe.label.is_empty() {
            obj.insert(
                "description".to_string(),
                Value::String(describe.label.clone()),
            );
        }

        if !required_fields.is_empty() {
            obj.insert("required".to_string(), Value::Array(required_fields));
        }
    }

    schema
}

fn generate_field_schema(field: &FieldDescribe) -> Value {
    let mut schema = serde_json::Map::new();

    if let Some(help) = &field.inline_help_text {
        schema.insert("description".to_string(), Value::String(help.clone()));
    } else {
        schema.insert(
            "description".to_string(),
            Value::String(field.label.clone()),
        );
    }

    if !field.updateable && !field.createable {
        schema.insert("readOnly".to_string(), Value::Bool(true));
    }

    match field.type_ {
        FieldType::String
        | FieldType::Email
        | FieldType::Url
        | FieldType::Phone
        | FieldType::Id
        | FieldType::Reference
        | FieldType::Combobox => {
            schema.insert("type".to_string(), Value::String("string".to_string()));
            if field.length > 0 {
                schema.insert(
                    "maxLength".to_string(),
                    Value::Number(serde_json::Number::from(field.length)),
                );
            }
        }

        FieldType::Picklist | FieldType::Multipicklist => {
            schema.insert("type".to_string(), Value::String("string".to_string()));
            if let Some(values) = &field.picklist_values {
                if !values.is_empty() {
                    let enum_values: Vec<Value> = values
                        .iter()
                        .map(|pv| Value::String(pv.value.clone()))
                        .collect();
                    schema.insert("enum".to_string(), Value::Array(enum_values));
                }
            }
        }
        FieldType::Boolean => {
            schema.insert("type".to_string(), Value::String("boolean".to_string()));
        }
        FieldType::Int => {
            schema.insert("type".to_string(), Value::String("integer".to_string()));
        }
        FieldType::Double | FieldType::Percent | FieldType::Currency => {
            schema.insert("type".to_string(), Value::String("number".to_string()));
        }
        FieldType::Date => {
            schema.insert("type".to_string(), Value::String("string".to_string()));
            schema.insert("format".to_string(), Value::String("date".to_string()));
        }
        FieldType::Datetime => {
            schema.insert("type".to_string(), Value::String("string".to_string()));
            schema.insert("format".to_string(), Value::String("date-time".to_string()));
        }
        FieldType::Base64 => {
            schema.insert("type".to_string(), Value::String("string".to_string()));
            schema.insert(
                "contentEncoding".to_string(),
                Value::String("base64".to_string()),
            );
        }
        _ => {
            schema.insert("type".to_string(), Value::String("string".to_string()));
        }
    }

    Value::Object(schema)
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_support::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};

    #[test]
    fn test_json_schema_generator_basic() {
        let describe = MockSObjectDescribeBuilder::new("Account")
            .field(
                MockFieldDescribeBuilder::new("Id", FieldType::Id)
                    .label("Account ID")
                    .length(18)
                    .byte_length(18)
                    .nillable(false)
                    .createable(false)
                    .updateable(false)
                    .permissionable(false)
                    .defaulted_on_create(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("Name", FieldType::String)
                    .label("Account Name")
                    .length(255)
                    .byte_length(765)
                    .nillable(false)
                    .createable(true)
                    .updateable(true)
                    .permissionable(true)
                    .build(),
            )
            .build();

        let schema = generate_json_schema(&describe);

        assert_eq!(schema["$schema"], "http://json-schema.org/draft-07/schema#");
        assert_eq!(schema["title"], "Account");
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["description"], "Account");
        assert_eq!(schema["required"][0], "Name");

        let props = &schema["properties"];
        assert_eq!(props["Id"]["type"], "string");
        assert_eq!(props["Id"]["maxLength"], 18);
        assert_eq!(props["Id"]["readOnly"], true);

        assert_eq!(props["Name"]["type"], "string");
        assert_eq!(props["Name"]["maxLength"], 255);
    }
}
