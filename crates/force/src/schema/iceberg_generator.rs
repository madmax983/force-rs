//! Apache Iceberg schema generator for Salesforce SObject Describe metadata.
//!
//! Generates an Apache Iceberg table schema document as a [`serde_json::Value`]
//! from a Salesforce object schema. The produced JSON is a valid Iceberg schema
//! document (a `struct` type with a monotonically increasing field-id sequence)
//! that iceberg-rust's `iceberg::spec::Schema` can deserialize directly. This
//! keeps the `force` crate free of heavy Iceberg/Arrow dependencies while still
//! owning the canonical Salesforce-to-Iceberg type mapping.

#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
#[cfg(feature = "schema")]
use serde_json::{Value, json};

/// Generates an Apache Iceberg schema document for a given SObject.
///
/// The returned [`Value`] is an Iceberg `struct` schema with `schema-id` 0 and a
/// `fields` array. Field ids are assigned monotonically starting at 1, in the
/// same stable order used by the BigQuery generator (`Id` first, then
/// alphabetical via `crate::schema::cmp_field_names`). A field is `required`
/// when it is not nillable.
///
/// # Type mapping
///
/// | Salesforce `FieldType`                                                                                                   | Iceberg type          |
/// |--------------------------------------------------------------------------------------------------------------------------|-----------------------|
/// | `String`, `Picklist`, `Multipicklist`, `Combobox`, `Reference`, `Textarea`, `Phone`, `Id`, `Url`, `Email`, `Encryptedstring`, `Datacategorygroupreference`, `Location`, `Address`, `AnyType` | `string`              |
/// | `Base64`                                                                                                                 | `binary`              |
/// | `Int`                                                                                                                    | `int`                 |
/// | `Currency`, `Percent`, `Double`                                                                                          | `decimal(p,s)` / `double` |
/// | `Boolean`                                                                                                                | `boolean`             |
/// | `Date`                                                                                                                   | `date`                |
/// | `Datetime`                                                                                                               | `timestamptz`         |
/// | `Time`                                                                                                                   | `time`                |
///
/// Numeric decimal fields map to `decimal(precision, scale)` only when
/// `precision > 0 && scale >= 0 && precision <= 38`; otherwise they fall back to
/// `double`.
///
/// Note: Salesforce `Long` is not a distinct [`FieldType`] variant, so the
/// widest integer variant available (`Int`) maps to Iceberg `int`.
#[cfg(feature = "schema")]
#[must_use]
pub fn generate_iceberg_schema(describe: &SObjectDescribe) -> Value {
    let mut sorted_fields: Vec<&_> = describe.fields.iter().collect();
    sorted_fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    let mut fields = Vec::with_capacity(sorted_fields.len());
    for (index, field) in sorted_fields.into_iter().enumerate() {
        let iceberg_type = map_field_type(&field.type_, field.precision, field.scale);
        // Field ids are 1-based and monotonically increasing.
        let id = i64::try_from(index).unwrap_or(i64::MAX).saturating_add(1);
        fields.push(json!({
            "id": id,
            "name": field.name,
            "required": !field.nillable,
            "type": iceberg_type,
            "doc": field.label,
        }));
    }

    json!({
        "type": "struct",
        "schema-id": 0,
        "fields": fields,
    })
}

