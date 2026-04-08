//! Protocol Buffers schema generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::api::rest::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates a Protocol Buffers schema definition from an SObject describe result.
#[cfg(feature = "schema")]
#[must_use]
pub fn generate_protobuf_schema(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_protobuf_schema(&mut out, describe);
    out
}

/// Writes a Protocol Buffers schema definition from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_protobuf_schema(out: &mut String, describe: &SObjectDescribe) {
    let _ = writeln!(out, "syntax = \"proto3\";");
    let _ = writeln!(out, "package com.salesforce.schema;");
    let _ = writeln!(out);
    let _ = writeln!(out, "message {} {{", describe.name);

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| {
        if a.name == "Id" && b.name == "Id" {
            std::cmp::Ordering::Equal
        } else if a.name == "Id" {
            std::cmp::Ordering::Less
        } else if b.name == "Id" {
            std::cmp::Ordering::Greater
        } else {
            a.name.cmp(&b.name)
        }
    });

    let mut field_number = 1;
    for field in fields {
        let proto_type = map_type(&field.type_);

        let optional_modifier = if field.nillable { "optional " } else { "" };

        let _ = writeln!(
            out,
            "  {}{} {} = {};",
            optional_modifier, proto_type, field.name, field_number
        );
        field_number += 1;
    }

    let _ = writeln!(out, "}}");
}

/// Maps a Salesforce `FieldType` to a Protocol Buffers scalar type.
#[cfg(feature = "schema")]
fn map_type(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Boolean => "bool",
        FieldType::Int => "int32",
        FieldType::Double | FieldType::Currency | FieldType::Percent => "double",
        // Most Salesforce types serialize as strings
        _ => "string",
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;

    use crate::test_support::MustMsg;

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_protobuf_generator() {
        // Use JSON to instantiate the complex SObjectDescribe struct securely against layout changes.
        let describe_json = r#"{
            "activateable": false,
            "createable": true,
            "custom": false,
            "customSetting": false,
            "deletable": true,
            "deprecatedAndHidden": false,
            "feedEnabled": false,
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
            "urls": {},
            "childRelationships": [],
            "recordTypeInfos": [],
            "fields": [
                {
                    "name": "Id", "type": "id", "label": "Id", "nillable": false,
                    "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
                    "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 18, "nameField": false, "namePointing": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:ID",
                    "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false,
                    "referenceTo": []
                },
                {
                    "name": "Name", "type": "string", "label": "Name", "nillable": false,
                    "aggregatable": true, "autoNumber": false, "byteLength": 255, "calculated": false,
                    "cascadeDelete": false, "caseSensitive": false, "createable": true, "custom": false,
                    "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 255, "nameField": true, "namePointing": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false,
                    "referenceTo": []
                },
                {
                    "name": "NumberOfEmployees", "type": "int", "label": "Employees", "nillable": true,
                    "aggregatable": true, "autoNumber": false, "byteLength": 0, "calculated": false,
                    "cascadeDelete": false, "caseSensitive": false, "createable": true, "custom": false,
                    "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 8, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 0, "nameField": false, "namePointing": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:int",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false,
                    "referenceTo": []
                },
                {
                    "name": "AnnualRevenue", "type": "currency", "label": "Annual Revenue", "nillable": true,
                    "aggregatable": true, "autoNumber": false, "byteLength": 0, "calculated": false,
                    "cascadeDelete": false, "caseSensitive": false, "createable": true, "custom": false,
                    "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 18, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 0, "nameField": false, "namePointing": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 18, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:double",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false,
                    "referenceTo": []
                },
                {
                    "name": "IsActive", "type": "boolean", "label": "Active", "nillable": true,
                    "aggregatable": true, "autoNumber": false, "byteLength": 0, "calculated": false,
                    "cascadeDelete": false, "caseSensitive": false, "createable": true, "custom": false,
                    "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 0, "nameField": false, "namePointing": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:boolean",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false,
                    "referenceTo": []
                }
            ]
        }"#;

        let describe: SObjectDescribe = serde_json::from_str(describe_json).must_msg("failed to parse mock json");

        let proto_code = generate_protobuf_schema(&describe);

        let expected = r#"syntax = "proto3";
package com.salesforce.schema;

message Account {
  string Id = 1;
  optional double AnnualRevenue = 2;
  optional bool IsActive = 3;
  string Name = 4;
  optional int32 NumberOfEmployees = 5;
}
"#;
        assert_eq!(proto_code, expected);
    }
}
