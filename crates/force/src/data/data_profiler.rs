//! Data Profiler for Salesforce Records.
//!
//! This module provides the `DataProfiler` utility to analyze datasets
//! of `DynamicSObject`s. It calculates statistics like null rates,
//! distinct values, and max lengths for each field, offering insights
//! into actual data distributions locally.

use crate::types::DynamicSObject;
use std::collections::{HashMap, HashSet};

/// Profile statistics for a single field across a dataset.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FieldProfile {
    /// Number of non-null values.
    pub populated_count: usize,
    /// Number of null values.
    pub null_count: usize,
    /// Number of unique string representations of the values.
    pub distinct_values_count: usize,
    /// Maximum string length of the field's values (0 if all null or not string-like).
    pub max_length: usize,
}

/// The complete profile of a dataset.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DataProfile {
    /// Total number of records analyzed.
    pub total_records: usize,
    /// Field-level statistics.
    pub field_profiles: HashMap<String, FieldProfile>,
}

/// Utility for profiling `DynamicSObject` datasets.
#[derive(Debug, Default)]
pub struct DataProfiler;

impl DataProfiler {
    /// Creates a new `DataProfiler`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Analyzes a slice of `DynamicSObject` records and returns a `DataProfile`.
    #[must_use]
    pub fn profile(&self, records: &[DynamicSObject]) -> DataProfile {
        let mut profile = DataProfile {
            total_records: records.len(),
            field_profiles: HashMap::new(),
        };

        // Temporary storage for unique values to calculate distinct counts
        let mut distinct_tracker: HashMap<String, HashSet<String>> = HashMap::new();

        for record in records {
            for (key, val) in &record.fields {
                let entry = profile.field_profiles.entry(key.clone()).or_default();
                let tracker = distinct_tracker.entry(key.clone()).or_default();

                if val.is_null() {
                    entry.null_count += 1;
                } else {
                    entry.populated_count += 1;

                    // Convert value to string for distinct and length tracking
                    let val_str = match val {
                        serde_json::Value::String(s) => s.clone(),
                        _ => val.to_string(),
                    };

                    tracker.insert(val_str.clone());

                    if val_str.len() > entry.max_length {
                        entry.max_length = val_str.len();
                    }
                }
            }
        }

        // Finalize distinct counts
        for (key, tracker) in distinct_tracker {
            if let Some(field_profile) = profile.field_profiles.get_mut(&key) {
                field_profile.distinct_values_count = tracker.len();
            }
        }

        profile
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    #![allow(clippy::panic)]
    use super::*;
    use crate::types::sobject::{Attributes, DynamicSObject};
    use serde_json::json;

    fn create_record(fields: serde_json::Value) -> DynamicSObject {
        let serde_json::Value::Object(fields_map) = fields else {
            panic!("Expected object");
        };
        DynamicSObject {
            attributes: Attributes {
                type_: "Account".to_string(),
                url: "/services/data/v67.0/sobjects/Account/123".to_string(),
            },
            fields: fields_map,
        }
    }

    #[test]
    fn test_profiling_empty() {
        let profiler = DataProfiler::new();
        let profile = profiler.profile(&[]);
        assert_eq!(profile.total_records, 0);
        assert!(profile.field_profiles.is_empty());
    }

    #[test]
    fn test_profiling_records() {
        let profiler = DataProfiler::new();
        let records = vec![
            create_record(json!({"Name": "Acme", "Employees": 100, "Industry": null})),
            create_record(json!({"Name": "Acme", "Employees": 200, "Industry": "Tech"})),
            create_record(json!({"Name": "Beta", "Employees": null, "Industry": "Retail"})),
        ];

        let profile = profiler.profile(&records);

        assert_eq!(profile.total_records, 3);
        assert_eq!(profile.field_profiles.len(), 3);

        let name_prof = profile.field_profiles.get("Name").unwrap();
        assert_eq!(name_prof.populated_count, 3);
        assert_eq!(name_prof.null_count, 0);
        assert_eq!(name_prof.distinct_values_count, 2); // "Acme", "Beta"
        assert_eq!(name_prof.max_length, 4); // "Acme" and "Beta" are length 4

        let emp_prof = profile.field_profiles.get("Employees").unwrap();
        assert_eq!(emp_prof.populated_count, 2);
        assert_eq!(emp_prof.null_count, 1);
        assert_eq!(emp_prof.distinct_values_count, 2); // 100, 200
        assert_eq!(emp_prof.max_length, 3); // "100" is length 3

        let ind_prof = profile.field_profiles.get("Industry").unwrap();
        assert_eq!(ind_prof.populated_count, 2);
        assert_eq!(ind_prof.null_count, 1);
        assert_eq!(ind_prof.distinct_values_count, 2); // "Tech", "Retail"
        assert_eq!(ind_prof.max_length, 6); // "Retail" is length 6
    }
}
