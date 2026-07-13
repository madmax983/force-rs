//! Salesforce describe → Iceberg → Arrow schema mapping.
//!
//! The canonical Salesforce-to-Iceberg type mapping lives in the `force` crate
//! ([`force::schema::generate_iceberg_schema`]), which emits an Iceberg schema
//! document as JSON. This module deserializes that document into a concrete
//! [`iceberg::spec::Schema`] and derives the corresponding Arrow
//! [`arrow_schema::SchemaRef`] used for `RecordBatch` assembly and Parquet
//! writing.

use std::sync::Arc;

use arrow_schema::SchemaRef;
use force::types::SObjectDescribe;
use iceberg::arrow::schema_to_arrow_schema;
use iceberg::spec::Schema as IcebergSchema;

use crate::error::Result;

/// The Iceberg and Arrow schemas derived from a Salesforce object describe.
///
/// Both views are kept together because downstream stages need the Iceberg
/// schema (to create / commit to the table) and the Arrow schema (to build
/// record batches and write Parquet) in lock-step.
#[derive(Debug, Clone)]
pub struct MappedSchema {
    iceberg: Arc<IcebergSchema>,
    arrow: SchemaRef,
}

impl MappedSchema {
    /// The Apache Iceberg schema.
    #[must_use]
    pub fn iceberg(&self) -> &IcebergSchema {
        &self.iceberg
    }

    /// A cheaply cloneable handle to the Iceberg schema.
    #[must_use]
    pub fn iceberg_ref(&self) -> Arc<IcebergSchema> {
        Arc::clone(&self.iceberg)
    }

    /// The Arrow schema derived from the Iceberg schema.
    #[must_use]
    pub fn arrow(&self) -> SchemaRef {
        Arc::clone(&self.arrow)
    }
}

/// Builds an [`iceberg::spec::Schema`] from a Salesforce object describe.
///
/// Delegates the type mapping to [`force::schema::generate_iceberg_schema`] and
/// deserializes the resulting JSON document into an Iceberg schema.
///
/// # Errors
///
/// Returns [`crate::error::LakeError::Serde`] if the generated schema document
/// cannot be deserialized into an [`iceberg::spec::Schema`] (which would signal
/// an incompatibility between the generator and the pinned iceberg-rust version).
pub fn iceberg_schema_from_describe(describe: &SObjectDescribe) -> Result<IcebergSchema> {
    let document = force::schema::generate_iceberg_schema(describe);
    let schema = serde_json::from_value::<IcebergSchema>(document)?;
    Ok(schema)
}

/// Derives an Arrow [`SchemaRef`] from an [`iceberg::spec::Schema`].
///
/// # Errors
///
/// Returns [`crate::error::LakeError::Iceberg`] if the Iceberg schema contains a
/// type that has no Arrow representation in the pinned iceberg-rust version.
pub fn arrow_schema_from_iceberg(schema: &IcebergSchema) -> Result<SchemaRef> {
    let arrow = schema_to_arrow_schema(schema)?;
    Ok(Arc::new(arrow))
}

/// Builds both the Iceberg and Arrow schemas for a Salesforce object describe.
///
/// # Errors
///
/// Propagates errors from [`iceberg_schema_from_describe`] and
/// [`arrow_schema_from_iceberg`].
pub fn map_schema(describe: &SObjectDescribe) -> Result<MappedSchema> {
    let iceberg = iceberg_schema_from_describe(describe)?;
    let arrow = arrow_schema_from_iceberg(&iceberg)?;
    Ok(MappedSchema {
        iceberg: Arc::new(iceberg),
        arrow,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{describe_json, field_json};
    use arrow_schema::DataType;
    use serde_json::json;

    fn opportunity_describe() -> SObjectDescribe {
        let value = describe_json(
            "Opportunity",
            vec![
                field_json("Name", "string", json!({"nillable": true})),
                field_json("Id", "id", json!({"nillable": false})),
                field_json(
                    "Amount",
                    "currency",
                    json!({"nillable": true, "precision": 18, "scale": 2}),
                ),
                field_json("IsWon", "boolean", json!({"nillable": true})),
                field_json("CloseDate", "datetime", json!({"nillable": true})),
            ],
        );
        serde_json::from_value(value).expect("fixture deserializes")
    }

    #[test]
    fn iceberg_schema_has_expected_field_ids_and_types() {
        let schema = iceberg_schema_from_describe(&opportunity_describe()).expect("iceberg schema");
        // Id-first ordering: Id has id 1.
        let id_field = schema.field_by_name("Id").expect("Id field");
        assert_eq!(id_field.id, 1);
        assert!(id_field.required);

        let amount = schema.field_by_name("Amount").expect("Amount field");
        assert_eq!(
            format!("{}", amount.field_type),
            "decimal(18,2)",
            "currency with precision/scale maps to decimal"
        );
    }

    #[test]
    fn arrow_schema_field_count_matches() {
        let mapped = map_schema(&opportunity_describe()).expect("mapped schema");
        assert_eq!(mapped.arrow().fields().len(), 5);
    }

    #[test]
    fn arrow_schema_maps_scalar_types() {
        let mapped = map_schema(&opportunity_describe()).expect("mapped schema");
        let arrow = mapped.arrow();

        let name = arrow.field_with_name("Name").expect("Name");
        assert_eq!(name.data_type(), &DataType::Utf8);
        assert!(name.is_nullable());

        let id = arrow.field_with_name("Id").expect("Id");
        assert_eq!(id.data_type(), &DataType::Utf8);
        assert!(!id.is_nullable());

        let amount = arrow.field_with_name("Amount").expect("Amount");
        assert_eq!(amount.data_type(), &DataType::Decimal128(18, 2));

        let is_won = arrow.field_with_name("IsWon").expect("IsWon");
        assert_eq!(is_won.data_type(), &DataType::Boolean);

        let close = arrow.field_with_name("CloseDate").expect("CloseDate");
        assert!(matches!(close.data_type(), DataType::Timestamp(_, _)));
    }
}
