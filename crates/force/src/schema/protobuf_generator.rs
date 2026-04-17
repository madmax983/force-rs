//! Protocol Buffers schema generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates a Protocol Buffers schema definition from an SObject describe result.
#[cfg(feature = "schema")]
#[must_use]
pub fn generate_protobuf_schema(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_protobuf_schema(&mut out, describe);
    out
}

/// Writes a Protocol Buffers schema definition from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_protobuf_schema(out: &mut String, describe: &SObjectDescribe) {
    let _ = writeln!(out, "syntax = \"proto3\";");
    let _ = writeln!(out, "package com.salesforce.schema;");
    let _ = writeln!(out);
    let _ = writeln!(out, "message {} {{", describe.name);

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| {
        if a.name == "Id" && b.name == "Id" {
            std::cmp::Ordering::Equal
        } else if a.name == "Id" {
            std::cmp::Ordering::Less
        } else if b.name == "Id" {
            std::cmp::Ordering::Greater
        } else {
            a.name.cmp(&b.name)
        }
    });

    let mut field_number = 1;
    for field in fields {
        let proto_type = map_type(&field.type_);

        let optional_modifier = if field.nillable { "optional " } else { "" };

        let _ = writeln!(
            out,
            "  {}{} {} = {};",
            optional_modifier, proto_type, field.name, field_number
        );
        field_number += 1;
    }

    let _ = writeln!(out, "}}");
}

/// Maps a Salesforce `FieldType` to a Protocol Buffers scalar type.
#[cfg(feature = "schema")]
fn map_type(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Boolean => "bool",
        FieldType::Int => "int32",
        FieldType::Double | FieldType::Currency | FieldType::Percent => "double",
        // Most Salesforce types serialize as strings
        _ => "string",
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;

    use crate::test_support::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};

    #[test]
    fn test_protobuf_generator() {
        let describe = MockSObjectDescribeBuilder::new("Account")
            .field(
                MockFieldDescribeBuilder::new("Id", FieldType::Id)
                    .label("Id")
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
                    .label("Name")
                    .length(255)
                    .byte_length(255)
                    .nillable(false)
                    .createable(true)
                    .updateable(true)
                    .permissionable(false)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("NumberOfEmployees", FieldType::Int)
                    .label("Employees")
                    .length(0)
                    .byte_length(0)
                    .nillable(true)
                    .createable(true)
                    .updateable(true)
                    .permissionable(false)
                    .digits(8)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("AnnualRevenue", FieldType::Currency)
                    .label("Annual Revenue")
                    .length(0)
                    .byte_length(0)
                    .nillable(true)
                    .createable(true)
                    .updateable(true)
                    .permissionable(false)
                    .digits(18)
                    .precision(18)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("IsActive", FieldType::Boolean)
                    .label("Active")
                    .length(0)
                    .byte_length(0)
                    .nillable(true)
                    .createable(true)
                    .updateable(true)
                    .permissionable(false)
                    .build(),
            )
            .build();

        let proto_code = generate_protobuf_schema(&describe);

        let expected = r#"syntax = "proto3";
package com.salesforce.schema;

message Account {
  string Id = 1;
  optional double AnnualRevenue = 2;
  optional bool IsActive = 3;
  string Name = 4;
  optional int32 NumberOfEmployees = 5;
}
"#;
        assert_eq!(proto_code, expected);
    }
}