/// Maps a Salesforce [`FieldType`] to an Iceberg primitive type string.
#[cfg(feature = "schema")]
fn map_field_type(field_type: &FieldType, precision: i32, scale: i32) -> String {
    match field_type {
        FieldType::Base64 => "binary".to_owned(),
        FieldType::Int => "int".to_owned(),
        FieldType::Currency | FieldType::Percent | FieldType::Double => {
            if precision > 0 && scale >= 0 && precision <= 38 {
                format!("decimal({precision},{scale})")
            } else {
                "double".to_owned()
            }
        }
        FieldType::Boolean => "boolean".to_owned(),
        FieldType::Date => "date".to_owned(),
        FieldType::Datetime => "timestamptz".to_owned(),
        FieldType::Time => "time".to_owned(),
        // All remaining textual / reference / structured types map to string.
        FieldType::String
        | FieldType::Picklist
        | FieldType::Multipicklist
        | FieldType::Combobox
        | FieldType::Reference
        | FieldType::Textarea
        | FieldType::Phone
        | FieldType::Id
        | FieldType::Url
        | FieldType::Email
        | FieldType::Encryptedstring
        | FieldType::Datacategorygroupreference
        | FieldType::Location
        | FieldType::Address
        | FieldType::AnyType => "string".to_owned(),
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_utils::mock_describe::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};
    use crate::test_utils::must::MustMsg;

    fn sample_describe() -> SObjectDescribe {
        MockSObjectDescribeBuilder::new("Opportunity")
            .field(
                MockFieldDescribeBuilder::new("Name", FieldType::String)
                    .label("Opportunity Name")
                    .length(120)
                    .byte_length(360)
                    .nillable(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("Id", FieldType::Id)
                    .label("Opportunity ID")
                    .length(18)
                    .byte_length(18)
                    .nillable(false)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("Amount", FieldType::Currency)
                    .label("Amount")
                    .nillable(true)
                    .precision(18)
                    .scale(2)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("Quantity", FieldType::Int)
                    .label("Quantity")
                    .nillable(true)
                    .precision(8)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("CloseDate", FieldType::Datetime)
                    .label("Close Date")
                    .nillable(true)
                    .build(),
            )
            .build()
    }

    #[test]
    fn test_generates_valid_iceberg_struct_document() {
        let schema = generate_iceberg_schema(&sample_describe());
        assert_eq!(schema["type"], "struct");
        assert_eq!(schema["schema-id"], 0);
        let fields = schema["fields"]
            .as_array()
            .must_msg("Expected fields array");
        assert_eq!(fields.len(), 5);
    }

    #[test]
    fn test_id_is_first_string_required_with_id_one() {
        let schema = generate_iceberg_schema(&sample_describe());
        let fields = schema["fields"]
            .as_array()
            .must_msg("Expected fields array");
        // Id-first ordering.
        assert_eq!(fields[0]["name"], "Id");
        assert_eq!(fields[0]["id"], 1);
        assert_eq!(fields[0]["type"], "string");
        assert_eq!(fields[0]["required"], true);
    }

    #[test]
    fn test_field_ids_are_monotonic_from_one() {
        let schema = generate_iceberg_schema(&sample_describe());
        let fields = schema["fields"]
            .as_array()
            .must_msg("Expected fields array");
        for (index, field) in fields.iter().enumerate() {
            let expected = i64::try_from(index).unwrap_or(i64::MAX) + 1;
            assert_eq!(field["id"], expected);
        }
    }

    #[test]
    fn test_currency_with_precision_scale_maps_to_decimal() {
        let schema = generate_iceberg_schema(&sample_describe());
        let fields = schema["fields"]
            .as_array()
            .must_msg("Expected fields array");
        let amount = fields
            .iter()
            .find(|f| f["name"] == "Amount")
            .must_msg("Expected Amount field");
        assert_eq!(amount["type"], "decimal(18,2)");
    }

    #[test]
    fn test_int_maps_to_int() {
        let schema = generate_iceberg_schema(&sample_describe());
        let fields = schema["fields"]
            .as_array()
            .must_msg("Expected fields array");
        let quantity = fields
            .iter()
            .find(|f| f["name"] == "Quantity")
            .must_msg("Expected Quantity field");
        assert_eq!(quantity["type"], "int");
    }

    #[test]
    fn test_datetime_maps_to_timestamptz() {
        let schema = generate_iceberg_schema(&sample_describe());
        let fields = schema["fields"]
            .as_array()
            .must_msg("Expected fields array");
        let close = fields
            .iter()
            .find(|f| f["name"] == "CloseDate")
            .must_msg("Expected CloseDate field");
        assert_eq!(close["type"], "timestamptz");
    }

    #[test]
    fn test_nillable_string_is_not_required() {
        let schema = generate_iceberg_schema(&sample_describe());
        let fields = schema["fields"]
            .as_array()
            .must_msg("Expected fields array");
        let name = fields
            .iter()
            .find(|f| f["name"] == "Name")
            .must_msg("Expected Name field");
        assert_eq!(name["required"], false);
    }

    #[test]
    fn test_decimal_fallback_to_double_when_precision_zero() {
        let describe = MockSObjectDescribeBuilder::new("Account")
            .field(
                MockFieldDescribeBuilder::new("Score", FieldType::Double)
                    .label("Score")
                    .nillable(true)
                    .precision(0)
                    .scale(0)
                    .build(),
            )
            .build();
        let schema = generate_iceberg_schema(&describe);
        let fields = schema["fields"]
            .as_array()
            .must_msg("Expected fields array");
        assert_eq!(fields[0]["type"], "double");
    }
}
