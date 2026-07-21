//! Data Profiler for Salesforce Records.
//!
//! This module provides the `DataProfiler` utility to analyze a dataset of `DynamicSObject`
//! records, generating insights into data quality, fill rates, and uniqueness.

use crate::types::DynamicSObject;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Profile statistics for a single field across a dataset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FieldProfile {
    /// Number of records where this field is null or missing.
    pub null_count: usize,
    /// Number of records where this field is populated.
    pub populated_count: usize,
    /// Number of distinct values for this field (for populated values).
    pub distinct_count: usize,
    /// Percentage of records where this field is populated (0.0 to 100.0).
    pub fill_rate: f64,
}

/// The result of profiling a dataset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DataProfileReport {
    /// Total number of records analyzed.
    pub total_records: usize,
    /// Profile statistics per field.
    pub fields: HashMap<String, FieldProfile>,
}

/// Utility for profiling Salesforce data to determine data quality.
#[derive(Debug, Default)]
pub struct DataProfiler {
    total_records: usize,
    field_stats: HashMap<String, FieldStatsTracker>,
}

#[derive(Debug, Default)]
struct FieldStatsTracker {
    populated_count: usize,
    distinct_values: HashSet<String>,
}

impl DataProfiler {
    /// Creates a new, empty DataProfiler.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a single record to the profile.
    pub fn add_record(&mut self, record: &DynamicSObject) {
        self.total_records += 1;

        for (field_name, value) in &record.fields {
            let tracker = self.field_stats.entry(field_name.clone()).or_default();

            if !value.is_null() {
                tracker.populated_count += 1;
                tracker.distinct_values.insert(value.to_string());
            }
        }
    }

    /// Adds multiple records to the profile.
    pub fn add_records<'a, I>(&mut self, records: I)
    where
        I: IntoIterator<Item = &'a DynamicSObject>,
    {
        for record in records {
            self.add_record(record);
        }
    }

    /// Generates the final DataProfileReport.
    #[must_use]
    pub fn generate_report(self) -> DataProfileReport {
        let mut fields = HashMap::new();
        let total = self.total_records;

        for (field_name, tracker) in self.field_stats {
            let null_count = total.saturating_sub(tracker.populated_count);
            let fill_rate = if total > 0 {
                (tracker.populated_count as f64 / total as f64) * 100.0
            } else {
                0.0
            };

            fields.insert(
                field_name,
                FieldProfile {
                    null_count,
                    populated_count: tracker.populated_count,
                    distinct_count: tracker.distinct_values.len(),
                    fill_rate,
                },
            );
        }

        DataProfileReport {
            total_records: total,
            fields,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Attributes, SalesforceId};
    use serde_json::json;

    fn create_record(id: &str, fields_val: serde_json::Value) -> DynamicSObject {
        let Ok(sf_id) = SalesforceId::new(id) else {
            panic!("Invalid ID")
        };
        let attrs = Attributes::new("Account", &sf_id, "v67.0");
        let mut obj = DynamicSObject::new(attrs);
        if let serde_json::Value::Object(map) = fields_val {
            for (k, v) in map {
                obj.fields.insert(k, v);
            }
        }
        obj
    }

    #[test]
    fn test_data_profiler() {
        let mut profiler = DataProfiler::new();

        let rec1 = create_record(
            "001000000000001AAA",
            json!({
                "Name": "Acme",
                "Industry": "Tech",
                "Employees": 100
            }),
        );

        let rec2 = create_record(
            "001000000000002AAA",
            json!({
                "Name": "Globex",
                "Industry": "Tech",
                "Employees": null
            }),
        );

        let rec3 = create_record(
            "001000000000003AAA",
            json!({
                "Name": "Initech",
                "Industry": null
            }),
        );

        profiler.add_records([&rec1, &rec2, &rec3]);

        let report = profiler.generate_report();

        assert_eq!(report.total_records, 3);

        let Some(name_prof) = report.fields.get("Name") else {
            panic!("Missing field Name")
        };
        assert_eq!(name_prof.populated_count, 3);
        assert_eq!(name_prof.null_count, 0);
        assert_eq!(name_prof.distinct_count, 3);
        assert!((name_prof.fill_rate - 100.0).abs() < f64::EPSILON);

        let Some(ind_prof) = report.fields.get("Industry") else {
            panic!("Missing field Industry")
        };
        assert_eq!(ind_prof.populated_count, 2);
        assert_eq!(ind_prof.null_count, 1);
        assert_eq!(ind_prof.distinct_count, 1);
        assert!((ind_prof.fill_rate - 66.666).abs() < 0.1);

        let Some(emp_prof) = report.fields.get("Employees") else {
            panic!("Missing field Employees")
        };
        assert_eq!(emp_prof.populated_count, 1);
        assert_eq!(emp_prof.null_count, 2);
        assert_eq!(emp_prof.distinct_count, 1);
        assert!((emp_prof.fill_rate - 33.333).abs() < 0.1);
    }
}
