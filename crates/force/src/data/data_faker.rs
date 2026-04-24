//! Salesforce Mock Data Generator.
//!
//! This module provides the `DataFaker` utility to automatically generate
//! mock `DynamicSObject` records based on an `SObjectDescribe` metadata payload.
//! This is extremely useful for generating test fixtures, populating sandboxes
//! with dummy data, and prototyping.
//!
//! # Example
//!
//! ```no_run
//! # use force::api::RestOperation;
//! # use force::client::ForceClientBuilder;
//! # use force::data::generate_mock_record;
//! # use force::auth::ClientCredentials;
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! # let auth = ClientCredentials::new("id", "secret", "url");
//! # let client = ForceClientBuilder::new().authenticate(auth).build().await?;
//! // Fetch the metadata for an Account
//! let describe = client.rest().describe("Account").await?;
//!
//! // Generate a fake record that conforms to the schema
//!
//! let fake_account = generate_mock_record(&describe);
//!
//! println!("Generated Account: {:?}", fake_account);
//! # Ok(())
//! # }
//! ```

use crate::types::describe::{FieldType, SObjectDescribe};
use crate::types::{Attributes, DynamicSObject, SalesforceId};

/// Utility for generating mock data based on Salesforce schema metadata.
///
/// Generates a mock `DynamicSObject` populated with fake data.
///
/// It iterates through all the fields defined in the `SObjectDescribe`
/// payload. If a field is `createable`, not `auto_number`, and not
/// `calculated`, it assigns a sensible default mock value based on the
/// field's `FieldType`.
///
/// # Arguments
///
/// * `describe` - The metadata describing the SObject's fields.
///
/// # Returns
///
/// A `DynamicSObject` containing mock values for createable fields.
#[must_use]
pub fn generate_mock_record(describe: &SObjectDescribe) -> DynamicSObject {
    // Create standard attributes with a dummy ID (for cases where you might upsert or just need an ID).
    // Since it's a new record creation payload normally it wouldn't have an ID,
    // but DynamicSObject requires Attributes to initialize.
    // We'll use a valid-looking 18 character dummy ID.
    let dummy_id = SalesforceId::new("001000000000000AAA").unwrap_or_else(|_| unreachable!());
    let attrs = Attributes::new(describe.name.clone(), &dummy_id, "v60.0");
    let mut record = DynamicSObject::new(attrs);

    for field in &describe.fields {
        // Only generate data for fields that we can actually create/insert.
        if !field.createable || field.auto_number || field.calculated {
            continue;
        }

        // Generate a sensible default mock value based on the field type
        match field.type_ {
            FieldType::String | FieldType::Id | FieldType::Reference | FieldType::AnyType => {
                record.set_field(&field.name, format!("Mock {}", field.label));
            }
            FieldType::Textarea | FieldType::Encryptedstring => {
                record.set_field(
                    &field.name,
                    format!("Detailed mock description for {}", field.label),
                );
            }
            FieldType::Int => {
                record.set_field(&field.name, 42);
            }
            FieldType::Double | FieldType::Currency | FieldType::Percent => {
                record.set_field(&field.name, 42.42);
            }
            FieldType::Boolean => {
                record.set_field(&field.name, true);
            }
            FieldType::Date => {
                record.set_field(&field.name, "2024-01-01");
            }
            FieldType::Datetime => {
                record.set_field(&field.name, "2024-01-01T12:00:00.000+0000");
            }
            FieldType::Time => {
                record.set_field(&field.name, "12:00:00.000Z");
            }
            FieldType::Email => {
                record.set_field(&field.name, "mock@example.com");
            }
            FieldType::Phone => {
                record.set_field(&field.name, "555-0100");
            }
            FieldType::Url => {
                record.set_field(&field.name, "https://example.com");
            }
            FieldType::Picklist | FieldType::Multipicklist | FieldType::Combobox => {
                // Try to use the first available picklist value if it exists
                if let Some(ref values) = field.picklist_values {
                    if let Some(first_active) = values.iter().find(|v| v.active) {
                        record.set_field(&field.name, &first_active.value);
                    } else if let Some(first) = values.first() {
                        record.set_field(&field.name, &first.value);
                    } else {
                        record.set_field(&field.name, "Mock Selection");
                    }
                } else {
                    record.set_field(&field.name, "Mock Selection");
                }
            }
            // Handle unsupported or complex types gracefully by ignoring them
            FieldType::Base64
            | FieldType::Datacategorygroupreference
            | FieldType::Location
            | FieldType::Address => {
                continue;
            }
        }
    }

    record
}

