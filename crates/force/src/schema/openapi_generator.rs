//! OpenAPI Generator for Salesforce SObject Describe metadata.
//!
//! Generates OpenAPI 3.0 schemas and path operations based on Salesforce object schemas.

#[cfg(feature = "schema")]
use crate::types::describe::{FieldDescribe, FieldType, SObjectDescribe};
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
    // ⚡ Bolt: Avoid intermediate `Vec<&str>` allocation by using a two-pass approach.
    let is_required = |f: &FieldDescribe| -> bool {
        !f.nillable
            && !f.defaulted_on_create
            && f.createable
            && f.type_ != FieldType::Id
            && f.type_ != FieldType::Boolean
    };

    let has_required = describe.fields.iter().any(is_required);

    if has_required {
        out.push_str("      required:\n");
        for field in describe.fields.iter().filter(|&f| is_required(f)) {
            let _ = writeln!(out, "        - {}", field.name);
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
    use crate::test_utils::mock_describe::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};

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

    #[test]
    fn test_openapi_generator_all_types() {
        let describe = MockSObjectDescribeBuilder::new("AllTypes")
            .field(MockFieldDescribeBuilder::new("BoolField", FieldType::Boolean).build())
            .field(MockFieldDescribeBuilder::new("IntField", FieldType::Int).build())
            .field(MockFieldDescribeBuilder::new("DoubleField", FieldType::Double).build())
            .field(MockFieldDescribeBuilder::new("PercentField", FieldType::Percent).build())
            .field(MockFieldDescribeBuilder::new("CurrencyField", FieldType::Currency).build())
            .field(MockFieldDescribeBuilder::new("DateField", FieldType::Date).build())
            .field(MockFieldDescribeBuilder::new("DatetimeField", FieldType::Datetime).build())
            .field(MockFieldDescribeBuilder::new("Base64Field", FieldType::Base64).build())
            .field(MockFieldDescribeBuilder::new("TextareaField", FieldType::Textarea).build())
            .field(
                MockFieldDescribeBuilder::new("PicklistField", FieldType::Picklist)
                    .picklist_values(vec![
                        crate::types::describe::PicklistValue {
                            active: true,
                            default_value: false,
                            label: "A".to_string(),
                            valid_for: None,
                            value: "A".to_string(),
                        },
                        crate::types::describe::PicklistValue {
                            active: true,
                            default_value: false,
                            label: "B".to_string(),
                            valid_for: None,
                            value: "B".to_string(),
                        },
                    ])
                    .build(),
            )
            .field(MockFieldDescribeBuilder::new("UnknownField", FieldType::AnyType).build())
            .build();

        let schema = generate_openapi_schema(&describe);

        assert!(schema.contains("BoolField:"));
        assert!(schema.contains("type: boolean"));

        assert!(schema.contains("IntField:"));
        assert!(schema.contains("type: integer"));

        assert!(schema.contains("DoubleField:"));
        assert!(schema.contains("type: number"));

        assert!(schema.contains("PercentField:"));

        assert!(schema.contains("CurrencyField:"));

        assert!(schema.contains("DateField:"));
        assert!(schema.contains(
            "format: date
"
        ));

        assert!(schema.contains("DatetimeField:"));
        assert!(schema.contains(
            "format: date-time
"
        ));

        assert!(schema.contains("Base64Field:"));
        assert!(schema.contains(
            "format: byte
"
        ));

        assert!(schema.contains("TextareaField:"));
        assert!(schema.contains("type: string"));

        assert!(schema.contains("PicklistField:"));
        assert!(schema.contains("enum:"));
        assert!(schema.contains("- A"));
        assert!(schema.contains("- B"));

        assert!(schema.contains("UnknownField:"));
    }
}
