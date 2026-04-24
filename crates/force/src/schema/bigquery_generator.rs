//! BigQuery JSON schema generator for Salesforce SObject Describe metadata.
//!
//! Generates Google BigQuery table schema arrays based on Salesforce object schemas.

#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
#[cfg(feature = "schema")]
use serde_json::{Value, json};

/// Generates a Google BigQuery JSON schema definition for a given SObject.
#[cfg(feature = "schema")]
pub fn generate_bigquery_schema(describe: &SObjectDescribe) -> Value {
    let mut schema_fields = Vec::with_capacity(describe.fields.len());

    let mut sorted_fields: Vec<&_> = describe.fields.iter().collect();
    sorted_fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in sorted_fields {
        let bq_type = match field.type_ {
            FieldType::Int => "INTEGER",
            FieldType::Double | FieldType::Percent | FieldType::Currency => "FLOAT",
            FieldType::Boolean => "BOOLEAN",
            FieldType::Date => "DATE",
            FieldType::Datetime => "TIMESTAMP",
            FieldType::Time => "TIME",
            FieldType::Base64 => "BYTES",
            _ => "STRING",
        };

        let mode = if field.nillable {
            "NULLABLE"
        } else {
            "REQUIRED"
        };

        schema_fields.push(json!({
            "name": field.name,
            "type": bq_type,
            "mode": mode,
            "description": field.label
        }));
    }

    Value::Array(schema_fields)
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use crate::test_support::{
        MockFieldDescribeBuilder, MockSObjectDescribeBuilder, MustMsg,
    };
    use super::*;

    #[test]
    fn test_generate_bigquery_schema() {
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
                    .byte_length(765)
                    .nillable(false)
                    .createable(true)
                    .updateable(true)
                    .permissionable(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("NumberOfEmployees", FieldType::Int)
                    .label("Employees")
                    .length(0)
                    .byte_length(0)
                    .nillable(true)
                    .createable(true)
                    .updateable(true)
                    .permissionable(true)
                    .precision(8)
                    .build(),
            )
            .build();

        let schema = generate_bigquery_schema(&describe);

        let arr = schema.as_array().must_msg("Expected JSON array");
        assert_eq!(arr.len(), 3);

        // Sorting should guarantee order: Id, Name, NumberOfEmployees
        assert_eq!(arr[0]["name"], "Id");
        assert_eq!(arr[0]["type"], "STRING");
        assert_eq!(arr[0]["mode"], "REQUIRED");
        assert_eq!(arr[0]["description"], "Account ID");

        assert_eq!(arr[1]["name"], "Name");
        assert_eq!(arr[1]["type"], "STRING");
        assert_eq!(arr[1]["mode"], "REQUIRED");
        assert_eq!(arr[1]["description"], "Account Name");

        assert_eq!(arr[2]["name"], "NumberOfEmployees");
        assert_eq!(arr[2]["type"], "INTEGER");
        assert_eq!(arr[2]["mode"], "NULLABLE");
        assert_eq!(arr[2]["description"], "Employees");
    }
}
