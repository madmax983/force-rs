//! Apex Test Data Factory Generator.
//!
//! Generates Salesforce Apex classes to instantiate valid mock records for Unit Testing
//! based on SObject Describe metadata.

use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates an Apex Data Factory class for the given SObject.
///
/// This provides a boilerplate `@isTest` class that sets default values for required
/// or commonly used fields to make unit testing easier for Salesforce developers.
#[cfg(feature = "schema")]
pub fn generate_apex_factory(describe: &SObjectDescribe) -> String {
    let mut out = String::new();
    let class_name = format!("{}DataFactory", describe.name.replace("__c", ""));

    let _ = writeln!(&mut out, "@isTest");
    let _ = writeln!(&mut out, "public class {} {{", class_name);
    let _ = writeln!(
        &mut out,
        "    public static {} create{}() {{",
        describe.name,
        describe.name.replace("__c", "")
    )
    ;
    let _ = writeln!(
        &mut out,
        "        {} obj = new {}();",
        describe.name, describe.name
    )
    ;

    let mut fields: Vec<_> = describe
        .fields
        .iter()
        .filter(|f| f.createable && !f.nillable && f.name != "OwnerId" && !f.defaulted_on_create)
        .collect();

    fields.sort_by(|a, b| a.name.cmp(&b.name));

    for field in fields {
        let default_val = match field.type_ {
            FieldType::String
            | FieldType::Textarea
            | FieldType::Email
            | FieldType::Phone
            | FieldType::Url => "'Test String'".to_string(),
            FieldType::Boolean => "true".to_string(),
            FieldType::Int => "42".to_string(),
            FieldType::Double | FieldType::Currency | FieldType::Percent => "42.0".to_string(),
            FieldType::Id => "'001000000000000AAA'".to_string(),
            _ => "null".to_string(),
        };

        let _ = writeln!(&mut out, "        obj.{} = {};", field.name, default_val);
    }

    let _ = writeln!(&mut out, "        return obj;");
    let _ = writeln!(&mut out, "    }}");
    let _ = writeln!(&mut out, "}}");

    out
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_utils::mock_describe::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};

    #[test]
    fn test_generate_apex_factory() {
        let describe = MockSObjectDescribeBuilder::new("Account")
            .field(
                MockFieldDescribeBuilder::new("Id", FieldType::Id)
                    .createable(false)
                    .nillable(false)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("Name", FieldType::String)
                    .createable(true)
                    .nillable(false)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("AnnualRevenue", FieldType::Currency)
                    .createable(true)
                    .nillable(false)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("Description", FieldType::Textarea)
                    .createable(true)
                    .nillable(true)
                    .build(),
            )
            .build();

        let apex = generate_apex_factory(&describe);

        let expected = "@isTest\npublic class AccountDataFactory {\n    public static Account createAccount() {\n        Account obj = new Account();\n        obj.AnnualRevenue = 42.0;\n        obj.Name = 'Test String';\n        return obj;\n    }\n}\n";
        assert_eq!(apex, expected);
    }
}
