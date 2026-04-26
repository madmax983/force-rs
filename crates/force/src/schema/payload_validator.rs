//! Schema Payload Validator.
//!
//! This module provides `PayloadValidator`, a utility that validates JSON data
//! against an `SObjectDescribe` schema before inserting or updating records.
//!
//! # The Spark
//! We have `DataFaker` to mock payloads, but no way to validate incoming data!
//! `PayloadValidator` acts as a guard, checking for type mismatches, missing
//! required fields, length violations, and write-only field attempts *before*
//! making expensive API calls to Salesforce.

use crate::types::describe::{FieldType, SObjectDescribe};
use serde_json::Value;

/// Validation error variants.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    /// A required field is missing from the payload.
    #[error("Missing required field: {0}")]
    MissingRequiredField(String),

    /// A field value is not of the expected type.
    #[error("Type mismatch on field {field}: expected {expected}, got {actual}")]
    TypeMismatch {
        /// The field name.
        field: String,
        /// The expected type.
        expected: String,
        /// The actual type received.
        actual: String,
    },

    /// A string field exceeds the maximum length.
    #[error("Field {field} exceeds max length of {max_len}")]
    LengthExceeded {
        /// The field name.
        field: String,
        /// The maximum allowed length.
        max_len: i32,
    },

    /// An attempt was made to write to a non-createable/updateable field.
    #[error("Field {0} is read-only in this context")]
    ReadOnlyField(String),
}

/// Validates JSON payloads against an `SObjectDescribe` schema.
#[derive(Debug, Clone)]
pub struct PayloadValidator<'a> {
    describe: &'a SObjectDescribe,
}

impl<'a> PayloadValidator<'a> {
    /// Creates a new validator for the given schema.
    #[must_use]
    pub fn new(describe: &'a SObjectDescribe) -> Self {
        Self { describe }
    }

    /// Validates a JSON payload for a CREATE operation.
    ///
    /// # Errors
    ///
    /// Returns a list of `ValidationError`s if the payload is invalid.
    pub fn validate_create(&self, payload: &Value) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        let Some(obj) = payload.as_object() else {
            errors.push(ValidationError::TypeMismatch {
                field: "payload".to_string(),
                expected: "Object".to_string(),
                actual: "Other".to_string(),
            });
            return Err(errors);
        };

        // Check for missing required fields
        for field in &self.describe.fields {
            if !field.nillable && !field.defaulted_on_create && field.createable {
                if !obj.contains_key(&field.name) {
                    errors.push(ValidationError::MissingRequiredField(field.name.clone()));
                }
            }
        }

