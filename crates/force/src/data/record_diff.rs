//! Record Diff Utility.
//!
//! This module provides the `compare_records` utility to compare two `DynamicSObject`
//! records. It identifies added fields, removed fields, and fields whose values
//! have changed. This is particularly useful for Change Data Capture (CDC),
//! audit logging, or syncing data to external systems.
//!
//! # Example
//!
//! ```
//! # use force::data::compare_records;
//! # use force::types::{DynamicSObject, Attributes, SalesforceId};
//! # use force::test_utils::must::Must;
//! # let id = SalesforceId::new("001000000000001AAA").must();
//! # let mut old_record = DynamicSObject::new(Attributes::new("Account", &id, "v60.0"));
//! # old_record.set_field("Name", "Acme");
//! # old_record.set_field("Industry", "Retail");
//! # let mut new_record = DynamicSObject::new(Attributes::new("Account", &id, "v60.0"));
//! # new_record.set_field("Name", "Acme Corp"); // Changed
//! # new_record.set_field("Website", "example.com"); // Added
//! // Industry is missing in new_record, so it's considered removed.
//!
//! let diff = compare_records(&old_record, &new_record);
//!
//! assert_eq!(diff.changed.len(), 1);
//! assert_eq!(diff.added.len(), 1);
//! assert_eq!(diff.removed.len(), 1);
//! ```

use crate::types::DynamicSObject;
use serde_json::Value;

/// Represents a change in a field's value between two records.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldValueChange<'a> {
    /// The name of the field.
    pub field_name: &'a str,
    /// The old value of the field. `None` if the field was added.
    pub old_value: Option<&'a Value>,
    /// The new value of the field. `None` if the field was removed.
    pub new_value: Option<&'a Value>,
}

/// The result of comparing two `DynamicSObject` records.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RecordDiffResult<'a> {
    /// Fields that are present in the new record but not in the old record.
    pub added: Vec<FieldValueChange<'a>>,
    /// Fields that are present in the old record but not in the new record.
    pub removed: Vec<FieldValueChange<'a>>,
    /// Fields whose values have changed between the old and new records.
    pub changed: Vec<FieldValueChange<'a>>,
}

impl RecordDiffResult<'_> {
    /// Returns true if there are no differences between the records.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }

    /// Applies this diff to a target `DynamicSObject`, mutating it in place.
    pub fn apply_to(&self, target: &mut DynamicSObject) {
        for change in &self.removed {
            target.remove_field(change.field_name);
        }
        for change in &self.added {
            if let Some(val) = change.new_value {
                target
                    .fields
                    .insert(change.field_name.to_string(), val.clone());
            }
        }
        for change in &self.changed {
            if let Some(val) = change.new_value {
                target
                    .fields
                    .insert(change.field_name.to_string(), val.clone());
            }
        }
    }
}

/// Compares two `DynamicSObject` records and returns a `RecordDiffResult`.
///
/// # Arguments
///
/// * `old_record` - The original record state.
/// * `new_record` - The updated record state.
///
/// # Returns
///
/// A `RecordDiffResult` containing added, removed, and changed fields.
#[must_use]
pub fn compare_records<'a>(
    old_record: &'a DynamicSObject,
    new_record: &'a DynamicSObject,
) -> RecordDiffResult<'a> {
    let mut result = RecordDiffResult::default();

    for (key, old_val) in &old_record.fields {
        match new_record.fields.get(key) {
            Some(new_val) => {
                if old_val != new_val {
                    result.changed.push(FieldValueChange {
                        field_name: key,
                        old_value: Some(old_val),
                        new_value: Some(new_val),
                    });
                }
            }
            None => {
                result.removed.push(FieldValueChange {
                    field_name: key,
                    old_value: Some(old_val),
                    new_value: None,
                });
            }
        }
    }

    for (key, new_val) in &new_record.fields {
        if !old_record.fields.contains_key(key) {
            result.added.push(FieldValueChange {
                field_name: key,
                old_value: None,
                new_value: Some(new_val),
            });
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;
    use crate::types::{Attributes, SalesforceId};

    fn create_record() -> DynamicSObject {
        let id = SalesforceId::new("001000000000001AAA").must();
        DynamicSObject::new(Attributes::new("Account", &id, "v60.0"))
    }

    #[test]
    fn test_compare_identical_records() {
        let mut old_rec = create_record();
        old_rec.set_field("Name", "Acme");
        old_rec.set_field("Industry", "Retail");

        let mut new_rec = create_record();
        new_rec.set_field("Name", "Acme");
        new_rec.set_field("Industry", "Retail");

        let diff = compare_records(&old_rec, &new_rec);
        assert!(diff.is_empty());
    }

    #[test]
    fn test_compare_records_with_changes() {
        let mut old_rec = create_record();
        old_rec.set_field("Name", "Acme");
        old_rec.set_field("Industry", "Retail");
        old_rec.set_field("EmployeeCount", 100);

        let mut new_rec = create_record();
        new_rec.set_field("Name", "Acme Corp"); // Changed
        new_rec.set_field("Industry", "Retail"); // Unchanged
        new_rec.set_field("Website", "example.com"); // Added
        // EmployeeCount removed

        let diff = compare_records(&old_rec, &new_rec);
        assert!(!diff.is_empty());

        assert_eq!(diff.changed.len(), 1);
        assert_eq!(diff.changed[0].field_name, "Name");
        assert_eq!(diff.changed[0].old_value.must().as_str().must(), "Acme");
        assert_eq!(
            diff.changed[0].new_value.must().as_str().must(),
            "Acme Corp"
        );

        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.added[0].field_name, "Website");
        assert_eq!(
            diff.added[0].new_value.must().as_str().must(),
            "example.com"
        );

        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.removed[0].field_name, "EmployeeCount");
        assert_eq!(diff.removed[0].old_value.must().as_i64().must(), 100);
    }

    #[test]
    fn test_apply_diff() {
        let mut old_rec = create_record();
        old_rec.set_field("Name", "Acme");
        old_rec.set_field("Industry", "Retail");
        old_rec.set_field("EmployeeCount", 100);

        let mut new_rec = create_record();
        new_rec.set_field("Name", "Acme Corp");
        new_rec.set_field("Industry", "Retail");
        new_rec.set_field("Website", "example.com");

        let diff = compare_records(&old_rec, &new_rec);

        let mut target = old_rec.clone();
        diff.apply_to(&mut target);

        // target should now match new_rec
        assert_eq!(target.fields, new_rec.fields);
        assert_eq!(
            target.get_field_as::<String>("Name").must().must(),
            "Acme Corp"
        );
        assert_eq!(
            target.get_field_as::<String>("Website").must().must(),
            "example.com"
        );
        assert!(!target.has_field("EmployeeCount"));
    }
}
