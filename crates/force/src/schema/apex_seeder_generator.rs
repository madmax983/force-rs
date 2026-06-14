//! Apex Seeder Generator for Salesforce SObject Describe metadata.

#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates an Anonymous Apex script to seed a single record based on an SObject describe result.
#[cfg(feature = "schema")]
#[must_use]
pub fn generate_apex_seeder(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(1024);
    write_apex_seeder(&mut out, describe);
    out
}

/// Writes an Anonymous Apex script to seed a record based on an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
#[allow(clippy::missing_panics_doc)]
pub fn write_apex_seeder(out: &mut String, describe: &SObjectDescribe) {
    let _ = writeln!(out, "// Generated Apex Seeder for {}", describe.name);
    let _ = writeln!(out, "{} newRecord = new {} (", describe.name, describe.name);

    let mut fields: Vec<&_> = describe.fields.iter().filter(|f| f.createable).collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    let mut first = true;
    for field in fields {
        if !first {
            let _ = writeln!(out, ",");
        }
        first = false;

        let mock_val = match field.type_ {
            FieldType::String
            | FieldType::Textarea
            | FieldType::Email
            | FieldType::Phone
            | FieldType::Url => "'mock_string'",
            FieldType::Boolean => "true",
            FieldType::Int => "42",
            FieldType::Double | FieldType::Currency | FieldType::Percent => "42.0",
            FieldType::Date => "Date.today()",
            FieldType::Datetime => "Datetime.now()",
            FieldType::Id | FieldType::Reference => "'001000000000000AAA'",
            _ => "null",
        };

        let _ = write!(out, "    {} = {}", field.name, mock_val);
    }

    let _ = writeln!(out, "\n);");
    let _ = writeln!(out, "insert newRecord;");
    let _ = writeln!(out, "System.debug('Inserted: ' + newRecord.Id);");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::mock_describe::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};

    #[test]
    fn test_generate_apex_seeder() {
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
            .build();

        let script = generate_apex_seeder(&describe);

        assert!(script.contains("Account newRecord = new Account ("));
        assert!(script.contains("    Name = 'mock_string',"));
        assert!(script.contains("    NumberOfEmployees = 42"));
        assert!(script.contains(");"));
        assert!(script.contains("insert newRecord;"));
        assert!(!script.contains("    Id = "));
    }
}
