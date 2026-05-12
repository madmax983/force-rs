//! Elasticsearch Mapping Generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
#[cfg(feature = "schema")]
use serde_json::{Value, json};

/// Generates an Elasticsearch mapping for a Salesforce SObject describe metadata.
///
/// Maps Salesforce field types to standard Elasticsearch data types.
#[cfg(feature = "schema")]
pub fn generate_elasticsearch_mapping(describe: &SObjectDescribe) -> Value {
    let mut properties = serde_json::Map::new();

    for field in &describe.fields {
        let es_type = match field.type_ {
            FieldType::String
            | FieldType::Textarea
            | FieldType::Email
            | FieldType::Phone
            | FieldType::Url => "text",
            FieldType::Boolean => "boolean",
            FieldType::Int => "integer",
            FieldType::Double | FieldType::Currency | FieldType::Percent => "double",
            FieldType::Date | FieldType::Datetime => "date",
            // Includes Id, Reference, Picklist, Multipicklist, and complex types.
            _ => "keyword",
        };

        properties.insert(field.name.clone(), json!({ "type": es_type }));
    }

    json!({
        "mappings": {
            "properties": properties
        }
    })
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;

    #[test]
    fn test_generate_elasticsearch_mapping() {
        let describe_json = r#"{
            "activateable": false, "createable": true, "custom": false, "customSetting": false,
            "deletable": true, "deprecatedAndHidden": false, "feedEnabled": true,
            "hasSubtypes": false, "isSubtype": false, "label": "Account", "labelPlural": "Accounts",
            "layoutable": true, "mergeable": true, "mruEnabled": true, "name": "Account",
            "queryable": true, "replicateable": true, "retrieveable": true, "searchable": true,
            "triggerable": true, "undeletable": true, "updateable": true, "urls": {},
            "childRelationships": [], "recordTypeInfos": [], "fields": [
                {
                    "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
                    "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "label": "Account ID", "length": 18, "name": "Id", "nameField": false,
                    "namePointing": false, "nillable": false, "permissionable": false,
                    "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "referenceTo": [], "restrictedDelete": false, "restrictedPicklist": false, "scale": 0,
                    "soapType": "xsd:id", "sortable": true, "type": "id", "unique": true, "updateable": false,
                    "writeRequiresMasterRead": false
                },
                {
                    "aggregatable": true, "autoNumber": false, "byteLength": 255, "calculated": false,
                    "cascadeDelete": false, "caseSensitive": false, "createable": true, "custom": false,
                    "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "label": "Account Name", "length": 255, "name": "Name", "nameField": true,
                    "namePointing": false, "nillable": false, "permissionable": false,
                    "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "referenceTo": [], "restrictedDelete": false, "restrictedPicklist": false, "scale": 0,
                    "soapType": "xsd:string", "sortable": true, "type": "string", "unique": false, "updateable": true,
                    "writeRequiresMasterRead": false
                },
                {
                    "aggregatable": true, "autoNumber": false, "byteLength": 4, "calculated": false,
                    "cascadeDelete": false, "caseSensitive": false, "createable": true, "custom": false,
                    "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 8, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "label": "Employees", "length": 0, "name": "NumberOfEmployees", "nameField": false,
                    "namePointing": false, "nillable": true, "permissionable": false,
                    "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "referenceTo": [], "restrictedDelete": false, "restrictedPicklist": false, "scale": 0,
                    "soapType": "xsd:int", "sortable": true, "type": "int", "unique": false, "updateable": true,
                    "writeRequiresMasterRead": false
                },
                {
                    "aggregatable": true, "autoNumber": false, "byteLength": 8, "calculated": false,
                    "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "label": "Created Date", "length": 0, "name": "CreatedDate", "nameField": false,
                    "namePointing": false, "nillable": false, "permissionable": false,
                    "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "referenceTo": [], "restrictedDelete": false, "restrictedPicklist": false, "scale": 0,
                    "soapType": "xsd:dateTime", "sortable": true, "type": "datetime", "unique": false, "updateable": false,
                    "writeRequiresMasterRead": false
                }
            ]
        }"#;

        let describe: SObjectDescribe = serde_json::from_str(describe_json).must();
        let mapping = generate_elasticsearch_mapping(&describe);

        assert_eq!(mapping["mappings"]["properties"]["Id"]["type"], "keyword");
        assert_eq!(mapping["mappings"]["properties"]["Name"]["type"], "text");
        assert_eq!(
            mapping["mappings"]["properties"]["NumberOfEmployees"]["type"],
            "integer"
        );
        assert_eq!(
            mapping["mappings"]["properties"]["CreatedDate"]["type"],
            "date"
        );
    }
}
