//! Data Profiler for Salesforce Records.
//!
//! This module provides the `DataProfiler` utility to analyze a dataset of
//! `DynamicSObject` records against an `SObjectDescribe` schema. It generates
//! statistics such as fill rates, uniqueness, and min/max values.
//!
//! # The Spark
//! We have `DataValidator` to check if a single record is valid, and `DataMasker` to sanitize it.
//! But what if developers need to understand the shape and quality of a whole dataset?
//! `DataProfiler` analyzes data distributions, finding heavily unused fields or
//! identifying potential data anomalies!

use crate::types::DynamicSObject;
use crate::types::describe::{FieldType, SObjectDescribe};
use std::collections::{HashMap, HashSet};

/// Statistics for a specific field across a dataset.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldProfile {
    /// Total number of records analyzed.
    pub total_records: usize,
    /// Number of records where this field is populated (non-null).
    pub populated_count: usize,
    /// Number of unique values observed.
    pub unique_values: usize,
    /// Minimum string length (if applicable).
    pub min_length: Option<usize>,
    /// Maximum string length (if applicable).
    pub max_length: Option<usize>,
}

impl FieldProfile {
    /// The percentage of records that have a value for this field.
    #[must_use]
    pub fn fill_rate(&self) -> f64 {
        if self.total_records == 0 {
            0.0
        } else {
            (self.populated_count as f64 / self.total_records as f64) * 100.0
        }
    }
}

/// A profiler report containing statistics for all analyzed fields.
#[derive(Debug, Clone)]
pub struct ProfilerReport {
    /// The name of the SObject profiled.
    pub object_name: String,
    /// Total number of records analyzed.
    pub total_records: usize,
    /// Field-level statistics.
    pub field_stats: HashMap<String, FieldProfile>,
}

/// Utility for profiling mock or actual data against its schema.
#[derive(Debug)]
pub struct DataProfiler<'a> {
    describe: &'a SObjectDescribe,
}

impl<'a> DataProfiler<'a> {
    /// Creates a new data profiler.
    #[must_use]
    pub fn new(describe: &'a SObjectDescribe) -> Self {
        Self { describe }
    }

