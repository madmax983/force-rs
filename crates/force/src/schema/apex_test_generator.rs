#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
#[cfg(feature = "schema")]
use std::fmt::Write;

/// Generates an Apex `@isTest` Test Data Factory class for the given SObject.
#[cfg(feature = "schema")]
#[must_use]
pub fn generate_apex_test_factory(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 64 + 256);
    write_apex_test_factory(&mut out, describe);
    out
}

/// Writes an Apex `@isTest` Test Data Factory class for the given SObject directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_apex_test_factory(out: &mut String, describe: &SObjectDescribe) {
    let class_name = format!("{}TestFactory", describe.name.replace("__c", ""));
    let method_name = format!("create{}", describe.name.replace("__c", ""));

    let _ = writeln!(out, "@isTest");
    let _ = writeln!(out, "public class {} {{", class_name);
    let _ = writeln!(
        out,
        "    public static {} {}() {{",
        describe.name, method_name
    );
    let _ = writeln!(out, "        return new {}(", describe.name);

    let mut fields: Vec<&crate::types::describe::FieldDescribe> =
        describe.fields.iter().filter(|f| f.createable).collect();

    // Sort to ensure deterministic output
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    let mut first = true;
    for field in fields {
        if !first {
            let _ = writeln!(out, ",");
        }
        first = false;

        let _ = write!(
            out,
            "            {} = {}",
            field.name,
            map_apex_literal(field)
        );
    }

    if !first {
        let _ = writeln!(out);
    }
    let _ = writeln!(out, "        );");
    let _ = writeln!(out, "    }}");
    let _ = writeln!(out, "}}");
}

#[cfg(feature = "schema")]
fn map_apex_literal(field: &crate::types::describe::FieldDescribe) -> String {
    match field.type_ {
        FieldType::Boolean => "true".to_string(),
        FieldType::Int | FieldType::Double | FieldType::Currency | FieldType::Percent => {
            "42".to_string()
        }
        FieldType::Date => "System.today()".to_string(),
        FieldType::Datetime => "System.now()".to_string(),
        FieldType::Time => "Time.newInstance(12, 0, 0, 0)".to_string(),
        FieldType::Reference => {
            // Provide a TODO for developers
            "null // TODO: Inject related record ID".to_string()
        }
        FieldType::Picklist | FieldType::Multipicklist => {
            if let Some(ref picklist_values) = field.picklist_values {
                if let Some(val) = picklist_values.iter().find(|p| p.active) {
                    format!("'{}'", val.value.replace('\'', "\\'"))
                } else {
                    "'Test'".to_string()
                }
            } else {
                "'Test'".to_string()
            }
        }
        _ => "'Test String'".to_string(),
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_utils::mock_describe::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};
    use serde_json::json;

    #[test]
    fn test_generate_apex_test_factory() {
        let mut picklist_field = MockFieldDescribeBuilder::new("Industry", FieldType::Picklist)
            .createable(true)
            .build();

        // Mock the picklist values
        let picklist_json = json!([
            {"active": false, "defaultValue": false, "label": "Inactive", "value": "Inactive"},
            {"active": true, "defaultValue": false, "label": "Technology", "value": "Technology"}
        ]);
        picklist_field.picklist_values = serde_json::from_value(picklist_json).ok();

        let describe = MockSObjectDescribeBuilder::new("Account")
            .field(
                MockFieldDescribeBuilder::new("Id", FieldType::Id)
                    .createable(false)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("Name", FieldType::String)
                    .createable(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("NumberOfEmployees", FieldType::Int)
                    .createable(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("IsActive", FieldType::Boolean)
                    .createable(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("CloseDate", FieldType::Date)
                    .createable(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("OwnerId", FieldType::Reference)
                    .createable(true)
                    .build(),
            )
            .field(picklist_field)
            .build();

        let result = generate_apex_test_factory(&describe);

        let expected = r"@isTest
public class AccountTestFactory {
    public static Account createAccount() {
        return new Account(
            CloseDate = System.today(),
            Industry = 'Technology',
            IsActive = true,
            Name = 'Test String',
            NumberOfEmployees = 42,
            OwnerId = null // TODO: Inject related record ID
        );
    }
}
";

        assert_eq!(result, expected);
    }

    #[test]
    fn test_generate_apex_test_factory_custom_object() {
        let describe = MockSObjectDescribeBuilder::new("CustomObject__c")
            .field(
                MockFieldDescribeBuilder::new("Name", FieldType::String)
                    .createable(true)
                    .build(),
            )
            .build();

        let result = generate_apex_test_factory(&describe);

        let expected = r"@isTest
public class CustomObjectTestFactory {
    public static CustomObject__c createCustomObject() {
        return new CustomObject__c(
            Name = 'Test String'
        );
    }
}
";

        assert_eq!(result, expected);
    }
}
