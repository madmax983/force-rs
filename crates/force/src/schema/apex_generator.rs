//! Apex class generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates an Apex class definition from an SObject describe result.
#[cfg(feature = "schema")]
#[must_use]
pub fn generate_apex_class(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_apex_class(&mut out, describe);
    out
}

/// Writes an Apex class definition from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_apex_class(out: &mut String, describe: &SObjectDescribe) {
    let _ = writeln!(out, "/**");
    let _ = writeln!(out, " * {}", describe.label);
    let _ = writeln!(out, " */");
    let _ = writeln!(out, "public class {} {{", describe.name);

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in fields {
        let apex_type = map_type(&field.type_);
        let _ = writeln!(
            out,
            "    @AuraEnabled public {} {} {{ get; set; }}",
            apex_type, field.name
        );
    }
    let _ = writeln!(out, "}}");
}

#[cfg(feature = "schema")]
fn map_type(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Boolean => "Boolean",
        FieldType::Int => "Integer",
        FieldType::Double | FieldType::Currency | FieldType::Percent => "Decimal",
        FieldType::Date => "Date",
        FieldType::Datetime => "Datetime",
        FieldType::Id | FieldType::Reference => "Id",
        FieldType::Base64 => "Blob",
        _ => "String",
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_utils::mock_describe::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};

    #[test]
    fn test_generate_apex_class() {
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

        let result = generate_apex_class(&describe);
        let expected = "/**\n * Account Object\n */\npublic class Account {\n    @AuraEnabled public Id Id { get; set; }\n    @AuraEnabled public Boolean IsActive { get; set; }\n    @AuraEnabled public String Name { get; set; }\n}\n";
        assert_eq!(result, expected);
    }
}
