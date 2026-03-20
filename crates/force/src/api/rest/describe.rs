//! Salesforce Describe API for retrieving object metadata.
//!
//! Type definitions live in [`crate::types::describe`]. This module re-exports them
//! for backward compatibility and contains the tests for deserialization.

pub use crate::types::describe::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Must;

    // RED PHASE - Write failing tests first

    #[test]
    fn test_global_sobject_describe_deserialize() {
        let json = r#"{
            "activateable": false,
            "mergeable": false,
            "queryable": true,
            "replicateable": true,
            "retrieveable": true,
            "searchable": true,
            "triggerable": true,
            "undeletable": true,
            "name": "Account",
            "label": "Account",
            "labelPlural": "Accounts",
            "keyPrefix": "001",
            "custom": false,
            "urls": {
                "sobject": "/services/data/v60.0/sobjects/Account"
            }
        }"#;

        let sobject: GlobalSObjectDescribe = serde_json::from_str(json).must();
        assert_eq!(sobject.name, "Account");
        assert_eq!(sobject.label, "Account");
        assert_eq!(sobject.key_prefix, Some("001".to_string()));
        assert!(sobject.queryable);
        assert!(!sobject.custom);
    }

    #[test]
    fn test_global_describe_deserialize() {
        let json = r#"{
            "encoding": "UTF-8",
            "maxBatchSize": 200,
            "sobjects": [
                {
                    "activateable": false,
                    "mergeable": false,
                    "queryable": true,
                    "replicateable": true,
                    "retrieveable": true,
                    "searchable": true,
                    "triggerable": true,
                    "undeletable": true,
                    "name": "Account",
                    "label": "Account",
                    "labelPlural": "Accounts",
                    "keyPrefix": "001",
                    "custom": false,
                    "urls": {}
                }
            ]
        }"#;

        let global: GlobalDescribe = serde_json::from_str(json).must();
        assert_eq!(global.encoding, "UTF-8");
        assert_eq!(global.max_batch_size, 200);
        assert_eq!(global.sobjects.len(), 1);
        assert_eq!(global.sobjects[0].name, "Account");
    }

    #[test]
    fn test_field_type_deserialize() {
        let types = vec![
            (r#""string""#, FieldType::String),
            (r#""picklist""#, FieldType::Picklist),
            (r#""boolean""#, FieldType::Boolean),
            (r#""reference""#, FieldType::Reference),
            (r#""datetime""#, FieldType::Datetime),
            (r#""currency""#, FieldType::Currency),
        ];

        for (json, expected) in types {
            let field_type: FieldType = serde_json::from_str(json).must();
            assert_eq!(field_type, expected);
        }
    }

    #[test]
    fn test_picklist_value_deserialize() {
        let json = r#"{
            "active": true,
            "defaultValue": false,
            "label": "Hot",
            "value": "Hot"
        }"#;

        let value: PicklistValue = serde_json::from_str(json).must();
        assert!(value.active);
        assert!(!value.default_value);
        assert_eq!(value.label, "Hot");
        assert_eq!(value.value, "Hot");
    }

    #[test]
    fn test_child_relationship_deserialize() {
        let json = r#"{
            "cascadeDelete": false,
            "childSObject": "Contact",
            "deprecatedAndHidden": false,
            "field": "AccountId",
            "relationshipName": "Contacts",
            "restrictedDelete": false
        }"#;

        let rel: ChildRelationship = serde_json::from_str(json).must();
        assert!(!rel.cascade_delete);
        assert_eq!(rel.child_sobject, "Contact");
        assert_eq!(rel.field, "AccountId");
        assert_eq!(rel.relationship_name, Some("Contacts".to_string()));
    }

    #[test]
    fn test_record_type_info_deserialize() {
        let json = r#"{
            "active": true,
            "defaultRecordTypeMapping": true,
            "developerName": "Master",
            "master": true,
            "name": "Master",
            "recordTypeId": "012000000000000AAA"
        }"#;

        let rt: RecordTypeInfo = serde_json::from_str(json).must();
        assert!(rt.active);
        assert!(rt.default_record_type_mapping);
        assert!(rt.master);
        assert_eq!(rt.name, "Master");
    }

    #[test]
    fn test_sobject_describe_deserialize() {
        // A minimized but representative JSON payload for an SObject
        let json = r#"{
            "activateable": false,
            "createable": true,
            "custom": false,
            "customSetting": false,
            "deletable": true,
            "deprecatedAndHidden": false,
            "feedEnabled": true,
            "hasSubtypes": false,
            "isSubtype": false,
            "keyPrefix": "001",
            "label": "Account",
            "labelPlural": "Accounts",
            "layoutable": true,
            "mergeable": true,
            "mruEnabled": true,
            "name": "Account",
            "queryable": true,
            "replicateable": true,
            "retrieveable": true,
            "searchable": true,
            "triggerable": true,
            "undeletable": true,
            "updateable": true,
            "urls": {
                "sobject": "/services/data/v60.0/sobjects/Account"
            },
            "fields": [
                {
                    "aggregatable": true,
                    "autoNumber": false,
                    "byteLength": 18,
                    "calculated": false,
                    "cascadeDelete": false,
                    "caseSensitive": false,
                    "createable": false,
                    "custom": false,
                    "defaultedOnCreate": true,
                    "dependentPicklist": false,
                    "deprecatedAndHidden": false,
                    "digits": 0,
                    "displayLocationInDecimal": false,
                    "encrypted": false,
                    "externalId": false,
                    "filterable": true,
                    "groupable": true,
                    "highScaleNumber": false,
                    "htmlFormatted": false,
                    "idLookup": true,
                    "label": "Account ID",
                    "length": 18,
                    "name": "Id",
                    "nameField": false,
                    "namePointing": false,
                    "nillable": false,
                    "permissionable": false,
                    "polymorphicForeignKey": false,
                    "precision": 0,
                    "queryByDistance": false,
                    "referenceTo": [],
                    "restrictedDelete": false,
                    "restrictedPicklist": false,
                    "scale": 0,
                    "soapType": "tns:ID",
                    "sortable": true,
                    "type": "id",
                    "unique": false,
                    "updateable": false,
                    "writeRequiresMasterRead": false
                }
            ],
            "childRelationships": [],
            "recordTypeInfos": []
        }"#;

        let describe: SObjectDescribe = serde_json::from_str(json).must();

        assert_eq!(describe.name, "Account");
        assert_eq!(describe.label, "Account");
        assert!(!describe.custom);
        assert!(describe.queryable);
        assert_eq!(describe.key_prefix, Some("001".to_string()));

        assert_eq!(describe.fields.len(), 1);
        let id_field = &describe.fields[0];
        assert_eq!(id_field.name, "Id");
        assert_eq!(id_field.type_, FieldType::Id);
        assert_eq!(id_field.length, 18);
        assert!(!id_field.custom);
        assert!(id_field.id_lookup);
    }

    #[test]
    fn test_field_describe_optional_handling() {
        // JSON with minimal required fields to verify Option<T> handling
        let json = r#"{
            "aggregatable": true,
            "autoNumber": false,
            "byteLength": 18,
            "calculated": false,
            "cascadeDelete": false,
            "caseSensitive": false,
            "createable": false,
            "custom": false,
            "defaultedOnCreate": true,
            "dependentPicklist": false,
            "deprecatedAndHidden": false,
            "digits": 0,
            "displayLocationInDecimal": false,
            "encrypted": false,
            "externalId": false,
            "filterable": true,
            "groupable": true,
            "highScaleNumber": false,
            "htmlFormatted": false,
            "idLookup": true,
            "label": "Account ID",
            "length": 18,
            "name": "Id",
            "nameField": false,
            "namePointing": false,
            "nillable": false,
            "permissionable": false,
            "polymorphicForeignKey": false,
            "precision": 0,
            "queryByDistance": false,
            "referenceTo": [],
            "restrictedDelete": false,
            "restrictedPicklist": false,
            "scale": 0,
            "soapType": "tns:ID",
            "sortable": true,
            "type": "id",
            "unique": false,
            "updateable": false,
            "writeRequiresMasterRead": false
        }"#;

        let field: FieldDescribe = serde_json::from_str(json).must();

        assert_eq!(field.name, "Id");
        assert!(field.calculated_formula.is_none());
        assert!(field.default_value.is_none());
        assert!(field.picklist_values.is_none());
        assert!(field.reference_target_field.is_none());
        assert!(field.relationship_name.is_none());
    }
}