#[cfg(test)]
mod tests {
    use crate::test_support::Must;
    use super::*;
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
        createable: bool,
        auto_number: bool,
        calculated: bool,
    ) -> serde_json::Value {
        json!({
            "name": name,
            "type": field_type,
            "label": format!("{} Label", name),
            "referenceTo": [],
            "createable": createable,
            "autoNumber": auto_number,
            "calculated": calculated,
            // Mandatory fields filler
            "aggregatable": true, "byteLength": 18,
            "cascadeDelete": false, "caseSensitive": false, "custom": false,
            "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:ID",
            "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false
        })
    }

    #[test]
    fn test_generate_mock_record_basic() {
        let describe = create_mock_describe(&json!([
            mock_field("Id", "id", false, false, false),
            mock_field("Name", "string", true, false, false),
            mock_field("NumberOfEmployees", "int", true, false, false),
            mock_field("AnnualRevenue", "double", true, false, false),
            mock_field("IsActive", "boolean", true, false, false),
            mock_field("FormulaField", "string", true, false, true),
            mock_field("AutoNum", "string", true, true, false)
        ]));

        let record = generate_mock_record(&describe);

        // Id should not be present as it's not createable
        assert!(!record.has_field("Id"));

        // Name should be generated
        assert_eq!(
            record.get_field("Name").and_then(|v| v.as_str()),
            Some("Mock Name Label")
        );

        // Int field
        assert_eq!(
            record.get_field_as::<i32>("NumberOfEmployees").must(),
            Some(42)
        );

        // Double field
        assert_eq!(
            record.get_field_as::<f64>("AnnualRevenue").must(),
            Some(42.42)
        );

        // Boolean field
        assert_eq!(record.get_field_as::<bool>("IsActive").must(), Some(true));

        // Formula field should be ignored
        assert!(!record.has_field("FormulaField"));

        // AutoNumber field should be ignored
        assert!(!record.has_field("AutoNum"));
    }

    #[test]
    fn test_generate_mock_record_special_types() {
        let describe = create_mock_describe(&json!([
            mock_field("Email", "email", true, false, false),
            mock_field("Phone", "phone", true, false, false),
            mock_field("Website", "url", true, false, false),
            mock_field("Birthdate", "date", true, false, false)
        ]));

        let record = generate_mock_record(&describe);

        assert_eq!(
            record.get_field("Email").and_then(|v| v.as_str()),
            Some("mock@example.com")
        );
        assert_eq!(
            record.get_field("Phone").and_then(|v| v.as_str()),
            Some("555-0100")
        );
        assert_eq!(
            record.get_field("Website").and_then(|v| v.as_str()),
            Some("https://example.com")
        );
        assert_eq!(
            record.get_field("Birthdate").and_then(|v| v.as_str()),
            Some("2024-01-01")
        );
    }

    #[test]
    fn test_generate_mock_record_datetime_time_currency_percent() {
        let describe = create_mock_describe(&json!([
            mock_field("CreatedDate", "datetime", true, false, false),
            mock_field("CloseTime", "time", true, false, false),
            mock_field("AnnualRevenue", "currency", true, false, false),
            mock_field("Probability", "percent", true, false, false),
            mock_field("EncryptedField", "encryptedstring", true, false, false),
            mock_field("AnyField", "anyType", true, false, false),
            mock_field("LookupField", "reference", true, false, false),
            mock_field("Description", "textarea", true, false, false)
        ]));

        let record = generate_mock_record(&describe);

        assert_eq!(
            record.get_field("CreatedDate").and_then(|v| v.as_str()),
            Some("2024-01-01T12:00:00.000+0000")
        );
        assert_eq!(
            record.get_field("CloseTime").and_then(|v| v.as_str()),
            Some("12:00:00.000Z")
        );
        assert_eq!(
            record.get_field_as::<f64>("AnnualRevenue").must(),
            Some(42.42)
        );
        assert_eq!(
            record.get_field_as::<f64>("Probability").must(),
            Some(42.42)
        );
        assert_eq!(
            record.get_field("EncryptedField").and_then(|v| v.as_str()),
            Some("Detailed mock description for EncryptedField Label")
        );
        assert!(record.has_field("AnyField"));
        assert!(record.has_field("LookupField"));
        assert!(record.has_field("Description"));
    }

    #[test]
    fn test_generate_mock_record_picklists() {
        let mut field1 = mock_field("Pick1", "picklist", true, false, false);
        field1["picklistValues"] = json!([{"active": false, "value": "A", "defaultValue": false, "label": "A"}, {"active": true, "value": "B", "defaultValue": false, "label": "B"}]);

        let mut field2 = mock_field("Pick2", "multipicklist", true, false, false);
        field2["picklistValues"] = json!([{"active": false, "value": "X", "defaultValue": false, "label": "X"}, {"active": false, "value": "Y", "defaultValue": false, "label": "Y"}]);

        let mut field3 = mock_field("Pick3", "combobox", true, false, false);
        field3["picklistValues"] = json!([]);

        let field4 = mock_field("Pick4", "picklist", true, false, false);

        let field5 = mock_field("Base64F", "base64", true, false, false);
        let field6 = mock_field("LocF", "location", true, false, false);
        let field7 = mock_field("AddrF", "address", true, false, false);
        let field8 = mock_field("DataCatF", "datacategorygroupreference", true, false, false);

        let describe = create_mock_describe(&json!([
            field1, field2, field3, field4, field5, field6, field7, field8
        ]));
        let record = generate_mock_record(&describe);

        assert_eq!(
            record.get_field("Pick1").and_then(|v| v.as_str()),
            Some("B")
        );
        assert_eq!(
            record.get_field("Pick2").and_then(|v| v.as_str()),
            Some("X")
        );
        assert_eq!(
            record.get_field("Pick3").and_then(|v| v.as_str()),
            Some("Mock Selection")
        );
        assert_eq!(
            record.get_field("Pick4").and_then(|v| v.as_str()),
            Some("Mock Selection")
        );

        assert!(!record.has_field("Base64F"));
        assert!(!record.has_field("LocF"));
        assert!(!record.has_field("AddrF"));
        assert!(!record.has_field("DataCatF"));
    }
}

