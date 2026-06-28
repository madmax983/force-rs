#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates an Apex Test Data Factory class for the given SObject describe metadata.
///
/// This automatically populates required fields (that cannot be null and have no default)
/// with dummy data, providing a clean boilerplate for writing Apex tests.
#[cfg(feature = "schema")]
#[must_use]
pub fn generate_apex_test_factory(describe: &SObjectDescribe) -> String {
    let class_name = format!("{}TestFactory", describe.name.replace("__c", ""));
    let method_name = format!("create{}", describe.name.replace("__c", ""));

    let mut out = String::with_capacity(1024);
    let _ = writeln!(out, "/// Auto-generated Test Factory for {}", describe.name);
    let _ = writeln!(out, "@isTest");
    let _ = writeln!(out, "public class {} {{", class_name);

    let _ = writeln!(
        out,
        "    public static {} {}(Boolean doInsert) {{",
        describe.name, method_name
    );
    let _ = writeln!(
        out,
        "        {} record = new {}();",
        describe.name, describe.name
    );

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by_key(|f| &f.name);

    for field in fields {
        if !field.createable || field.nillable || field.defaulted_on_create || field.name == "Id" {
            continue; // Skip optional or read-only fields
        }

        let val = match field.type_ {
            FieldType::String
            | FieldType::Textarea
            | FieldType::Email
            | FieldType::Phone
            | FieldType::Url => "'Test String'",
            FieldType::Boolean => "true",
            FieldType::Int => "42",
            FieldType::Double | FieldType::Currency | FieldType::Percent => "42.0",
            FieldType::Date => "Date.today()",
            FieldType::Datetime => "Datetime.now()",
            FieldType::Id | FieldType::Reference => "null /* TODO: Provide valid Id */",
            _ => "null",
        };

        let _ = writeln!(out, "        record.{} = {};", field.name, val);
    }

    let _ = writeln!(out, "        if (doInsert) {{");
    let _ = writeln!(out, "            insert record;");
    let _ = writeln!(out, "        }}");
    let _ = writeln!(out, "        return record;");
    let _ = writeln!(out, "    }}");
    let _ = writeln!(out, "}}");

    out
}

/// Writes an Apex Test Data Factory class directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_apex_test_factory(out: &mut String, describe: &SObjectDescribe) {
    let result = generate_apex_test_factory(describe);
    out.push_str(&result);
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_utils::mock_describe::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};

    #[test]
    fn test_generate_apex_test_factory() {
        let describe = MockSObjectDescribeBuilder::new("Account")
            .field(
                MockFieldDescribeBuilder::new("Id", FieldType::Id)
                    .nillable(false)
                    .createable(false)
                    .defaulted_on_create(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("Name", FieldType::String)
                    .nillable(false)
                    .createable(true)
                    .defaulted_on_create(false)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("IsActive", FieldType::Boolean)
                    .nillable(false)
                    .createable(true)
                    .defaulted_on_create(false)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("Optional__c", FieldType::String)
                    .nillable(true)
                    .createable(true)
                    .defaulted_on_create(false)
                    .build(),
            )
            .build();

        let factory = generate_apex_test_factory(&describe);

        assert!(factory.contains("@isTest"));
        assert!(factory.contains("public class AccountTestFactory"));
        assert!(factory.contains("public static Account createAccount(Boolean doInsert)"));
        assert!(factory.contains("record.Name = 'Test String';"));
        assert!(factory.contains("record.IsActive = true;"));
        assert!(!factory.contains("record.Optional__c"));
        assert!(!factory.contains("record.Id"));
        assert!(factory.contains("insert record;"));
    }
}
