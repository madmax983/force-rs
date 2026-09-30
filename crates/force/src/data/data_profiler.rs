//! Data Profiler for Salesforce Records.
//!
//! This module provides the `DataProfiler` utility to analyze a dataset of `DynamicSObject`
//! records against an `SObjectDescribe` metadata payload to generate data quality metrics.
//!
//! # The Spark
//! We have `DataFaker` to generate data and `DataValidator` to check constraints, but we lack
//! a way to summarize the overall health and shape of a dataset (e.g., null counts, field lengths).
//! `DataProfiler` bridges this gap!
//!
//! # Example
//!
//! ```no_run
//! # use force::api::RestOperation;
//! # use force::client::ForceClientBuilder;
//! # use force::data::DataProfiler;
//! # use force::auth::ClientCredentials;
//! # use force::types::DynamicSObject;
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! # let auth = ClientCredentials::new("id", "secret", "url");
//! # let client = ForceClientBuilder::new().authenticate(auth).build().await?;
//! let describe = client.rest().describe("Contact").await?;
//!
//! let records: Vec<DynamicSObject> = vec![]; // Fetch or generate records
//!
//! let profiler = DataProfiler::new(&describe);
//! let report = profiler.profile(&records);
//!
//! println!("Email null percentage: {}%", report.get_null_percentage("Email").unwrap_or(0.0));
//! # Ok(())
//! # }
//! ```

use crate::types::DynamicSObject;
use crate::types::describe::SObjectDescribe;
use std::collections::HashMap;

/// A profile report for a single field across a dataset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldProfile {
    /// The API name of the field.
    pub field_name: String,
    /// The total number of records analyzed.
    pub total_records: usize,
    /// The number of records where this field was null or missing.
    pub null_count: usize,
    /// The number of records where this field was present and non-null.
    pub non_null_count: usize,
    /// The minimum string length of the field's value (if applicable/present).
    pub min_length: Option<usize>,
    /// The maximum string length of the field's value (if applicable/present).
    pub max_length: Option<usize>,
}

impl FieldProfile {
    /// Calculates the percentage of records where this field is null.
    #[must_use]
    pub fn null_percentage(&self) -> f64 {
        if self.total_records == 0 {
            return 0.0;
        }
        (self.null_count as f64 / self.total_records as f64) * 100.0
    }
}

/// A complete profile report for a dataset of SObject records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataProfileReport {
    /// The API name of the SObject.
    pub object_name: String,
    /// The total number of records analyzed.
    pub total_records: usize,
    /// The field profiles, keyed by field API name.
    pub field_profiles: HashMap<String, FieldProfile>,
}

impl DataProfileReport {
    /// Returns the null percentage for a specific field, or None if the field doesn't exist in the report.
    #[must_use]
    pub fn get_null_percentage(&self, field_name: &str) -> Option<f64> {
        self.field_profiles
            .get(field_name)
            .map(FieldProfile::null_percentage)
    }
}

/// Utility for profiling data quality of SObject datasets.
#[derive(Debug, Clone)]
pub struct DataProfiler<'a> {
    describe: &'a SObjectDescribe,
}

impl<'a> DataProfiler<'a> {
    /// Creates a new `DataProfiler` for the given schema describe.
    #[must_use]
    pub fn new(describe: &'a SObjectDescribe) -> Self {
        Self { describe }
    }