/// Utility for generating a mock SOQL query based on Salesforce schema metadata.
///
/// Generates a valid SOQL query selecting up to 50 createable fields
/// from the given SObject describe payload, appending a `LIMIT 10` clause.
///
/// # Arguments
///
/// * `describe` - The metadata describing the SObject's fields.
///
/// # Returns
///
/// A String representing a valid SOQL query.
#[must_use]
pub fn generate_mock_query(describe: &SObjectDescribe) -> String {
    let fields: Vec<String> = describe
        .fields
        .iter()
        .filter(|f| f.createable)
        .take(50)
        .map(|f| f.name.clone())
        .collect();

    crate::api::soql::SoqlQueryBuilder::new()
        .select(&fields)
        .from(&describe.name)
        .limit(10)
        .build()
}

#[cfg(test)]
mod additional_tests {
    use crate::test_support::Must;
    use super::*;
    use crate::types::describe::SObjectDescribe;
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
        createable: bool,
        auto_number: bool,
        calculated: bool,
    ) -> serde_json::Value {
        json!({
            "name": name,
            "type": field_type,
            "label": format!("{} Label", name),
            "createable": createable,
            "autoNumber": auto_number,
            "calculated": calculated,
            "aggregatable": true, "byteLength": 18,
            "cascadeDelete": false, "caseSensitive": false, "custom": false,
            "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "referenceTo": [], "restrictedDelete": false, "restrictedPicklist": false, "scale": 0,
            "soapType": "tns:ID", "sortable": true, "unique": false, "updateable": false,
            "writeRequiresMasterRead": false
        })
    }

    #[test]
    fn test_generate_mock_query() {
        let describe = create_mock_describe(&json!([
            mock_field("Name", "string", true, false, false),
            mock_field("CustomField__c", "string", true, false, false),
            mock_field("NotCreateable", "string", false, false, false)
        ]));

        let query = generate_mock_query(&describe);

        assert_eq!(query, "SELECT Name, CustomField__c FROM Account LIMIT 10");
    }
}
