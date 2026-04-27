//! Client-side validation for Salesforce Records.
//!
//! This module provides the `DataValidator` utility to automatically validate
//! `DynamicSObject` records against an `SObjectDescribe` metadata payload *before*
//! sending them to Salesforce. This saves API calls and catches data errors locally.
//!
//! # The Spark
//! We have `DataFaker` to generate data and `DataMasker` to sanitize it.
//! But what if developers try to upload records that are missing required fields
//! or have strings that are too long? `DataValidator` uses the object schema to
//! enforce data integrity on the client side, preventing slow API failures!
//!
//! # Example
//!
//! ```no_run
//! # use force::api::RestOperation;
//! # use force::client::ForceClientBuilder;
//! # use force::data::DataValidator;
//! # use force::auth::ClientCredentials;
//! # use force::types::DynamicSObject;
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! # let auth = ClientCredentials::new("id", "secret", "url");
//! # let client = ForceClientBuilder::new().authenticate(auth).build().await?;
//! // Fetch metadata for Contact
//! let describe = client.rest().describe("Contact").await?;
//!
//! let validator = DataValidator::new(&describe);
//!
//! // Assume we have a record to validate
//! let contact: DynamicSObject = /* ... */
//! # force::data::generate_mock_record(&describe);
//!
//! if let Err(errors) = validator.validate(&contact) {
//!     for err in errors {
//!         println!("Validation Error: {}", err);
//!     }
//! }
//! # Ok(())
//! # }
//! ```

use crate::types::DynamicSObject;
use crate::types::describe::{FieldDescribe, FieldType, SObjectDescribe};
use serde_json::Value;

/// Represents a validation error on a specific field.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    /// A required field is missing or null.
    #[error("Required field is missing or null: {0}")]
    MissingRequiredField(String),

    /// A string field exceeds the maximum allowed length.
    #[error("String length for '{0}' exceeds maximum length of {1}")]
    LengthExceeded(String, i32),

    /// A field value's JSON type does not match the expected Salesforce field type.
    #[error("Type mismatch for '{0}': expected {1}, found {2}")]
    TypeMismatch(String, String, String),
}

