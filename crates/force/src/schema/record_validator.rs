//! Record validation against SObject Describe metadata.
//!
//! This module provides utilities to validate `DynamicSObject` records against
//! the schema rules defined in an `SObjectDescribe`. This allows developers
//! to catch data errors (e.g., missing required fields, exceeding length limits,
//! or writing to read-only fields) locally before sending API requests.

use crate::types::describe::{FieldType, SObjectDescribe};
use crate::types::sobject::DynamicSObject;
use serde_json::Value;

/// Represents a validation error found in a record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// The record contains a field that does not exist in the object's schema.
    UnknownField(String),
    /// A required field is missing from the record.
    MissingRequiredField(String),
    /// A field value exceeds its maximum length constraint.
    LengthExceeded {
        /// The name of the field.
        field_name: String,
        /// The actual length of the value.
        actual_length: usize,
        /// The maximum allowed length.
        max_length: usize,
    },
    /// A read-only field (not createable/updateable) was modified.
    ReadOnlyFieldModified {
        /// The name of the field.
        field_name: String,
        /// Whether the field is createable.
        createable: bool,
        /// Whether the field is updateable.
        updateable: bool,
    },
    /// A field has an invalid type (e.g., expecting a number, got a string).
    InvalidType {
        /// The name of the field.
        field_name: String,
        /// The expected type.
        expected_type: String,
    },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownField(name) => write!(f, "Unknown field: {}", name),
            Self::MissingRequiredField(name) => write!(f, "Missing required field: {}", name),
            Self::LengthExceeded {
                field_name,
                actual_length,
                max_length,
            } => write!(
                f,
                "Field {} exceeds maximum length ({} > {})",
                field_name, actual_length, max_length
            ),
            Self::ReadOnlyFieldModified {
                field_name,
                createable,
                updateable,
            } => write!(
                f,
                "Field {} cannot be modified (createable: {}, updateable: {})",
                field_name, createable, updateable
            ),
            Self::InvalidType {
                field_name,
                expected_type,
            } => write!(
                f,
                "Field {} has an invalid type (expected {})",
                field_name, expected_type
            ),
        }
    }
}

/// A utility to validate records against their schema.
#[derive(Debug, Clone)]
pub struct RecordValidator<'a> {
    describe: &'a SObjectDescribe,
}

impl<'a> RecordValidator<'a> {
    /// Creates a new `RecordValidator` for the given `SObjectDescribe`.
    #[must_use]
    pub const fn new(describe: &'a SObjectDescribe) -> Self {
        Self { describe }
    }

    /// Validates a `DynamicSObject` record against the schema rules.
    ///
    /// # Arguments
    ///
    /// * `record` - The record to validate.
    /// * `is_update` - `true` if validating an update operation, `false` for insert.
    ///
    /// # Returns
    ///
    /// A list of validation errors. An empty list indicates the record is valid.
    #[must_use]
    pub fn validate_record(
        &self,
        record: &DynamicSObject,
        is_update: bool,
    ) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // Check for missing required fields (only on insert)
        if !is_update {
            for field in &self.describe.fields {
                if !field.nillable
                    && !field.defaulted_on_create
                    && field.name != "Id"
                    && !record.fields.contains_key(&field.name)
                {
                    errors.push(ValidationError::MissingRequiredField(field.name.clone()));
                }
            }
        }

