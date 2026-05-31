//! Chaos Engineering for Salesforce Data.
//!
//! This module provides the `DataFuzzer` utility to intentionally corrupt
//! or mutate `DynamicSObject` records against an `SObjectDescribe` metadata payload.
//!
//! # The Spark
//! We have `DataValidator` to ensure records are perfect before sending, and
//! `DataFaker` to generate pristine data. But what if developers want to test
//! their Apex triggers, validation rules, or error handling? `DataFuzzer` acts
//! as a Chaos Monkey, injecting specific violations (like exceeding field lengths,
//! nulling required fields, or changing types) into valid records!
//!
//! # Example
//!
//! ```no_run
//! # use force::data::DataFuzzer;
//! # use force::data::generate_mock_record;
//! # fn main() {
//! # let describe_json = serde_json::json!({"name": "Account", "label": "Account", "fields": []});
//! # let describe: force::types::describe::SObjectDescribe = serde_json::from_value(describe_json).unwrap();
//! let valid_record = generate_mock_record(&describe);
//! let fuzzer = DataFuzzer::new(&describe);
//! let fuzzed = fuzzer.fuzz(valid_record);
//!
//! println!("Mutations applied: {:?}", fuzzed.mutations);
//! # }
//! ```

use crate::types::DynamicSObject;
use crate::types::describe::{FieldType, SObjectDescribe};
use serde_json::Value;

/// The type of mutation applied to the record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutationType {
    /// Removed a required field.
    RemovedRequiredField(String),
    /// Exceeded the maximum length of a string field.
    LengthExceeded(String, i32),
    /// Changed the type of a field to something invalid.
    TypeMismatch(String),
}

/// Represents a record that has been fuzzed.
#[derive(Debug, Clone)]
pub struct FuzzedRecord {
    /// The mutated record.
    pub record: DynamicSObject,
    /// The mutations applied.
    pub mutations: Vec<MutationType>,
}

/// Utility for intentionally mutating `DynamicSObject` records to test resilience.
#[derive(Debug, Clone)]
pub struct DataFuzzer<'a> {
    describe: &'a SObjectDescribe,
}

impl<'a> DataFuzzer<'a> {
    /// Creates a new `DataFuzzer` initialized with the target schema.
    #[must_use]
    pub fn new(describe: &'a SObjectDescribe) -> Self {
        Self { describe }
    }

    /// Mutates the given record to introduce schema violations.
    ///
    /// It attempts to introduce a variety of errors into the record.
    #[must_use]
    pub fn fuzz(&self, mut record: DynamicSObject) -> FuzzedRecord {
        let mut mutations = Vec::new();
        let mut removed_required = false;
        let mut exceeded_length = false;
        let mut type_mismatched = false;

        for field in &self.describe.fields {
            if !field.createable {
                continue;
            }

            // 1. Missing Required Field
            if !removed_required
                && !field.nillable
                && !field.defaulted_on_create
                && field.name != "Id"
            {
                record.fields.remove(&field.name);
                mutations.push(MutationType::RemovedRequiredField(field.name.clone()));
                removed_required = true;
                continue;
            }

            // 2. Length Exceeded
            if !exceeded_length
                && field.type_ == FieldType::String
                && field.length > 0
                && field.length < 10000
            {
                let long_string = "X".repeat(field.length.unsigned_abs() as usize + 10);
                record
                    .fields
                    .insert(field.name.clone(), Value::String(long_string));
                mutations.push(MutationType::LengthExceeded(
                    field.name.clone(),
                    field.length,
                ));
                exceeded_length = true;
                continue;
            }

            // 3. Type Mismatch
            if !type_mismatched && field.type_ == FieldType::Boolean {
                record.fields.insert(
                    field.name.clone(),
                    Value::String("not_a_boolean".to_string()),
                );
                mutations.push(MutationType::TypeMismatch(field.name.clone()));
                type_mismatched = true;
                continue;
            }
        }

        FuzzedRecord { record, mutations }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;

    #[test]
    fn test_data_fuzzer() {
        let describe_json: serde_json::Value = serde_json::from_str(r#"{
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
            "fields": [
                {
                    "name": "LastName", "type": "string", "label": "Last Name", "createable": true,
                    "autoNumber": false, "calculated": false,
                    "aggregatable": true, "byteLength": 80,
                    "cascadeDelete": false, "caseSensitive": false, "custom": false,
                    "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 80, "nameField": true, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false,
                    "referenceTo": []
                },
                {
                    "name": "FirstName", "type": "string", "label": "First Name", "createable": true,
                    "autoNumber": false, "calculated": false,
                    "aggregatable": true, "byteLength": 40,
                    "cascadeDelete": false, "caseSensitive": false, "custom": false,
                    "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 40, "nameField": false, "namePointing": false, "nillable": true,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false,
                    "referenceTo": []
                },
                {
                    "name": "DoNotCall", "type": "boolean", "label": "Do Not Call", "createable": true,
                    "autoNumber": false, "calculated": false,
                    "aggregatable": true, "byteLength": 0,
                    "cascadeDelete": false, "caseSensitive": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 0, "nameField": false, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:boolean",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false,
                    "referenceTo": []
                }
            ]
        }"#).must();

        let describe: SObjectDescribe = serde_json::from_value(describe_json).must();
        let valid_record = crate::data::generate_mock_record(&describe);

        let data_fuzzer = DataFuzzer::new(&describe);
        let fuzzed_record = data_fuzzer.fuzz(valid_record);

        assert_eq!(fuzzed_record.mutations.len(), 3);

        let has_removed = fuzzed_record
            .mutations
            .iter()
            .any(|m| matches!(m, MutationType::RemovedRequiredField(n) if n == "LastName"));
        let has_exceeded = fuzzed_record
            .mutations
            .iter()
            .any(|m| matches!(m, MutationType::LengthExceeded(n, _) if n == "FirstName"));
        let has_mismatch = fuzzed_record
            .mutations
            .iter()
            .any(|m| matches!(m, MutationType::TypeMismatch(n) if n == "DoNotCall"));

        assert!(has_removed, "Missing RemovedRequiredField mutation");
        assert!(has_exceeded, "Missing LengthExceeded mutation");
        assert!(has_mismatch, "Missing TypeMismatch mutation");

        assert!(!fuzzed_record.record.fields.contains_key("LastName"));

        let first_name_field = fuzzed_record
            .record
            .fields
            .get("FirstName")
            .unwrap_or_else(|| panic!("FirstName should exist"));
        let first_name = first_name_field
            .as_str()
            .unwrap_or_else(|| panic!("FirstName should be a string"));
        assert!(first_name.len() > 40);

        let do_not_call = fuzzed_record.record.fields.get("DoNotCall").unwrap_or_else(|| panic!("DoNotCall should exist"));
        assert!(do_not_call.is_string());
    }
}
