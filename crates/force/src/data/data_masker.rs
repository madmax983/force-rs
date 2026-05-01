//! Automated PII Sanitization for Salesforce Records.
//!
//! This module provides the `DataMasker` utility to automatically identify and
//! redact sensitive data in `DynamicSObject` records based on the object's
//! schema metadata. It acts as a bridge between schema awareness and data processing.
//!
//! # The Spark
//! We have `DataArchiver` to export data and `DataFaker` to generate dummy data.
//! But what if developers need to export *production* data for local testing
//! without leaking PII? `DataMasker` uses heuristics (field types, encrypted flags,
//! name patterns) to sanitize sensitive fields on the fly!
//!
//! # Example
//!
//! ```no_run
//! # use force::api::RestOperation;
//! # use force::client::ForceClientBuilder;
//! # use force::data::DataMasker;
//! # use force::auth::ClientCredentials;
//! # use force::types::DynamicSObject;
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! # let auth = ClientCredentials::new("id", "secret", "url");
//! # let client = ForceClientBuilder::new().authenticate(auth).build().await?;
//! // Fetch metadata for Contact
//! let describe = client.rest().describe("Contact").await?;
//!
//! let masker = DataMasker::new(&describe);
//!
//! // Assume we have a record from a query
//! let mut contact: DynamicSObject = /* ... */
//! # return Ok(());
//! masker.mask_record(&mut contact);
//!
//! println!("Masked Email: {:?}", contact.get_field_as::<String>("Email").unwrap());
//! // Outputs: Some("***@***.***")
//! # Ok(())
//! # }
//! ```

use crate::types::DynamicSObject;
use crate::types::describe::{FieldDescribe, FieldType, SObjectDescribe};
use serde_json::Value;

/// Utility for masking sensitive fields in SObject records.
#[derive(Debug, Clone)]
pub struct DataMasker<'a> {
    describe: &'a SObjectDescribe,
}

impl<'a> DataMasker<'a> {
    /// Creates a new `DataMasker` initialized with the target schema.
    #[must_use]
    pub fn new(describe: &'a SObjectDescribe) -> Self {
        Self { describe }
    }

    /// Mutates the given record in-place by masking fields identified as sensitive.
    ///
    /// It iterates through the record's keys, checks the schema for the corresponding
    /// field, and if it's sensitive (e.g., Email, Phone, Encrypted), replaces the
    /// value with a masked version.
    pub fn mask_record(&self, record: &mut DynamicSObject) {
        // ⚡ Bolt: Iterate over fields to find sensitive ones, avoiding cloning all keys into a new Vec
        let mut to_update = Vec::new();

        for (key, val) in &record.fields {
            if val.is_null() {
                continue;
            }

            if let Some(field) = self.find_field(key) {
                if Self::is_sensitive(field) {
                    let masked_val = Self::generate_mask(field, val);
                    to_update.push((key.clone(), masked_val));
                }
            }
        }

        for (key, val) in to_update {
            record.set_field(&key, val);
        }
    }

    /// Finds a field definition by name (case-insensitive).
    fn find_field(&self, name: &str) -> Option<&FieldDescribe> {
        self.describe
            .fields
            .iter()
            .find(|f| f.name.eq_ignore_ascii_case(name))
    }

    /// Determines if a field should be considered sensitive.
    ///
    /// Uses several heuristics:
    /// - Explicit `encrypted` flag
    /// - Field types (Email, Phone)
    /// - Name-based patterns (SSN, Password, CreditCard)
    fn is_sensitive(field: &FieldDescribe) -> bool {
        if field.encrypted {
            return true;
        }

        match field.type_ {
            FieldType::Email | FieldType::Phone => return true,
            _ => {}
        }

        let name_lower = field.name.to_ascii_lowercase();
        if name_lower.contains("ssn")
            || name_lower.contains("password")
            || name_lower.contains("creditcard")
            || name_lower.contains("secret")
        {
            return true;
        }

        false
    }

    /// Generates a masked value appropriate for the field type.
    fn generate_mask(field: &FieldDescribe, _original: &Value) -> Value {
        match field.type_ {
            FieldType::Email => Value::String("***@***.***".to_string()),
            FieldType::Phone => Value::String("***-***-****".to_string()),
            FieldType::Int | FieldType::Double | FieldType::Currency | FieldType::Percent => {
                Value::Number(serde_json::Number::from(0))
            }
            FieldType::Boolean => Value::Bool(false),
            _ => Value::String("********".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;
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

    fn mock_field(name: &str, field_type: &str, encrypted: bool) -> serde_json::Value {
        json!({
            "name": name,
            "type": field_type,
            "label": format!("{} Label", name),
            "referenceTo": [],
            "encrypted": encrypted,
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
        let attrs = Attributes::new("Contact", &id, "v60.0");
        let mut record = DynamicSObject::new(attrs);
        record.fields = fields;
        record
    }

    #[test]
    fn test_mask_record() {
        let describe = create_mock_describe(&json!([
            mock_field("FirstName", "string", false),
            mock_field("Email", "email", false),
            mock_field("Phone", "phone", false),
            mock_field("SSN__c", "string", false),
            mock_field("SecretField", "string", true),
            mock_field("Revenue", "currency", true)
        ]));

        let masker = DataMasker::new(&describe);

        let mut record_fields = serde_json::Map::new();
        record_fields.insert("FirstName".to_string(), json!("John"));
        record_fields.insert("Email".to_string(), json!("john.doe@example.com"));
        record_fields.insert("Phone".to_string(), json!("123-456-7890"));
        record_fields.insert("SSN__c".to_string(), json!("000-11-2222"));
        record_fields.insert("SecretField".to_string(), json!("top_secret"));
        record_fields.insert("Revenue".to_string(), json!(50000.0));

        let mut record = create_mock_record(record_fields);

        masker.mask_record(&mut record);

        // Safe field remains untouched
        assert_eq!(
            record.get_field_as::<String>("FirstName").must().must(),
            "John"
        );

        // Type-based masking
        assert_eq!(
            record.get_field_as::<String>("Email").must().must(),
            "***@***.***"
        );
        assert_eq!(
            record.get_field_as::<String>("Phone").must().must(),
            "***-***-****"
        );

        // Name-heuristic masking
        assert_eq!(
            record.get_field_as::<String>("SSN__c").must().must(),
            "********"
        );

        // Encrypted-flag masking
        assert_eq!(
            record.get_field_as::<String>("SecretField").must().must(),
            "********"
        );

        let revenue = record.get_field_as::<f64>("Revenue").must().must();
        assert!((revenue - 0.0).abs() < f64::EPSILON);
    }
}
