#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};

/// Generates a CSV template for an SObject with headers and optional mock data.
#[cfg(feature = "schema")]
pub fn generate_csv_template(describe: &SObjectDescribe, include_mock_data: bool) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 64);

    // Generate Headers
    let mut createable_fields: Vec<&_> = describe.fields.iter().filter(|f| f.createable).collect();
    // Sort fields to ensure deterministic output
    createable_fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    let mut first = true;
    for field in &createable_fields {
        if !first {
            out.push(',');
        }
        out.push_str(&field.name);
        first = false;
    }
    out.push('\n');

    // Optional Mock Data Row
    if include_mock_data {
        first = true;
        for field in &createable_fields {
            if !first {
                out.push(',');
            }
            let mock_val = match field.type_ {
                FieldType::String
                | FieldType::Textarea
                | FieldType::Email
                | FieldType::Phone
                | FieldType::Url => "mock_string",
                FieldType::Boolean => "true",
                FieldType::Int => "42",
                FieldType::Double | FieldType::Currency | FieldType::Percent => "42.0",
                FieldType::Date => "2023-01-01",
                FieldType::Datetime => "2023-01-01T00:00:00Z",
                FieldType::Id | FieldType::Reference => "001000000000000AAA",
                _ => "",
            };
            out.push_str(mock_val);
            first = false;
        }
        out.push('\n');
    }

    out
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
                MockFieldDescribeBuilder::new("IsActive", FieldType::Boolean)
                    .createable(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("NumberOfEmployees", FieldType::Int)
                    .createable(true)
                    .build(),
            )
            .build();

        let result_no_mock = generate_csv_template(&describe, false);
        assert_eq!(result_no_mock, "IsActive,Name,NumberOfEmployees\n");

        let result_with_mock = generate_csv_template(&describe, true);
        assert_eq!(
            result_with_mock,
            "IsActive,Name,NumberOfEmployees\ntrue,mock_string,42\n"
        );
    }
}
