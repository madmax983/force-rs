//! OpenAPI Generator for Salesforce SObject Describe metadata.
//!
//! Generates OpenAPI 3.0 schemas and path operations based on Salesforce object schemas.

#[cfg(feature = "schema")]
use crate::api::rest::describe::{FieldDescribe, FieldType, SObjectDescribe};
#[cfg(feature = "schema")]
use std::fmt::Write;

/// Generates an OpenAPI 3.0 component schema definition for a given SObject.
#[cfg(feature = "schema")]
pub fn generate_openapi_schema(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_openapi_schema(&mut out, describe);
    out
}

/// Writes an OpenAPI 3.0 component schema definition for a given SObject directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_openapi_schema(out: &mut String, describe: &SObjectDescribe) {
    let _ = writeln!(out, "    {}:", describe.name);
    out.push_str("      type: object\n");
    if !describe.label.is_empty() {
        let _ = writeln!(out, "      description: {}", describe.label);
    }

    // Find required fields for the required array
    let required_fields: Vec<&str> = describe
        .fields
        .iter()
        .filter(|f| {
            !f.nillable
                && !f.defaulted_on_create
                && f.createable
                && f.type_ != FieldType::Id
                && f.type_ != FieldType::Boolean
        })
        .map(|f| f.name.as_str())
        .collect();

    if !required_fields.is_empty() {
        out.push_str("      required:\n");
        for field in required_fields {
            let _ = writeln!(out, "        - {}", field);
        }
    }

    out.push_str("      properties:\n");

    for field in &describe.fields {
        let _ = writeln!(out, "        {}:", field.name);
        write_field_schema(out, field);
    }
}
fn write_field_schema(out: &mut String, field: &FieldDescribe) {
    if let Some(help) = &field.inline_help_text {
        let _ = writeln!(out, "          description: {}", help);
    } else {
        let _ = writeln!(out, "          description: {}", field.label);
    }

    if !field.updateable && !field.createable {
        out.push_str("          readOnly: true\n");
    }

    match field.type_ {
        FieldType::String
        | FieldType::Email
        | FieldType::Url
        | FieldType::Phone
        | FieldType::Id
        | FieldType::Reference
        | FieldType::Combobox => {
            out.push_str("          type: string\n");
            if field.length > 0 {
                let _ = writeln!(out, "          maxLength: {}", field.length);
            }
        }
        FieldType::Textarea => {
            out.push_str("          type: string\n");
        }
        FieldType::Picklist | FieldType::Multipicklist => {
            out.push_str("          type: string\n");
            if let Some(values) = &field.picklist_values {
                if !values.is_empty() {
                    out.push_str("          enum:\n");
                    for pv in values {
                        let _ = writeln!(out, "            - {}", pv.value);
                    }
                }
            }
        }
        FieldType::Boolean => {
            out.push_str("          type: boolean\n");
        }
        FieldType::Int => {
            out.push_str("          type: integer\n");
        }
        FieldType::Double | FieldType::Percent | FieldType::Currency => {
            out.push_str("          type: number\n");
        }
        FieldType::Date => {
            out.push_str("          type: string\n");
            out.push_str("          format: date\n");
        }
        FieldType::Datetime => {
            out.push_str("          type: string\n");
            out.push_str("          format: date-time\n");
        }
        FieldType::Base64 => {
            out.push_str("          type: string\n");
            out.push_str("          format: byte\n");
        }
        _ => {
            // Fallback for any other type
            out.push_str("          type: string\n");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};

    #[test]
    fn test_openapi_generator_basic() {
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

        let schema = generate_openapi_schema(&describe);

        assert!(schema.contains("Account:"));
        assert!(schema.contains("type: object"));
        assert!(schema.contains("description: Account"));
        assert!(schema.contains("required:"));
        assert!(schema.contains("- Name"));
        assert!(!schema.contains("- Id")); // Id shouldn't be required
        assert!(schema.contains("Id:"));
        assert!(schema.contains("readOnly: true"));
        assert!(schema.contains("maxLength: 18"));
    }
}
