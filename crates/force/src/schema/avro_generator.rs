//! Apache Avro schema generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates an Apache Avro schema definition from an SObject describe result.
#[cfg(feature = "schema")]
#[must_use]
pub fn generate_avro_schema(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_avro_schema(&mut out, describe);
    out
}

/// Writes an Apache Avro schema definition from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_avro_schema(out: &mut String, describe: &SObjectDescribe) {
    let _ = writeln!(out, "{{");
    let _ = writeln!(out, "  \"type\": \"record\",");
    let _ = writeln!(out, "  \"name\": \"{}\",", describe.name);
    let _ = writeln!(out, "  \"namespace\": \"com.salesforce.schema\",");
    let _ = writeln!(out, "  \"fields\": [");

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    let mut first = true;
    for field in fields {
        if first {
            first = false;
        } else {
            let _ = writeln!(out, ",");
        }

        let avro_type = map_type(&field.type_);

        let _ = write!(out, "    {{ \"name\": \"{}\", \"type\": ", field.name);

        if field.nillable {
            let _ = write!(out, "[\"null\", \"{}\"] }}", avro_type);
        } else {
            let _ = write!(out, "\"{}\" }}", avro_type);
        }
    }

    let _ = writeln!(out);
    let _ = writeln!(out, "  ]");
    let _ = write!(out, "}}");
}

/// Maps a Salesforce `FieldType` to an Apache Avro type.
#[cfg(feature = "schema")]
fn map_type(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Boolean => "boolean",
        FieldType::Int => "int",
        FieldType::Double | FieldType::Currency | FieldType::Percent => "double",
        // Most Salesforce types serialize as strings
        _ => "string",
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_support::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};

    #[test]
    fn test_avro_generator() {
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
                MockFieldDescribeBuilder::new("AnnualRevenue", FieldType::Currency)
                    .nillable(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("IsActive", FieldType::Boolean)
                    .nillable(true)
                    .build(),
            )
            .build();

        let avro_code = generate_avro_schema(&describe);

        let expected = r#"{
  "type": "record",
  "name": "Account",
  "namespace": "com.salesforce.schema",
  "fields": [
    { "name": "Id", "type": "string" },
    { "name": "AnnualRevenue", "type": ["null", "double"] },
    { "name": "IsActive", "type": ["null", "boolean"] },
    { "name": "Name", "type": "string" },
    { "name": "NumberOfEmployees", "type": ["null", "int"] }
  ]
}"#;
        assert_eq!(avro_code, expected);
    }
}
