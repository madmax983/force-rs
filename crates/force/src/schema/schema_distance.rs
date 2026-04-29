//! Schema Similarity Distance Calculator.
//!
//! Calculates a simple similarity score (Jaccard-like distance) between two Salesforce objects
//! based on their field names, field types, and relationships.
//! This allows developers to find "similar" objects (e.g., "Invoice__c" is 90% similar to "Order").
//!
//! # Example
//!
//! ```no_run
//! # use force::api::RestOperation;
//! # use force::client::ForceClientBuilder;
//! # use force::schema::calculate_schema_similarity;
//! # use force::auth::ClientCredentials;
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! # let auth = ClientCredentials::new("id", "secret", "url");
//! # let client = ForceClientBuilder::new().authenticate(auth).build().await?;
//! let describe_a = client.rest().describe("Account").await?;
//! let describe_b = client.rest().describe("Contact").await?;
//!
//! let similarity = calculate_schema_similarity(&describe_a, &describe_b);
//!
//! println!("Similarity Score: {:.2}", similarity);
//! # Ok(())
//! # }
//! ```

use crate::types::describe::SObjectDescribe;
use std::collections::HashSet;

/// Calculates a similarity score between 0.0 (completely different) and 1.0 (identical structure).
#[must_use]
pub fn calculate_schema_similarity(a: &SObjectDescribe, b: &SObjectDescribe) -> f64 {
    let fields_a: HashSet<String> = a
        .fields
        .iter()
        .map(|f| format!("{}:{:?}", f.name.to_lowercase(), f.type_))
        .collect();

    let fields_b: HashSet<String> = b
        .fields
        .iter()
        .map(|f| format!("{}:{:?}", f.name.to_lowercase(), f.type_))
        .collect();

    let intersection_count = fields_a.intersection(&fields_b).count();
    let union_count = fields_a.union(&fields_b).count();

    if union_count == 0 {
        return 1.0; // Two empty schemas are identical
    }

    intersection_count as f64 / union_count as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::describe::{FieldDescribe, FieldType};
    use std::collections::HashMap;

    fn create_mock_describe(name: &str, fields: Vec<(&str, FieldType)>) -> SObjectDescribe {
        SObjectDescribe {
            activateable: false,
            createable: true,
            custom: false,
            custom_setting: false,
            deletable: true,
            deprecated_and_hidden: false,
            feed_enabled: false,
            has_subtypes: false,
            is_subtype: false,
            key_prefix: Some("000".to_string()),
            label: name.to_string(),
            label_plural: format!("{}s", name),
            layoutable: true,
            mergeable: true,
            mru_enabled: true,
            name: name.to_string(),
            queryable: true,
            replicateable: true,
            retrieveable: true,
            searchable: true,
            triggerable: true,
            undeletable: true,
            updateable: true,
            urls: HashMap::new(),
            child_relationships: vec![],
            record_type_infos: vec![],
            fields: fields
                .into_iter()
                .map(|(n, t)| FieldDescribe {
                    aggregatable: true,
                    auto_number: false,
                    byte_length: 18,
                    calculated: false,
                    calculated_formula: None,
                    cascade_delete: false,
                    case_sensitive: false,
                    compound_field_name: None,
                    controller_name: None,
                    createable: true,
                    custom: false,
                    default_value: None,
                    default_value_formula: None,
                    defaulted_on_create: false,
                    dependent_picklist: false,
                    deprecated_and_hidden: false,
                    digits: 0,
                    display_location_in_decimal: false,
                    encrypted: false,
                    external_id: false,
                    extra_type_info: None,
                    filterable: true,
                    filtered_lookup_info: None,
                    formula_treat_blanks_as: None,
                    groupable: true,
                    high_scale_number: false,
                    html_formatted: false,
                    id_lookup: false,
                    inline_help_text: None,
                    label: n.to_string(),
                    length: 255,
                    mask: None,
                    mask_type: None,
                    name: n.to_string(),
                    name_field: false,
                    name_pointing: false,
                    nillable: true,
                    permissionable: false,
                    picklist_values: None,
                    polymorphic_foreign_key: false,
                    precision: 0,
                    query_by_distance: false,
                    reference_target_field: None,
                    reference_to: vec![],
                    relationship_name: None,
                    relationship_order: None,
                    restricted_delete: false,
                    restricted_picklist: false,
                    scale: 0,
                    search_prefixes_supported: None,
                    soap_type: "xsd:string".to_string(),
                    sortable: true,
                    type_: t,
                    unique: false,
                    updateable: true,
                    write_requires_master_read: false,
                })
                .collect(),
        }
    }

    #[test]
    fn test_identical_schemas() {
        let a = create_mock_describe(
            "ObjectA",
            vec![("Id", FieldType::Id), ("Name", FieldType::String)],
        );
        let b = create_mock_describe(
            "ObjectB",
            vec![("Id", FieldType::Id), ("Name", FieldType::String)],
        );

        assert!((calculate_schema_similarity(&a, &b) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_completely_different_schemas() {
        let a = create_mock_describe(
            "ObjectA",
            vec![("Id", FieldType::Id), ("Name", FieldType::String)],
        );
        let b = create_mock_describe(
            "ObjectB",
            vec![
                ("CreatedDate", FieldType::Datetime),
                ("OwnerId", FieldType::Reference),
            ],
        );

        assert!((calculate_schema_similarity(&a, &b) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_partially_similar_schemas() {
        let a = create_mock_describe(
            "ObjectA",
            vec![
                ("Id", FieldType::Id),
                ("Name", FieldType::String),
                ("Amount", FieldType::Currency),
            ],
        );
        let b = create_mock_describe(
            "ObjectB",
            vec![
                ("Id", FieldType::Id),
                ("Name", FieldType::String),
                ("Status", FieldType::Picklist),
            ],
        );

        // Intersection: Id, Name (2)
        // Union: Id, Name, Amount, Status (4)
        // Similarity: 2 / 4 = 0.5
        assert!((calculate_schema_similarity(&a, &b) - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_empty_schemas() {
        let a = create_mock_describe("ObjectA", vec![]);
        let b = create_mock_describe("ObjectB", vec![]);

        assert!((calculate_schema_similarity(&a, &b) - 1.0).abs() < f64::EPSILON);
    }
}
