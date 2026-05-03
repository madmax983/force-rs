use crate::types::describe::{FieldType, SObjectDescribe};
use serde_json::{Map, Value};

/// A utility to anonymize Salesforce record data based on its schema metadata.
#[derive(Debug)]
pub struct DataAnonymizer<'a> {
    describe: &'a SObjectDescribe,
}

impl<'a> DataAnonymizer<'a> {
    /// Creates a new `DataAnonymizer` for the given SObject describe.
    #[must_use]
    pub fn new(describe: &'a SObjectDescribe) -> Self {
        Self { describe }
    }

    /// Anonymizes a JSON representation of a Salesforce record.
    ///
    /// Sensitive fields (Emails, Phones, URLs, and encrypted fields) are redacted.
    #[must_use]
    pub fn anonymize(&self, record: &Value) -> Value {
        let Value::Object(map) = record else {
            return record.clone();
        };

        let mut anonymized = Map::new();

        for (key, value) in map {
            if let Some(field) = self.describe.fields.iter().find(|f| &f.name == key) {
                if field.encrypted {
                    anonymized.insert(key.clone(), Value::String("***REDACTED***".to_string()));
                    continue;
                }

                let new_val = match field.type_ {
                    FieldType::Email => Value::String("redacted@example.com".to_string()),
                    FieldType::Phone => Value::String("XXX-XXX-XXXX".to_string()),
                    FieldType::Url => Value::String("https://redacted.com".to_string()),
                    _ => value.clone(),
                };
                anonymized.insert(key.clone(), new_val);
            } else {
                anonymized.insert(key.clone(), value.clone());
            }
        }

        Value::Object(anonymized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_mock_describe() -> SObjectDescribe {
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
            "fields": [
                {
                    "name": "FirstName",
                    "type": "string",
                    "label": "First Name",
                    "referenceTo": [],
                    "custom": false,
                    "nillable": true,
                    "defaultedOnCreate": false,
                    "calculated": false,
                    "createable": true, "autoNumber": false, "aggregatable": true, "byteLength": 255,
                    "cascadeDelete": false, "caseSensitive": false,
                    "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 255, "nameField": false, "namePointing": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false
                },
                {
                    "name": "Email",
                    "type": "email",
                    "label": "Email",
                    "referenceTo": [],
                    "custom": false,
                    "nillable": true,
                    "defaultedOnCreate": false,
                    "calculated": false,
                    "createable": true, "autoNumber": false, "aggregatable": true, "byteLength": 80,
                    "cascadeDelete": false, "caseSensitive": false,
                    "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 80, "nameField": false, "namePointing": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false
                },
                {
                    "name": "Secret__c",
                    "type": "string",
                    "label": "Secret",
                    "referenceTo": [],
                    "custom": true,
                    "nillable": true,
                    "defaultedOnCreate": false,
                    "calculated": false,
                    "createable": true, "autoNumber": false, "aggregatable": true, "byteLength": 255,
                    "cascadeDelete": false, "caseSensitive": false,
                    "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": true, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 255, "nameField": false, "namePointing": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false
                }
            ]
        });
        serde_json::from_value(describe_json).unwrap_or_else(|_| panic!("Failed to create mock describe"))
    }

    #[test]
    fn test_anonymize_record() {
        let describe = create_mock_describe();
        let anonymizer = DataAnonymizer::new(&describe);

        let record = json!({
            "FirstName": "John",
            "Email": "john.doe@example.com",
            "Secret__c": "MySuperSecretData",
            "UnknownField": "Kept"
        });

        let result = anonymizer.anonymize(&record);

        assert_eq!(result["FirstName"], "John");
        assert_eq!(result["Email"], "redacted@example.com");
        assert_eq!(result["Secret__c"], "***REDACTED***");
        assert_eq!(result["UnknownField"], "Kept");
    }
}