        // Validate provided fields
        for (key, val) in obj {
            if let Some(field) = self
                .describe
                .fields
                .iter()
                .find(|f| f.name.eq_ignore_ascii_case(key))
            {
                if !field.createable {
                    errors.push(ValidationError::ReadOnlyField(field.name.clone()));
                    continue;
                }

                Self::validate_field_value(field, val, &mut errors);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn validate_field_value(
        field: &crate::types::describe::FieldDescribe,
        val: &Value,
        errors: &mut Vec<ValidationError>,
    ) {
        if val.is_null() {
            if !field.nillable {
                errors.push(ValidationError::MissingRequiredField(field.name.clone()));
            }
            return;
        }

        match field.type_ {
            FieldType::String
            | FieldType::Textarea
            | FieldType::Phone
            | FieldType::Email
            | FieldType::Url
            | FieldType::Id
            | FieldType::Reference => {
                if let Some(s) = val.as_str() {
                    if s.len() > field.length.try_into().unwrap_or(0) {
                        errors.push(ValidationError::LengthExceeded {
                            field: field.name.clone(),
                            max_len: field.length,
                        });
                    }
                } else {
                    errors.push(ValidationError::TypeMismatch {
                        field: field.name.clone(),
                        expected: "String".to_string(),
                        actual: Self::value_type_name(val).to_string(),
                    });
                }
            }
            FieldType::Boolean if !val.is_boolean() => {
                errors.push(ValidationError::TypeMismatch {
                    field: field.name.clone(),
                    expected: "Boolean".to_string(),
                    actual: Self::value_type_name(val).to_string(),
                });
            }

            FieldType::Int | FieldType::Double | FieldType::Currency | FieldType::Percent
                if !val.is_number() =>
            {
                errors.push(ValidationError::TypeMismatch {
                    field: field.name.clone(),
                    expected: "Number".to_string(),
                    actual: Self::value_type_name(val).to_string(),
                });
            }

            _ => {} // Other types omitted for brevity in prototype
        }
    }

    fn value_type_name(val: &Value) -> &'static str {
        match val {
            Value::Null => "Null",
            Value::Bool(_) => "Boolean",
            Value::Number(_) => "Number",
            Value::String(_) => "String",
            Value::Array(_) => "Array",
            Value::Object(_) => "Object",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Must;
    use serde_json::json;

    fn mock_field(
        name: &str,
        type_: &str,
        createable: bool,
        nillable: bool,
        length: i32,
    ) -> serde_json::Value {
        json!({
            "name": name,
            "type": type_,
            "label": name,
            "createable": createable,
            "nillable": nillable,
            "length": length,
            "defaultedOnCreate": false,
            "custom": false, "calculated": false, "autoNumber": false, "aggregatable": false,
            "byteLength": length, "cascadeDelete": false, "caseSensitive": false,
            "dependentPicklist": false, "deprecatedAndHidden": false, "digits": 0,
            "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": false, "groupable": false, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": false, "nameField": false, "namePointing": false,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "referenceTo": [], "restrictedDelete": false, "restrictedPicklist": false, "scale": 0,
            "soapType": "xsd:string", "sortable": false, "unique": false, "updateable": createable,
            "writeRequiresMasterRead": false
        })
    }

    #[test]
    fn test_validate_create_success() {
        let describe_json = json!({
            "name": "Account", "label": "Account", "custom": false, "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": false, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "001", "labelPlural": "Accounts", "layoutable": false,
            "mergeable": false, "mruEnabled": false, "replicateable": false, "retrieveable": false,
            "searchable": false, "triggerable": false, "undeletable": false, "updateable": false,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": [
                mock_field("Name", "string", true, false, 255),
                mock_field("Age", "int", true, true, 0)
            ]
        });
        let describe: SObjectDescribe = serde_json::from_value(describe_json).must();
        let validator = PayloadValidator::new(&describe);

        let payload = json!({
            "Name": "Test Account",
            "Age": 30
        });

        assert!(validator.validate_create(&payload).is_ok());
    }

    #[test]
    fn test_validate_create_missing_required() {
        let describe_json = json!({
            "name": "Account", "label": "Account", "custom": false, "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": false, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "001", "labelPlural": "Accounts", "layoutable": false,
            "mergeable": false, "mruEnabled": false, "replicateable": false, "retrieveable": false,
            "searchable": false, "triggerable": false, "undeletable": false, "updateable": false,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": [
                mock_field("Name", "string", true, false, 255),
                mock_field("Age", "int", true, true, 0)
            ]
        });
        let describe: SObjectDescribe = serde_json::from_value(describe_json).must();
        let validator = PayloadValidator::new(&describe);

        let payload = json!({
            "Age": 30
        });

        let result = validator.validate_create(&payload);
        let Err(errs) = result else {
            panic!("Expected validation error");
        };
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0],
            ValidationError::MissingRequiredField("Name".to_string())
        );
    }

    #[test]
    fn test_validate_create_length_exceeded() {
        let describe_json = json!({
            "name": "Account", "label": "Account", "custom": false, "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": false, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "001", "labelPlural": "Accounts", "layoutable": false,
            "mergeable": false, "mruEnabled": false, "replicateable": false, "retrieveable": false,
            "searchable": false, "triggerable": false, "undeletable": false, "updateable": false,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": [
                mock_field("ShortCode", "string", true, true, 5)
            ]
        });
        let describe: SObjectDescribe = serde_json::from_value(describe_json).must();
        let validator = PayloadValidator::new(&describe);

        let payload = json!({
            "ShortCode": "TooLong"
        });

        let result = validator.validate_create(&payload);
        let Err(errs) = result else {
            panic!("Expected validation error");
        };
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0],
            ValidationError::LengthExceeded {
                field: "ShortCode".to_string(),
                max_len: 5
            }
        );
    }

    #[test]
    fn test_validate_create_type_mismatch() {
        let describe_json = json!({
            "name": "Account", "label": "Account", "custom": false, "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": false, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "001", "labelPlural": "Accounts", "layoutable": false,
            "mergeable": false, "mruEnabled": false, "replicateable": false, "retrieveable": false,
            "searchable": false, "triggerable": false, "undeletable": false, "updateable": false,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": [
                mock_field("IsActive", "boolean", true, true, 0)
            ]
        });
        let describe: SObjectDescribe = serde_json::from_value(describe_json).must();
        let validator = PayloadValidator::new(&describe);

        let payload = json!({
            "IsActive": "true" // String instead of bool
        });

        let result = validator.validate_create(&payload);
        let Err(errs) = result else {
            panic!("Expected validation error");
        };
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0],
            ValidationError::TypeMismatch {
                field: "IsActive".to_string(),
                expected: "Boolean".to_string(),
                actual: "String".to_string()
            }
        );
    }

    #[test]
    fn test_validate_create_readonly() {
        let describe_json = json!({
            "name": "Account", "label": "Account", "custom": false, "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": false, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "001", "labelPlural": "Accounts", "layoutable": false,
            "mergeable": false, "mruEnabled": false, "replicateable": false, "retrieveable": false,
            "searchable": false, "triggerable": false, "undeletable": false, "updateable": false,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": [
                mock_field("CreatedDate", "datetime", false, false, 0)
            ]
        });
        let describe: SObjectDescribe = serde_json::from_value(describe_json).must();
        let validator = PayloadValidator::new(&describe);

        let payload = json!({
            "CreatedDate": "2023-01-01T00:00:00Z"
        });

        let result = validator.validate_create(&payload);
        let Err(errs) = result else {
            panic!("Expected validation error");
        };
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0],
            ValidationError::ReadOnlyField("CreatedDate".to_string())
        );
    }
}
