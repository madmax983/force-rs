//! Swift Codable struct generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates a Swift struct definition from an SObject describe result.
#[cfg(feature = "schema")]
pub fn generate_swift_struct(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_swift_struct(&mut out, describe);
    out
}

/// Writes a Swift struct definition from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_swift_struct(out: &mut String, describe: &SObjectDescribe) {
    let _ = writeln!(out, "/// {}", describe.label);
    let _ = writeln!(out, "public struct {}: Codable {{", describe.name);

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in fields {
        let swift_type = map_type(&field.type_);
        let optional = if field.nillable { "?" } else { "" };
        let _ = writeln!(
            out,
            "    public var {}: {}{}",
            field.name, swift_type, optional
        );
    }

    out.push_str("}\n");
}

/// Maps a Salesforce `FieldType` to a Swift type.
#[cfg(feature = "schema")]
fn map_type(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Boolean => "Bool",
        FieldType::Int => "Int",
        FieldType::Double | FieldType::Currency | FieldType::Percent => "Double",
        // Typically serialized as strings
        _ => "String",
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_utils::mock_describe::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};

    #[test]
    fn test_swift_generator() {
        let describe = MockSObjectDescribeBuilder::new("Account")
            .field(
                MockFieldDescribeBuilder::new("Id", FieldType::Id)
                    .label("Account ID")
                    .length(18)
                    .nillable(false)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("Name", FieldType::String)
                    .label("Account Name")
                    .length(255)
                    .nillable(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("IsActive", FieldType::Boolean)
                    .label("Active")
                    .length(0)
                    .nillable(false)
                    .build(),
            )
            .build();

        let mut describe = describe;
        describe.label = "Account".to_string();

        let swift_code = generate_swift_struct(&describe);

        let expected = "/// Account\npublic struct Account: Codable {\n    public var Id: String\n    public var IsActive: Bool\n    public var Name: String?\n}\n";
        assert_eq!(swift_code, expected);
    }
}