    /// Analyzes a dataset of records and returns a profiling report.
    #[must_use]
    pub fn profile(&self, records: &[DynamicSObject]) -> ProfilerReport {
        let mut field_stats = HashMap::new();
        let total_records = records.len();

        for field in &self.describe.fields {
            field_stats.insert(
                field.name.clone(),
                FieldProfile {
                    total_records,
                    populated_count: 0,
                    unique_values: 0,
                    min_length: None,
                    max_length: None,
                },
            );
        }

        let mut unique_trackers: HashMap<String, HashSet<String>> = HashMap::new();

        for record in records {
            for field in &self.describe.fields {
                let Some(val) = record.get_field(&field.name) else {
                    continue;
                };

                if val.is_null() {
                    continue;
                }

                if let Some(stat) = field_stats.get_mut(&field.name) {
                    stat.populated_count += 1;

                    let val_str = val.to_string();
                    unique_trackers
                        .entry(field.name.clone())
                        .or_default()
                        .insert(val_str.clone());

                    if field.type_ == FieldType::String
                        || field.type_ == FieldType::Textarea
                        || field.type_ == FieldType::Email
                        || field.type_ == FieldType::Phone
                        || field.type_ == FieldType::Url
                        || field.type_ == FieldType::Id
                        || field.type_ == FieldType::Reference
                    {
                        let len = val.as_str().map_or_else(|| val_str.len(), |s| s.len());
                        if let Some(min) = stat.min_length {
                            stat.min_length = Some(std::cmp::min(min, len));
                        } else {
                            stat.min_length = Some(len);
                        }

                        if let Some(max) = stat.max_length {
                            stat.max_length = Some(std::cmp::max(max, len));
                        } else {
                            stat.max_length = Some(len);
                        }
                    }
                }
            }
        }

        for (field_name, stat) in &mut field_stats {
            if let Some(tracker) = unique_trackers.get(field_name) {
                stat.unique_values = tracker.len();
            }
        }

        ProfilerReport {
            object_name: self.describe.name.clone(),
            total_records,
            field_stats,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;
    use crate::types::{Attributes, SalesforceId};
    use serde_json::{Value, json};

    fn create_mock_describe(fields_json: &serde_json::Value) -> SObjectDescribe {
        let describe_json = json!({
            "name": "Contact",
            "label": "Contact",
            "custom": false,
            "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "003", "labelPlural": "Contacts", "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": fields_json.clone()
        });
        serde_json::from_value(describe_json).must()
    }

    fn mock_field(name: &str, field_type: &str) -> serde_json::Value {
        json!({
            "name": name,
            "type": field_type,
            "label": format!("{} Label", name),
            "referenceTo": [],
            "encrypted": false,
            "createable": true, "autoNumber": false, "calculated": false,
            "aggregatable": true, "byteLength": 255, "cascadeDelete": false,
            "caseSensitive": false, "custom": false, "defaultedOnCreate": false,
            "dependentPicklist": false, "deprecatedAndHidden": false, "digits": 0,
            "displayLocationInDecimal": false, "externalId": false, "filterable": true,
            "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": false, "length": 255, "nameField": false, "namePointing": false,
            "nillable": true, "permissionable": false, "polymorphicForeignKey": false,
            "precision": 0, "queryByDistance": false, "restrictedDelete": false,
            "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
            "sortable": true, "unique": false, "updateable": true,
            "writeRequiresMasterRead": false
        })
    }

    fn create_mock_record(fields: serde_json::Map<String, Value>) -> DynamicSObject {
        let id = SalesforceId::new("003000000000001AAA").must();
        let attrs = Attributes::new("Contact", &id, "v67.0");
        let mut record = DynamicSObject::new(attrs);
        record.fields = fields;
        record
    }

    #[test]
    fn test_data_profiler() {
        let describe = create_mock_describe(&json!([
            mock_field("FirstName", "string"),
            mock_field("Age__c", "int")
        ]));

        let mut r1_fields = serde_json::Map::new();
        r1_fields.insert("FirstName".to_string(), json!("Alice"));
        r1_fields.insert("Age__c".to_string(), json!(30));
        let r1 = create_mock_record(r1_fields);

        let mut r2_fields = serde_json::Map::new();
        r2_fields.insert("FirstName".to_string(), json!("Bob"));
        r2_fields.insert("Age__c".to_string(), json!(null)); // Bob has no age
        let r2 = create_mock_record(r2_fields);

        let mut r3_fields = serde_json::Map::new();
        r3_fields.insert("FirstName".to_string(), json!("Alice")); // Duplicate name
        r3_fields.insert("Age__c".to_string(), json!(25));
        let r3 = create_mock_record(r3_fields);

        let profiler = DataProfiler::new(&describe);
        let report = profiler.profile(&[r1, r2, r3]);

        assert_eq!(report.total_records, 3);

        let first_name_profile = report.field_stats.get("FirstName").must();
        assert_eq!(first_name_profile.total_records, 3);
        assert_eq!(first_name_profile.populated_count, 3);
        assert_eq!(first_name_profile.unique_values, 2); // "Alice" and "Bob"
        assert_eq!(first_name_profile.min_length, Some(3)); // "Bob"
        assert_eq!(first_name_profile.max_length, Some(5)); // "Alice"
        assert!((first_name_profile.fill_rate() - 100.0).abs() < 0.01);

        let age_profile = report.field_stats.get("Age__c").must();
        assert_eq!(age_profile.total_records, 3);
        assert_eq!(age_profile.populated_count, 2);
        assert_eq!(age_profile.unique_values, 2);
        assert_eq!(age_profile.min_length, None);
        assert_eq!(age_profile.max_length, None);
        assert!((age_profile.fill_rate() - 66.66).abs() < 0.01);
    }
}
