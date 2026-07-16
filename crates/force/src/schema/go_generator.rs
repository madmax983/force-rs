#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates a Go struct definition from an SObject describe result.
#[cfg(feature = "schema")]
pub fn generate_go_struct(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_go_struct(&mut out, describe);
    out
}

/// Writes a Go struct definition from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_go_struct(out: &mut String, describe: &SObjectDescribe) {
    let _ = writeln!(out, "// {}", describe.label);
    out.push_str("type ");
    out.push_str(&describe.name);
    out.push_str(
        " struct {
",
    );

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| {
        if a.name == "Id" {
            std::cmp::Ordering::Less
        } else if b.name == "Id" {
            std::cmp::Ordering::Greater
        } else {
            a.name.cmp(&b.name)
        }
    });

    for field in fields {
        let go_type = map_type(&field.type_);
        out.push('\t');
        out.push_str(&field.name);
        out.push(' ');
        if field.nillable {
            out.push('*');
        }
        out.push_str(go_type);
        let _ = writeln!(out, " `json:\"{}\"` // {}", field.name, field.label);
    }

    out.push_str(
        "}
",
    );
}

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

    #[test]
    fn test_generate_go_struct() {
        use crate::test_utils::mock_describe::{
            MockFieldDescribeBuilder, MockSObjectDescribeBuilder,
        };

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
                    .byte_length(255)
                    .nillable(true)
                    .createable(true)
                    .updateable(true)
                    .permissionable(false)
                    .defaulted_on_create(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("IsActive", FieldType::Boolean)
                    .label("Active")
                    .length(0)
                    .byte_length(0)
                    .nillable(false)
                    .createable(true)
                    .updateable(true)
                    .permissionable(false)
                    .defaulted_on_create(true)
                    .build(),
            )
            .build();

        let mut describe = describe;
        describe.label = "Account Object".to_string();

        let result = generate_go_struct(&describe);

        let expected = r#"// Account Object
type Account struct {
	Id string `json:"Id"` // Account ID
	IsActive bool `json:"IsActive"` // Active
	Name *string `json:"Name"` // Account Name
}
"#;

        assert_eq!(result, expected);
    }
}
