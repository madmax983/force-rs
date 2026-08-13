//! Apex class generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Preview utility to generate Apex classes from SObject describe metadata.
#[cfg(feature = "schema")]
/// Generates an Apex class definition from an SObject describe result.
pub fn generate_apex_class(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_apex_class(&mut out, describe);
    out
}

/// Writes an Apex class definition from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_apex_class(out: &mut String, describe: &SObjectDescribe) {
    out.push_str("/**\n");
    let _ = writeln!(out, " * {}", describe.label);
    out.push_str(" */\n");
    let _ = writeln!(out, "public class {} {{", describe.name);

    // Sort fields alphabetically, Id first
    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in fields {
        out.push_str("    /**\n");
        let _ = writeln!(out, "     * {}", field.label);
        if let Some(help) = &field.inline_help_text {
            let _ = writeln!(out, "     * {}", help);
        }
        out.push_str("     */\n");

        let apex_type = map_type(&field.type_);
        let _ = writeln!(out, "    @AuraEnabled");
        let _ = writeln!(
            out,
            "    public {} {} {{ get; set; }}",
            apex_type, field.name
        );
        out.push('\n');
    }

    out.push_str("}\n");
}

#[cfg(feature = "schema")]
fn map_type(field_type: &FieldType) -> &'static str {
    match field_type {
        FieldType::String
        | FieldType::Textarea
        | FieldType::Email
        | FieldType::Phone
        | FieldType::Url
        | FieldType::Picklist
        | FieldType::Multipicklist
        | FieldType::Combobox => "String",
        FieldType::Id | FieldType::Reference => "Id",
        FieldType::Boolean => "Boolean",
        FieldType::Int => "Integer",
        FieldType::Double | FieldType::Currency | FieldType::Percent => "Decimal",
        FieldType::Date => "Date",
        FieldType::Datetime => "Datetime",
        FieldType::Base64 => "Blob",
        _ => "Object",
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;

    #[test]
    fn test_generate_apex_class() {
        let describe_json_str = r#"{
            "name": "CustomObject__c",
            "label": "Custom Object",
            "custom": false,
            "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "001", "labelPlural": "Custom Objects", "layoutable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "fields": [
                {
                    "name": "Id",
                    "type": "id",
                    "label": "Record ID",
                    "referenceTo": [],
                    "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
                    "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "precision": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "searchPrefilterable": false, "sortable": true,
                    "soapType": "tns:ID", "unique": false, "writeRequiresMasterRead": false, "updateable": false
                },
                {
                    "name": "Name",
                    "type": "string",
                    "label": "Name",
                    "inlineHelpText": "The name of the object",
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
                    "name": "Amount__c",
                    "type": "currency",
                    "label": "Amount",
                    "referenceTo": [],
                    "aggregatable": false, "autoNumber": false, "byteLength": 255, "calculated": false,
                    "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": true,
                    "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": true,
                    "digits": 0, "precision": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": false, "groupable": false, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 255, "nameField": false, "namePointing": false, "nillable": true,
                    "permissionable": false, "polymorphicForeignKey": false, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "searchPrefilterable": false, "sortable": false,
                    "soapType": "xsd:string", "unique": false, "writeRequiresMasterRead": false, "updateable": false
                }
            ]
        }"#;

        let describe: SObjectDescribe = serde_json::from_str(describe_json_str).must();
        let apex = generate_apex_class(&describe);

        assert!(apex.contains("public class CustomObject__c {"));
        assert!(apex.contains("public Id Id { get; set; }"));
        assert!(apex.contains("public String Name { get; set; }"));
        assert!(apex.contains("public Decimal Amount__c { get; set; }"));
        assert!(apex.contains("The name of the object"));
        assert!(apex.contains("@AuraEnabled"));
    }
}