        // Check each field in the record
        for (field_name, value) in &record.fields {
            let field = self.describe.fields.iter().find(|f| f.name == *field_name);

            if let Some(field) = field {
                // Check if field is read-only
                if (!is_update && !field.createable) || (is_update && !field.updateable) {
                    // It's okay if it's the Id field on an update
                    if !(is_update && field.name == "Id") {
                        errors.push(ValidationError::ReadOnlyFieldModified {
                            field_name: field.name.clone(),
                            createable: field.createable,
                            updateable: field.updateable,
                        });
                        continue;
                    }
                }

                if value.is_null() {
                    if !field.nillable && field.name != "Id" {
                        errors.push(ValidationError::MissingRequiredField(field.name.clone()));
                    }
                    continue;
                }

                // Check length for string types
                if let Value::String(s) = value {
                    #[allow(clippy::cast_sign_loss)]
                    let max_len = field.length as usize;
                    if max_len > 0 && s.len() > max_len {
                        errors.push(ValidationError::LengthExceeded {
                            field_name: field.name.clone(),
                            actual_length: s.len(),
                            max_length: max_len,
                        });
                    }

                    // Basic type checking
                    if !matches!(
                        field.type_,
                        FieldType::String
                            | FieldType::Textarea
                            | FieldType::Email
                            | FieldType::Phone
                            | FieldType::Url
                            | FieldType::Picklist
                            | FieldType::Multipicklist
                            | FieldType::Combobox
                            | FieldType::Reference
                            | FieldType::Id
                            | FieldType::Date
                            | FieldType::Datetime
                            | FieldType::Time
                            | FieldType::Base64
                            | FieldType::Location
                            | FieldType::AnyType
                    ) {
                        errors.push(ValidationError::InvalidType {
                            field_name: field.name.clone(),
                            expected_type: format!("{:?}", field.type_),
                        });
                    }
                } else if let Value::Number(_) = value {
                    if !matches!(
                        field.type_,
                        FieldType::Int
                            | FieldType::Double
                            | FieldType::Currency
                            | FieldType::Percent
                    ) {
                        errors.push(ValidationError::InvalidType {
                            field_name: field.name.clone(),
                            expected_type: format!("{:?}", field.type_),
                        });
                    }
                } else if let Value::Bool(_) = value {
                    if !matches!(field.type_, FieldType::Boolean) {
                        errors.push(ValidationError::InvalidType {
                            field_name: field.name.clone(),
                            expected_type: format!("{:?}", field.type_),
                        });
                    }
                }
            } else {
                errors.push(ValidationError::UnknownField(field_name.clone()));
            }
        }

        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Must;
    use crate::types::Attributes;
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
        nillable: bool,
        defaulted_on_create: bool,
        createable: bool,
        updateable: bool,
        length: i32,
    ) -> serde_json::Value {
        json!({
            "name": name,
            "type": field_type,
            "label": format!("{} Label", name),
            "referenceTo": [],
            "custom": false,
            "nillable": nillable,
            "defaultedOnCreate": defaulted_on_create,
            "calculated": false,
            "createable": createable,
            "updateable": updateable,
            "length": length,
            "autoNumber": false, "aggregatable": true, "byteLength": 18,
            "cascadeDelete": false, "caseSensitive": false,
            "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": true, "nameField": false, "namePointing": false,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:ID",
            "sortable": true, "unique": false, "writeRequiresMasterRead": false
        })
    }

    use crate::types::SalesforceId;
    use crate::types::api_version::ApiVersion;

    fn create_record(fields: serde_json::Map<String, Value>) -> DynamicSObject {
        let id = SalesforceId::new("001000000000000AAA").must();
        let api_ver = ApiVersion::V60;
        DynamicSObject {
            attributes: Attributes::new("Account", &id, &api_ver.to_string()),
            fields,
        }
    }

    #[test]
    fn test_valid_insert() {
        let describe = create_mock_describe(&json!([
            mock_field("Id", "id", false, false, false, false, 18),
            mock_field("Name", "string", false, false, true, true, 255),
            mock_field("Description", "string", true, false, true, true, 32000),
        ]));

        let validator = RecordValidator::new(&describe);
        let mut map = serde_json::Map::new();
        map.insert("Name".to_string(), Value::String("Acme Corp".to_string()));
        let record = create_record(map);

        let errors = validator.validate_record(&record, false);
        assert!(errors.is_empty(), "Expected no errors, got {:?}", errors);
    }

    #[test]
    fn test_missing_required_field_on_insert() {
        let describe = create_mock_describe(&json!([
            mock_field("Id", "id", false, false, false, false, 18),
            mock_field("Name", "string", false, false, true, true, 255),
        ]));

        let validator = RecordValidator::new(&describe);
        let record = create_record(serde_json::Map::new());

        let errors = validator.validate_record(&record, false);
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0],
            ValidationError::MissingRequiredField("Name".to_string())
        );
    }

    #[test]
    fn test_unknown_field() {
        let describe = create_mock_describe(&json!([
            mock_field("Id", "id", false, false, false, false, 18),
            mock_field("Name", "string", false, false, true, true, 255),
        ]));

        let validator = RecordValidator::new(&describe);
        let mut map = serde_json::Map::new();
        map.insert("Name".to_string(), Value::String("Acme Corp".to_string()));
        map.insert("Unknown__c".to_string(), Value::String("Oops".to_string()));
        let record = create_record(map);

        let errors = validator.validate_record(&record, false);
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0],
            ValidationError::UnknownField("Unknown__c".to_string())
        );
    }

    #[test]
    fn test_length_exceeded() {
        let describe = create_mock_describe(&json!([
            mock_field("Id", "id", false, false, false, false, 18),
            mock_field("Name", "string", false, false, true, true, 5),
        ]));

        let validator = RecordValidator::new(&describe);
        let mut map = serde_json::Map::new();
        map.insert("Name".to_string(), Value::String("Too Long".to_string()));
        let record = create_record(map);

        let errors = validator.validate_record(&record, false);
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0],
            ValidationError::LengthExceeded {
                field_name: "Name".to_string(),
                actual_length: 8,
                max_length: 5,
            }
        );
    }

    #[test]
    fn test_read_only_field_on_insert() {
        let describe = create_mock_describe(&json!([
            mock_field("Id", "id", false, false, false, false, 18),
            mock_field("Name", "string", false, false, true, true, 255),
            mock_field("CreatedDate", "datetime", false, true, false, false, 0),
        ]));

        let validator = RecordValidator::new(&describe);
        let mut map = serde_json::Map::new();
        map.insert("Name".to_string(), Value::String("Acme Corp".to_string()));
        map.insert(
            "CreatedDate".to_string(),
            Value::String("2023-01-01T00:00:00Z".to_string()),
        );
        let record = create_record(map);

        let errors = validator.validate_record(&record, false);
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0],
            ValidationError::ReadOnlyFieldModified {
                field_name: "CreatedDate".to_string(),
                createable: false,
                updateable: false,
            }
        );
    }

    #[test]
    fn test_id_allowed_on_update() {
        let describe = create_mock_describe(&json!([
            mock_field("Id", "id", false, false, false, false, 18),
            mock_field("Name", "string", false, false, true, true, 255),
        ]));

        let validator = RecordValidator::new(&describe);
        let mut map = serde_json::Map::new();
        map.insert(
            "Id".to_string(),
            Value::String("001000000000000AAA".to_string()),
        );
        map.insert("Name".to_string(), Value::String("Acme Corp".to_string()));
        let record = create_record(map);

        let errors = validator.validate_record(&record, true);
        assert!(
            errors.is_empty(),
            "Expected no errors on update with Id, got {:?}",
            errors
        );
    }

    #[test]
    fn test_invalid_type() {
        let describe = create_mock_describe(&json!([
            mock_field("Id", "id", false, false, false, false, 18),
            mock_field("Name", "string", false, false, true, true, 255),
            mock_field("Amount", "currency", true, false, true, true, 0),
        ]));

        let validator = RecordValidator::new(&describe);
        let mut map = serde_json::Map::new();
        map.insert("Name".to_string(), Value::String("Acme Corp".to_string()));
        // Try to pass a string to a currency field
        map.insert("Amount".to_string(), Value::String("100.00".to_string()));
        let record = create_record(map);

        let errors = validator.validate_record(&record, false);
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0],
            ValidationError::InvalidType {
                field_name: "Amount".to_string(),
                expected_type: "Currency".to_string(),
            }
        );
    }
}