/// Utility for validating `DynamicSObject` records against schema rules.
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
    /// Returns `Ok(())` if the record passes all checks.
    /// Returns `Err(Vec<ValidationError>)` if any validation rules are violated.
    ///
    /// The validation includes:
    /// - Required fields checking (fields that are not nillable, not defaulted on create, and are createable).
    /// - String length constraints.
    /// - Basic type checking (ensuring strings are strings, numbers are numbers, etc.).
    ///
    /// Note: This method currently validates for a "create" operation context.
    /// It does not enforce uniqueness or complex cross-field dependencies.
    pub fn validate(&self, record: &DynamicSObject) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        // 1. Check for missing required fields
        for field in &self.describe.fields {
            // A field is strictly required on create if it's createable, not nillable, and has no default.
            if field.createable && !field.nillable && !field.defaulted_on_create {
                let value_opt = record.fields.get(&field.name);
                if value_opt.is_none_or(serde_json::Value::is_null) {
                    errors.push(ValidationError::MissingRequiredField(field.name.clone()));
                }
            }
        }

        // 2. Validate populated fields against their definitions
        for (key, val) in &record.fields {
            if val.is_null() {
                continue;
            }

            if let Some(field) = self.find_field(key) {
                // Length check for string-like fields
                if Self::is_string_type(&field.type_) {
                    if let Value::String(s) = val {
                        if field.length > 0
                            && s.chars().count() > field.length.unsigned_abs() as usize
                        {
                            errors.push(ValidationError::LengthExceeded(
                                field.name.clone(),
                                field.length,
                            ));
                        }
                    } else {
                        errors.push(ValidationError::TypeMismatch(
                            field.name.clone(),
                            "string".to_string(),
                            Self::value_type_name(val).to_string(),
                        ));
                    }
                } else if field.type_ == FieldType::Boolean {
                    if !val.is_boolean() {
                        errors.push(ValidationError::TypeMismatch(
                            field.name.clone(),
                            "boolean".to_string(),
                            Self::value_type_name(val).to_string(),
                        ));
                    }
                } else if Self::is_number_type(&field.type_) {
                    if !val.is_number() {
                        errors.push(ValidationError::TypeMismatch(
                            field.name.clone(),
                            "number".to_string(),
                            Self::value_type_name(val).to_string(),
                        ));
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Finds a field definition by name (case-insensitive).
    fn find_field(&self, name: &str) -> Option<&FieldDescribe> {
        self.describe
            .fields
            .iter()
            .find(|f| f.name.eq_ignore_ascii_case(name))
    }

    fn is_string_type(field_type: &FieldType) -> bool {
        matches!(
            field_type,
            FieldType::String
                | FieldType::Textarea
                | FieldType::Email
                | FieldType::Phone
                | FieldType::Url
                | FieldType::Id
                | FieldType::Reference
                | FieldType::Picklist
                | FieldType::Combobox
        )
    }

    fn is_number_type(field_type: &FieldType) -> bool {
        matches!(
            field_type,
            FieldType::Int | FieldType::Double | FieldType::Currency | FieldType::Percent
        )
    }

    fn value_type_name(val: &Value) -> &'static str {
        match val {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        }
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

    fn mock_field(name: &str, field_type: &str, nillable: bool, length: i32) -> serde_json::Value {
        json!({
            "name": name,
            "type": field_type,
            "label": format!("{} Label", name),
            "referenceTo": [],
            "encrypted": false,
            "createable": true, "autoNumber": false, "calculated": false,
            "aggregatable": true, "byteLength": length * 3, "cascadeDelete": false,
            "caseSensitive": false, "custom": false, "defaultedOnCreate": false,
            "dependentPicklist": false, "deprecatedAndHidden": false, "digits": 0,
            "displayLocationInDecimal": false, "externalId": false, "filterable": true,
            "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": false, "length": length, "nameField": false, "namePointing": false,
            "nillable": nillable, "permissionable": false, "polymorphicForeignKey": false,
            "precision": 0, "queryByDistance": false, "restrictedDelete": false,
            "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
            "sortable": true, "unique": false, "updateable": true,
            "writeRequiresMasterRead": false
        })
    }

    fn create_mock_record(fields: serde_json::Map<String, Value>) -> DynamicSObject {
        let id = SalesforceId::new("003000000000001AAA").must();
        let attrs = Attributes::new("Contact", &id, "v60.0");
        let mut record = DynamicSObject::new(attrs);
        record.fields = fields;
        record
    }

    #[test]
    fn test_validation_error_display() {
        assert_eq!(
            ValidationError::MissingRequiredField("LastName".to_string()).to_string(),
            "Required field is missing or null: LastName"
        );
        assert_eq!(
            ValidationError::LengthExceeded("FirstName".to_string(), 10).to_string(),
            "String length for 'FirstName' exceeds maximum length of 10"
        );
        assert_eq!(
            ValidationError::TypeMismatch(
                "Age".to_string(),
                "number".to_string(),
                "string".to_string()
            )
            .to_string(),
            "Type mismatch for 'Age': expected number, found string"
        );
    }

    #[test]
    fn test_valid_record() {
        let describe = create_mock_describe(&json!([
            mock_field("LastName", "string", false, 80),
            mock_field("FirstName", "string", true, 40),
            mock_field("Age", "int", true, 0)
        ]));

        let validator = DataValidator::new(&describe);

        let mut record_fields = serde_json::Map::new();
        record_fields.insert("LastName".to_string(), json!("Doe"));
        record_fields.insert("FirstName".to_string(), json!("John"));
        record_fields.insert("Age".to_string(), json!(30));

        let record = create_mock_record(record_fields);

        assert_eq!(validator.validate(&record), Ok(()));
    }

    #[test]
    fn test_missing_required_field() {
        let describe = create_mock_describe(&json!([
            mock_field("LastName", "string", false, 80),
            mock_field("FirstName", "string", true, 40)
        ]));

        let validator = DataValidator::new(&describe);

        let mut record_fields = serde_json::Map::new();
        record_fields.insert("FirstName".to_string(), json!("John"));
        // Missing LastName

        let record = create_mock_record(record_fields);

        let errors = match validator.validate(&record) {
            Ok(()) => panic!("Expected validation error"),
            Err(e) => e,
        };
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0],
            ValidationError::MissingRequiredField("LastName".to_string())
        );
    }

    #[test]
    fn test_null_required_field() {
        let describe = create_mock_describe(&json!([
            mock_field("LastName", "string", false, 80),
            mock_field("FirstName", "string", true, 40)
        ]));

        let validator = DataValidator::new(&describe);

        let mut record_fields = serde_json::Map::new();
        record_fields.insert("LastName".to_string(), json!(null)); // Required field is null
        record_fields.insert("FirstName".to_string(), json!("John"));

        let record = create_mock_record(record_fields);

        let errors = match validator.validate(&record) {
            Ok(()) => panic!("Expected validation error"),
            Err(e) => e,
        };
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0],
            ValidationError::MissingRequiredField("LastName".to_string())
        );
    }

    #[test]
    fn test_length_exceeded() {
        let describe = create_mock_describe(&json!([
            mock_field("FirstName", "string", true, 5) // max length 5
        ]));

        let validator = DataValidator::new(&describe);

        let mut record_fields = serde_json::Map::new();
        record_fields.insert("FirstName".to_string(), json!("Jonathan")); // length 8

        let record = create_mock_record(record_fields);

        let errors = match validator.validate(&record) {
            Ok(()) => panic!("Expected validation error"),
            Err(e) => e,
        };
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0],
            ValidationError::LengthExceeded("FirstName".to_string(), 5)
        );
    }

    #[test]
    fn test_type_mismatch() {
        let describe = create_mock_describe(&json!([
            mock_field("Age", "int", true, 0),
            mock_field("IsActive", "boolean", true, 0),
            mock_field("Name", "string", true, 80)
        ]));

        let validator = DataValidator::new(&describe);

        let mut record_fields = serde_json::Map::new();
        record_fields.insert("Age".to_string(), json!("Thirty")); // Expected number, found string
        record_fields.insert("IsActive".to_string(), json!("True")); // Expected boolean, found string
        record_fields.insert("Name".to_string(), json!(123)); // Expected string, found number

        let record = create_mock_record(record_fields);

        let mut errors = match validator.validate(&record) {
            Ok(()) => panic!("Expected validation error"),
            Err(e) => e,
        };
        // Sort to make assertion deterministic as map iteration order isn't guaranteed
        errors.sort_by_key(|e| e.to_string());

        assert_eq!(errors.len(), 3);
        assert_eq!(
            errors[0],
            ValidationError::TypeMismatch(
                "Age".to_string(),
                "number".to_string(),
                "string".to_string()
            )
        );
        assert_eq!(
            errors[1],
            ValidationError::TypeMismatch(
                "IsActive".to_string(),
                "boolean".to_string(),
                "string".to_string()
            )
        );
        assert_eq!(
            errors[2],
            ValidationError::TypeMismatch(
                "Name".to_string(),
                "string".to_string(),
                "number".to_string()
            )
        );
    }
}
