//! BigQuery JSON schema generator for Salesforce SObject Describe metadata.
//!
//! Generates Google BigQuery table schema arrays based on Salesforce object schemas.

#[cfg(feature = "schema")]
use crate::api::rest::describe::{FieldType, SObjectDescribe};
#[cfg(feature = "schema")]
use serde_json::{Value, json};

/// Generates a Google BigQuery JSON schema definition for a given SObject.
#[cfg(feature = "schema")]
pub fn generate_bigquery_schema(describe: &SObjectDescribe) -> Value {
    let mut schema_fields = Vec::with_capacity(describe.fields.len());

    let mut sorted_fields: Vec<&_> = describe.fields.iter().collect();
    sorted_fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in sorted_fields {
        let bq_type = match field.type_ {
            FieldType::Int => "INTEGER",
            FieldType::Double | FieldType::Percent | FieldType::Currency => "FLOAT",
            FieldType::Boolean => "BOOLEAN",
            FieldType::Date => "DATE",
            FieldType::Datetime => "TIMESTAMP",
            FieldType::Time => "TIME",
            FieldType::Base64 => "BYTES",
            _ => "STRING",
        };

        let mode = if field.nillable {
            "NULLABLE"
        } else {
            "REQUIRED"
        };

        schema_fields.push(json!({
            "name": field.name,
            "type": bq_type,
            "mode": mode,
            "description": field.label
        }));
    }

    Value::Array(schema_fields)
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_support::{Must, MustMsg};

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_generate_bigquery_schema() {
        let json_data = r#"{
            "activateable": false,
            "createable": true,
            "custom": false,
            "customSetting": false,
            "deletable": true,
            "deprecatedAndHidden": false,
            "feedEnabled": false,
            "hasSubtypes": false,
            "isSubtype": false,
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
            "urls": {},
            "childRelationships": [], "recordTypeInfos": [], "fields": [
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
                },
                {
                    "aggregatable": true,
                    "autoNumber": false,
                    "byteLength": 765,
                    "calculated": false,
                    "cascadeDelete": false,
                    "caseSensitive": false,
                    "createable": true,
                    "custom": false,
                    "defaultedOnCreate": false,
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
                    "idLookup": false,
                    "label": "Account Name",
                    "length": 255,
                    "name": "Name",
                    "nameField": true,
                    "namePointing": false,
                    "nillable": false,
                    "permissionable": true,
                    "polymorphicForeignKey": false,
                    "precision": 0,
                    "queryByDistance": false,
                    "referenceTo": [],
                    "restrictedDelete": false,
                    "restrictedPicklist": false,
                    "scale": 0,
                    "soapType": "xsd:string",
                    "sortable": true,
                    "type": "string",
                    "unique": false,
                    "updateable": true,
                    "writeRequiresMasterRead": false
                },
                {
                    "aggregatable": true,
                    "autoNumber": false,
                    "byteLength": 0,
                    "calculated": false,
                    "cascadeDelete": false,
                    "caseSensitive": false,
                    "createable": true,
                    "custom": false,
                    "defaultedOnCreate": false,
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
                    "idLookup": false,
                    "label": "Employees",
                    "length": 0,
                    "name": "NumberOfEmployees",
                    "nameField": false,
                    "namePointing": false,
                    "nillable": true,
                    "permissionable": true,
                    "polymorphicForeignKey": false,
                    "precision": 8,
                    "queryByDistance": false,
                    "referenceTo": [],
                    "restrictedDelete": false,
                    "restrictedPicklist": false,
                    "scale": 0,
                    "soapType": "xsd:int",
                    "sortable": true,
                    "type": "int",
                    "unique": false,
                    "updateable": true,
                    "writeRequiresMasterRead": false
                }
            ]
        }"#;

        let describe: SObjectDescribe = serde_json::from_str(json_data).must();
        let schema = generate_bigquery_schema(&describe);

        let arr = schema.as_array().must_msg("Expected JSON array");
        assert_eq!(arr.len(), 3);

        // Sorting should guarantee order: Id, Name, NumberOfEmployees
        assert_eq!(arr[0]["name"], "Id");
        assert_eq!(arr[0]["type"], "STRING");
        assert_eq!(arr[0]["mode"], "REQUIRED");
        assert_eq!(arr[0]["description"], "Account ID");

        assert_eq!(arr[1]["name"], "Name");
        assert_eq!(arr[1]["type"], "STRING");
        assert_eq!(arr[1]["mode"], "REQUIRED");
        assert_eq!(arr[1]["description"], "Account Name");

        assert_eq!(arr[2]["name"], "NumberOfEmployees");
        assert_eq!(arr[2]["type"], "INTEGER");
        assert_eq!(arr[2]["mode"], "NULLABLE");
        assert_eq!(arr[2]["description"], "Employees");
    }
}
