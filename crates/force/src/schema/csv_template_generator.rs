//! CSV Template Generator for Salesforce Bulk API ingest operations.

#[cfg(feature = "schema")]
use crate::types::describe::SObjectDescribe;

/// Generates a CSV template header string from an SObject describe result.
///
/// Only includes fields that are `createable` to match Bulk API ingest requirements.
#[cfg(feature = "schema")]
pub fn generate_csv_template(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 16);
    write_csv_template(&mut out, describe);
    out
}

/// Writes a CSV template header string directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_csv_template(out: &mut String, describe: &SObjectDescribe) {
    let mut createable_fields: Vec<&_> = describe.fields.iter().filter(|f| f.createable).collect();

    // Sort alphabetically, with Id first if it somehow is createable
    createable_fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    let mut first = true;
    for field in createable_fields {
        if !first {
            out.push(',');
        }
        out.push_str(&field.name);
        first = false;
    }
    out.push('\n');
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_utils::mock_describe::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};
    use crate::types::describe::FieldType;

    #[test]
    fn test_generate_csv_template() {
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

        let result = generate_csv_template(&describe);
        assert_eq!(result, "Name,NumberOfEmployees\n");
    }
}