    /// Analyzes a slice of records and returns a `DataProfileReport`.
    #[must_use]
    pub fn profile(&self, records: &[DynamicSObject]) -> DataProfileReport {
        let mut report = DataProfileReport {
            object_name: self.describe.name.clone(),
            total_records: records.len(),
            field_profiles: HashMap::new(),
        };

        // Initialize profiles for all fields in the describe
        for field in &self.describe.fields {
            report.field_profiles.insert(
                field.name.clone(),
                FieldProfile {
                    field_name: field.name.clone(),
                    total_records: records.len(),
                    null_count: 0,
                    non_null_count: 0,
                    min_length: None,
                    max_length: None,
                },
            );
        }

        // Implement the logic to pass the test
        for record in records {
            for (field_name, profile) in &mut report.field_profiles {
                if let Some(val) = record.fields.get(field_name) {
                    if val.is_null() {
                        profile.null_count += 1;
                    } else {
                        profile.non_null_count += 1;
                        if let Some(s) = val.as_str() {
                            let len = s.len();
                            profile.min_length = Some(profile.min_length.unwrap_or(len).min(len));
                            profile.max_length = Some(profile.max_length.unwrap_or(len).max(len));
                        }
                    }
                } else {
                    profile.null_count += 1;
                }
            }
        }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::describe::{FieldDescribe, FieldType};
    use crate::types::salesforce_id::SalesforceId;
    use crate::types::sobject::Attributes;
    use serde_json::Value;

    fn mock_field(name: &str, type_: FieldType) -> FieldDescribe {
        FieldDescribe {
            name: name.to_string(),
            label: name.to_string(),
            type_,
            aggregatable: true,
            auto_number: false,
            byte_length: 0,
            calculated: false,
            calculated_formula: None,
            cascade_delete: false,
            case_sensitive: false,
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
            groupable: true,
            high_scale_number: false,
            html_formatted: false,
            id_lookup: false,
            inline_help_text: None,
            length: 255,
            mask: None,
            mask_type: None,
            name_field: false,
            name_pointing: false,
            nillable: true,
            permissionable: true,
            picklist_values: Some(vec![]),
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

            soap_type: "xsd:string".to_string(),
            sortable: true,
            unique: false,
            updateable: true,
            write_requires_master_read: false,
            compound_field_name: None,
            formula_treat_blanks_as: None,
            search_prefixes_supported: Some(false),
        }
    }

    fn create_mock_describe() -> SObjectDescribe {
        SObjectDescribe {
            name: "Contact".to_string(),
            label: "Contact".to_string(),
            label_plural: "Contacts".to_string(),
            key_prefix: Some("003".to_string()),
            custom: false,
            custom_setting: false,
            createable: true,
            deletable: true,
            updateable: true,
            queryable: true,
            retrieveable: true,
            searchable: true,
            activateable: false,
            deprecated_and_hidden: false,
            feed_enabled: false,
            has_subtypes: false,
            is_subtype: false,
            layoutable: true,
            mergeable: true,
            mru_enabled: true,
            replicateable: true,
            triggerable: true,
            undeletable: true,
            fields: vec![
                mock_field("Id", FieldType::Id),
                mock_field("Email", FieldType::Email),
                mock_field("Name", FieldType::String),
            ],
            urls: std::collections::HashMap::new(),
            child_relationships: vec![],
            record_type_infos: vec![],
        }
    }

    #[test]
    fn test_data_profiler_red_phase() {
        let describe = create_mock_describe();

        let id1 = SalesforceId::new("003000000000001AAA").unwrap_or_else(|_| unreachable!());
        let mut rec1 = DynamicSObject::new(Attributes::new("Contact".to_string(), &id1, "v67.0"));
        rec1.fields.insert(
            "Email".to_string(),
            Value::String("test@example.com".to_string()),
        );
        rec1.fields
            .insert("Name".to_string(), Value::String("Alice".to_string()));

        let id2 = SalesforceId::new("003000000000002AAA").unwrap_or_else(|_| unreachable!());
        let mut rec2 = DynamicSObject::new(Attributes::new("Contact".to_string(), &id2, "v67.0"));
        rec2.fields.insert("Email".to_string(), Value::Null);
        rec2.fields
            .insert("Name".to_string(), Value::String("Bob".to_string()));

        let records = vec![rec1, rec2];

        let profiler = DataProfiler::new(&describe);
        let report = profiler.profile(&records);

        assert_eq!(report.total_records, 2);

        // Check Email field profile
        let email_profile = report
            .field_profiles
            .get("Email")
            .unwrap_or_else(|| unreachable!());
        assert_eq!(email_profile.null_count, 1);
        assert_eq!(email_profile.non_null_count, 1);
        assert_eq!(email_profile.min_length, Some(16)); // "test@example.com".len()
        assert_eq!(email_profile.max_length, Some(16));

        // Check Name field profile
        let name_profile = report
            .field_profiles
            .get("Name")
            .unwrap_or_else(|| unreachable!());
        assert_eq!(name_profile.null_count, 0);
        assert_eq!(name_profile.non_null_count, 2);
        assert_eq!(name_profile.min_length, Some(3)); // "Bob"
        assert_eq!(name_profile.max_length, Some(5)); // "Alice"

        assert!((report.get_null_percentage("Email").unwrap_or(0.0) - 50.0).abs() < f64::EPSILON);
    }
}
