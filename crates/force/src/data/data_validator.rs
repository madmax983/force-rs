//! Local Record Validator.
//!
//! This module provides the `DataValidator` utility to automatically validate
//! `DynamicSObject` records against an `SObjectDescribe` metadata payload *before*
//! sending them to Salesforce.
//!
//! # The Spark
//! We have `DataFaker` to generate dummy data and `DataMasker` to sanitize it.
//! But what if developers create or modify records locally and want to know if
//! they will fail insertion *without* making an API call? `DataValidator`
//! uses the schema (required fields, string lengths, restricted picklists)
//! to fail fast and save API limits!

use crate::api::rest::describe::{FieldType, SObjectDescribe};
use crate::types::DynamicSObject;

/// A validation error found in a record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// The API name of the field that failed validation.
    pub field: String,
    /// A human-readable description of the validation failure.
    pub message: String,
}

/// Utility for validating records locally against Salesforce schema.
#[derive(Debug, Clone)]
pub struct DataValidator<'a> {
    describe: &'a SObjectDescribe,
}

impl<'a> DataValidator<'a> {
    /// Creates a new `DataValidator` initialized with the target schema.
    #[must_use]
    pub fn new(describe: &'a SObjectDescribe) -> Self {
        Self { describe }
    }

    /// Validates a record against the schema.
    ///
    /// Returns a list of `ValidationError`s if the record is invalid,
    /// or an empty list if it passes all checks.
    #[must_use]
    pub fn validate(&self, record: &DynamicSObject) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        for field in &self.describe.fields {
            let value = record.get_field(&field.name);

            // 1. Check required fields
            let is_required = !field.nillable && !field.defaulted_on_create && field.name != "Id";
            if is_required {
                let is_missing_or_null = match value {
                    None => true,
                    Some(v) => v.is_null(),
                };
                if is_missing_or_null {
                    errors.push(ValidationError {
                        field: field.name.clone(),
                        message: format!(
                            "Field '{}' is required but is missing or null.",
                            field.name
                        ),
                    });
                    continue; // Skip further validation if it's missing
                }
            }

            // 2. Value-based validation (only if value exists and is not null)
            if let Some(val) = value {
                if !val.is_null() {
                    // 2a. String length
                    if let Some(s) = val.as_str() {
                        if field.type_ == FieldType::String
                            || field.type_ == FieldType::Textarea
                            || field.type_ == FieldType::Url
                            || field.type_ == FieldType::Email
                            || field.type_ == FieldType::Phone
                        {
                            if s.chars().count() > field.length.try_into().unwrap_or(usize::MAX) {
                                errors.push(ValidationError {
                                    field: field.name.clone(),
                                    message: format!(
                                        "Value exceeds maximum length of {} characters.",
                                        field.length
                                    ),
                                });
                            }
                        }
                    }

                    // 2b. Restricted Picklist
                    if field.restricted_picklist
                        && (field.type_ == FieldType::Picklist
                            || field.type_ == FieldType::Multipicklist)
                    {
                        if let Some(s) = val.as_str() {
                            let is_valid = if let Some(picklist_values) = &field.picklist_values {
                                picklist_values.iter().any(|pv| pv.active && pv.value == s)
                            } else {
                                false // If restricted but no values are provided in describe, it's invalid (or unvalidatable)
                            };

                            if !is_valid {
                                errors.push(ValidationError {
                                    field: field.name.clone(),
                                    message: format!("'{}' is an invalid picklist value.", s),
                                });
                            }
                        }
                    }
                }
            }
        }

        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Must;
    use crate::types::{Attributes, SalesforceId};
    use serde_json::json;

    fn create_mock_describe(fields_json: &serde_json::Value) -> SObjectDescribe {
        let describe_json = json!({
            "name": "Account",
            "label": "Account",
            "custom": false,
            "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "001", "labelPlural": "Accounts", "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": fields_json.clone()
        });
        serde_json::from_value(describe_json).must()
    }

    fn mock_field(
        name: &str,
        field_type: &str,
        required: bool,
        length: i32,
        restricted_picklist: bool,
    ) -> serde_json::Value {
        json!({
            "name": name,
            "type": field_type,
            "label": format!("{} Label", name),
            "referenceTo": [],
            "nillable": !required,
            "defaultedOnCreate": false,
            "length": length,
            "restrictedPicklist": restricted_picklist,
            "picklistValues": if field_type == "picklist" {
                json!([
                    {"active": true, "value": "Valid1", "defaultValue": false, "label": "Valid 1"},
                    {"active": true, "value": "Valid2", "defaultValue": false, "label": "Valid 2"}
                ])
            } else {
                serde_json::Value::Null
            },
            // Mandatory fields filler
            "createable": true, "autoNumber": false, "calculated": false,
            "aggregatable": true, "byteLength": length * 3, "cascadeDelete": false,
            "caseSensitive": false, "custom": false, "dependentPicklist": false,
            "deprecatedAndHidden": false, "digits": 0, "displayLocationInDecimal": false,
            "encrypted": false, "externalId": false, "filterable": true, "groupable": true,
            "highScaleNumber": false, "htmlFormatted": false, "idLookup": false,
            "nameField": false, "namePointing": false, "permissionable": false,
            "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "restrictedDelete": false, "scale": 0, "soapType": "xsd:string",
            "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false
        })
    }

    fn create_mock_record() -> DynamicSObject {
        let id = SalesforceId::new("001000000000001AAA").must();
        let attrs = Attributes::new("Account", &id, "v60.0");
        DynamicSObject::new(attrs)
    }

    #[test]
    fn test_validate_required_fields() {
        let describe = create_mock_describe(&json!([
            mock_field("Name", "string", true, 255, false),
            mock_field("Description", "string", false, 255, false)
        ]));

        let validator = DataValidator::new(&describe);
        let record = create_mock_record(); // Empty record

        let errors = validator.validate(&record);

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].field, "Name");
        assert!(errors[0].message.contains("required"));
    }

    #[test]
    fn test_validate_string_length() {
        let describe = create_mock_describe(&json!([mock_field(
            "ShortField",
            "string",
            false,
            10,
            false
        )]));

        let validator = DataValidator::new(&describe);
        let mut record = create_mock_record();
        record.set_field("ShortField", "This is way too long");

        let errors = validator.validate(&record);

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].field, "ShortField");
        assert!(errors[0].message.contains("exceeds maximum length"));
    }

    #[test]
    fn test_validate_restricted_picklist() {
        let describe =
            create_mock_describe(&json!([mock_field("Status", "picklist", false, 255, true)]));

        let validator = DataValidator::new(&describe);
        let mut record = create_mock_record();
        record.set_field("Status", "InvalidValue");

        let errors = validator.validate(&record);

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].field, "Status");
        assert!(errors[0].message.contains("invalid picklist value"));
    }

    #[test]
    fn test_validate_valid_record() {
        let describe = create_mock_describe(&json!([
            mock_field("Name", "string", true, 255, false),
            mock_field("Status", "picklist", false, 255, true)
        ]));

        let validator = DataValidator::new(&describe);
        let mut record = create_mock_record();
        record.set_field("Name", "Valid Name");
        record.set_field("Status", "Valid1");

        let errors = validator.validate(&record);

        assert_eq!(errors.len(), 0);
    }
}
