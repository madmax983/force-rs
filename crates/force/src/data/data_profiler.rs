//! Data Profiler for Salesforce Records.
//!
//! Provides `DataProfiler`, a utility to analyze data quality and distribution
//! of Salesforce records locally.

use crate::types::DynamicSObject;
use crate::types::describe::SObjectDescribe;
use std::collections::HashMap;

/// Profiling statistics for a single field across a dataset.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldProfile {
    /// Name of the field
    pub name: String,
    /// Total records
    pub total_records: usize,
    /// Populated count
    pub populated_count: usize,
    /// Null count
    pub null_count: usize,
    /// Null percentage
    pub null_percentage: f64,
    /// Unique values count
    pub unique_values_count: usize,
}

/// A comprehensive data profile report for an SObject dataset.
#[derive(Debug, Clone, PartialEq)]
pub struct DataProfileReport {
    /// SObject name
    pub sobject_name: String,
    /// Total records
    pub total_records: usize,
    /// Field profiles
    pub field_profiles: HashMap<String, FieldProfile>,
}

/// Profiles a dataset of `DynamicSObject` records against their schema.
pub struct DataProfiler;

impl DataProfiler {
    /// Generates a data profile report for a slice of records.
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    #[allow(clippy::unwrap_used)]
    pub fn profile(describe: &SObjectDescribe, records: &[DynamicSObject]) -> DataProfileReport {
        let mut field_profiles = HashMap::new();
        let total_records = records.len();

        for field in &describe.fields {
            field_profiles.insert(
                field.name.clone(),
                FieldProfile {
                    name: field.name.clone(),
                    total_records,
                    populated_count: 0,
                    null_count: 0,
                    null_percentage: 0.0,
                    unique_values_count: 0,
                },
            );
        }

        let mut unique_values: HashMap<String, std::collections::HashSet<String>> = HashMap::new();
        for field in &describe.fields {
            unique_values.insert(field.name.clone(), std::collections::HashSet::new());
        }

        for record in records {
            for field in &describe.fields {
                let profile = field_profiles.get_mut(&field.name).unwrap();

                if let Some(val) = record.fields.get(&field.name) {
                    if val.is_null() {
                        profile.null_count += 1;
                    } else {
                        profile.populated_count += 1;
                        unique_values
                            .get_mut(&field.name)
                            .unwrap()
                            .insert(val.to_string());
                    }
                } else {
                    profile.null_count += 1;
                }
            }
        }

        for profile in field_profiles.values_mut() {
            if total_records > 0 {
                profile.null_percentage =
                    (profile.null_count as f64 / total_records as f64) * 100.0;
            }
            profile.unique_values_count = unique_values.get(&profile.name).unwrap().len();
        }

        DataProfileReport {
            sobject_name: describe.name.clone(),
            total_records,
            field_profiles,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::mock_describe::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};
    use crate::types::describe::FieldType;
    use crate::types::{Attributes, DynamicSObject};
    use serde_json::{Map, Value};

    #[test]
    fn test_data_profiler_calculates_correct_stats() {
        let describe = MockSObjectDescribeBuilder::new("Account")
            .field(MockFieldDescribeBuilder::new("Name", FieldType::String).build())
            .field(MockFieldDescribeBuilder::new("Industry", FieldType::String).build())
            .build();

        #[allow(clippy::similar_names)]
        let mut record1 = DynamicSObject {
            attributes: Attributes {
                type_: "Account".to_string(),
                url: String::new(),
            },
            fields: Map::new(),
        };
        record1
            .fields
            .insert("Name".to_string(), Value::String("Acme".to_string()));
        record1.fields.insert("Industry".to_string(), Value::Null);

        let mut record2 = DynamicSObject {
            attributes: Attributes {
                type_: "Account".to_string(),
                url: String::new(),
            },
            fields: Map::new(),
        };
        record2
            .fields
            .insert("Name".to_string(), Value::String("Acme".to_string()));
        record2
            .fields
            .insert("Industry".to_string(), Value::String("Tech".to_string()));

        let mut record3 = DynamicSObject {
            attributes: Attributes {
                type_: "Account".to_string(),
                url: String::new(),
            },
            fields: Map::new(),
        };
        // Name is completely missing from map
        record3
            .fields
            .insert("Industry".to_string(), Value::String("Finance".to_string()));

        #[allow(clippy::similar_names)]
        let dataset = vec![record1, record2, record3];

        let report = DataProfiler::profile(&describe, &dataset);

        assert_eq!(report.sobject_name, "Account");
        assert_eq!(report.total_records, 3);

        #[allow(clippy::unwrap_used)]
        let name_profile = report.field_profiles.get("Name").unwrap();
        assert_eq!(name_profile.populated_count, 2);
        assert_eq!(name_profile.null_count, 1);
        assert_eq!(name_profile.unique_values_count, 1); // "Acme" appears twice

        #[allow(clippy::unwrap_used)]
        let industry_profile = report.field_profiles.get("Industry").unwrap();
        assert_eq!(industry_profile.populated_count, 2);
        assert_eq!(industry_profile.null_count, 1);
        assert_eq!(industry_profile.unique_values_count, 2); // "Tech", "Finance"
    }
}
