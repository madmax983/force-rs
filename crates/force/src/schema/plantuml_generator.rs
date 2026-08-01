//! PlantUML Class Diagram Generator.
//!
//! This module provides a utility to generate PlantUML Class Diagrams
//! for Salesforce SObjects, turning Describe API metadata into
//! visual architecture documentation.

use crate::error::Result;
use crate::types::describe::SObjectDescribe;
use std::io::Write;

/// Generates a PlantUML Class Diagram for a Salesforce SObject.
///
/// This exports the schema metadata into a PlantUML format suitable for
/// visual documentation and system architecture design.
///
/// It correctly flags fields:
/// - `+` for Custom fields
/// - `-` for Standard fields
/// - `*` for Required fields
/// - Relationship lines for Reference types
#[cfg(feature = "schema")]
pub fn generate_plantuml<W: Write>(describe: &SObjectDescribe, mut writer: W) -> Result<()> {
    writeln!(writer, "@startuml")?;
    writeln!(writer, "class {} {{", describe.name)?;

    // Output fields
    for field in &describe.fields {
        let visibility = if field.custom { "+" } else { "-" };
        let required = if !field.nillable && field.createable && !field.defaulted_on_create {
            " *"
        } else {
            ""
        };
        let type_str = format!("{:?}", field.type_).to_lowercase();
        writeln!(
            writer,
            "  {} {}: {}{}",
            visibility, field.name, type_str, required
        )?;
    }

    writeln!(writer, "}}")?;

    // Output relationships (ReferenceTo)
    for field in &describe.fields {
        if !field.reference_to.is_empty() {
            for ref_obj in &field.reference_to {
                writeln!(
                    writer,
                    "{} --> \"1\" {} : {}",
                    describe.name, ref_obj, field.name
                )?;
            }
        }
    }

    writeln!(writer, "@enduml")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;
    use std::io::Cursor;

    fn mock_describe() -> SObjectDescribe {
        let describe_json = serde_json::json!({
            "name": "Account",
            "label": "Account",
            "custom": false,
            "keyPrefix": "001",
            "labelPlural": "Accounts",
            "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": [
                {
                    "name": "Id",
                    "type": "id",
                    "label": "Account ID",
                    "referenceTo": [],
                    "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
                    "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "precision": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "searchPrefilterable": false, "sortable": true,
                    "soapType": "tns:ID", "unique": true, "writeRequiresMasterRead": false, "updateable": false
                },
                {
                    "name": "Name",
                    "type": "string",
                    "label": "Account Name",
                    "referenceTo": [],
                    "aggregatable": true, "autoNumber": false, "byteLength": 255, "calculated": false,
                    "cascadeDelete": false, "caseSensitive": false, "createable": true, "custom": false,
                    "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "precision": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 255, "nameField": true, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "searchPrefilterable": false, "sortable": true,
                    "soapType": "xsd:string", "unique": false, "writeRequiresMasterRead": false, "updateable": true
                },
                {
                    "name": "ParentId",
                    "type": "reference",
                    "label": "Parent Account ID",
                    "referenceTo": ["Account"],
                    "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
                    "cascadeDelete": false, "caseSensitive": false, "createable": true, "custom": false,
                    "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "precision": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 18, "nameField": false, "namePointing": false, "nillable": true,
                    "permissionable": false, "polymorphicForeignKey": false, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "searchPrefilterable": false, "sortable": true,
                    "soapType": "tns:ID", "unique": false, "writeRequiresMasterRead": false, "updateable": true
                },
                {
                    "name": "CustomField__c",
                    "type": "string",
                    "label": "Custom Field",
                    "referenceTo": [],
                    "aggregatable": true, "autoNumber": false, "byteLength": 255, "calculated": false,
                    "cascadeDelete": false, "caseSensitive": false, "createable": true, "custom": true,
                    "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "precision": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 255, "nameField": false, "namePointing": false, "nillable": true,
                    "permissionable": false, "polymorphicForeignKey": false, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "searchPrefilterable": false, "sortable": true,
                    "soapType": "xsd:string", "unique": false, "writeRequiresMasterRead": false, "updateable": true
                }
            ]
        });
        serde_json::from_value(describe_json).must()
    }

    #[test]
    fn test_generate_plantuml() {
        let describe = mock_describe();

        let mut buf = Cursor::new(Vec::new());
        generate_plantuml(&describe, &mut buf).must();

        let puml = String::from_utf8(buf.into_inner()).must();

        assert!(puml.contains("@startuml"));
        assert!(puml.contains("class Account {"));
        assert!(puml.contains("  - Id: id"));
        assert!(puml.contains("  - Name: string *"));
        assert!(puml.contains("  - ParentId: reference"));
        assert!(puml.contains("  + CustomField__c: string"));
        assert!(puml.contains("}"));
        assert!(puml.contains("Account --> \"1\" Account : ParentId"));
        assert!(puml.contains("@enduml"));
    }
}
