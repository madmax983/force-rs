//! Go struct generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Preview utility to generate Go structs from SObject describe metadata.
#[cfg(feature = "schema")]
/// Generates a Go struct definition from an SObject describe result.
pub fn generate_go_struct(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_go_struct(&mut out, describe);
    out
}

/// Writes a Go struct definition from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_go_struct(out: &mut String, describe: &SObjectDescribe) {
    let _ = writeln!(
        out,
        "// {} represents the {} Salesforce object.",
        describe.name, describe.name
    );
    let _ = writeln!(out, "type {} struct {{", describe.name);

    // Sort fields alphabetically, Id first
    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in fields {
        let go_type = map_type(&field.type_);
        let pointer = if field.nillable { "*" } else { "" };
        let go_field_name = to_pascal_case(&field.name);

        // Go struct tags for JSON
        let _ = writeln!(
            out,
            "\t{} {}{} `json:\"{}\"`",
            go_field_name, pointer, go_type, field.name
        );
    }

    out.push_str("}\n");
}

#[cfg(feature = "schema")]
fn to_pascal_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = true;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
            // Go typically drops underscores in pascal case
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(feature = "schema")]
fn map_type(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Boolean => "bool",
        FieldType::Int => "int64",
        FieldType::Double | FieldType::Currency | FieldType::Percent => "float64",
        _ => "string",
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_utils::mock_describe::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};

    #[test]
    fn test_go_generator() {
        let describe = MockSObjectDescribeBuilder::new("Account")
            .field(
                MockFieldDescribeBuilder::new("Id", FieldType::Id)
                    .nillable(false)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("Name", FieldType::String)
                    .nillable(false)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("NumberOfEmployees", FieldType::Int)
                    .nillable(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("IsActive", FieldType::Boolean)
                    .nillable(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("Custom_Field__c", FieldType::String)
                    .nillable(true)
                    .build(),
            )
            .build();

        let go = generate_go_struct(&describe);

        let expected = "// Account represents the Account Salesforce object.\ntype Account struct {\n\tId string `json:\"Id\"`\n\tCustomFieldC *string `json:\"Custom_Field__c\"`\n\tIsActive *bool `json:\"IsActive\"`\n\tName string `json:\"Name\"`\n\tNumberOfEmployees *int64 `json:\"NumberOfEmployees\"`\n}\n";
        assert_eq!(go, expected);
    }
}
