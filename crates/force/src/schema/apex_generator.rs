//! Apex Class Generator for Salesforce SObject Describe metadata.

#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
#[cfg(feature = "schema")]
use std::fmt::Write;

/// Generates an Apex wrapper class definition from an SObject describe result.
#[cfg(feature = "schema")]
pub fn generate_apex_class(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 64);
    write_apex_class(&mut out, describe);
    out
}

/// Writes an Apex wrapper class definition directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_apex_class(out: &mut String, describe: &SObjectDescribe) {
    let _ = writeln!(out, "/**");
    let _ = writeln!(out, " * Wrapper class for {}", describe.name);
    let _ = writeln!(out, " */");
    let _ = writeln!(out, "public class {} {{", describe.name);

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in fields {
        let apex_type = map_type(&field.type_);
        let _ = writeln!(out, "    @AuraEnabled");
        let _ = writeln!(
            out,
            "    public {} {} {{ get; set; }}",
            apex_type, field.name
        );
    }

    let _ = writeln!(out, "}}");
}

#[cfg(feature = "schema")]
fn map_type(field_type: &FieldType) -> &'static str {
    match field_type {
        FieldType::Id | FieldType::Reference => "Id",
        FieldType::String
        | FieldType::Textarea
        | FieldType::Picklist
        | FieldType::Multipicklist
        | FieldType::Combobox
        | FieldType::Email
        | FieldType::Phone
        | FieldType::Url
        | FieldType::Encryptedstring
        | FieldType::AnyType
        | FieldType::Datacategorygroupreference
        | FieldType::Base64 => "String",
        FieldType::Boolean => "Boolean",
        FieldType::Int => "Integer",
        FieldType::Double | FieldType::Currency | FieldType::Percent => "Decimal",
        FieldType::Date => "Date",
        FieldType::Datetime => "Datetime",
        FieldType::Time => "Time",
        FieldType::Address => "Address",
        FieldType::Location => "Location",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "schema")]
    use crate::test_utils::must::Must;

    #[test]
    #[cfg(feature = "schema")]
    fn test_generate_apex_class() {
        let describe_json = r#"{
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
            "fields": [
                {
                    "name": "Id", "type": "id", "label": "Id", "createable": false,
                    "autoNumber": false, "calculated": false,
                    "aggregatable": true, "byteLength": 18,
                    "cascadeDelete": false, "caseSensitive": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:ID",
                    "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false,
                    "referenceTo": []
                },
                {
                    "name": "Name", "type": "string", "label": "Name", "createable": true,
                    "autoNumber": false, "calculated": false,
                    "aggregatable": true, "byteLength": 255,
                    "cascadeDelete": false, "caseSensitive": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 255, "nameField": true, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false,
                    "referenceTo": []
                },
                {
                    "name": "AnnualRevenue", "type": "currency", "label": "Annual Revenue", "createable": true,
                    "autoNumber": false, "calculated": false,
                    "aggregatable": true, "byteLength": 0,
                    "cascadeDelete": false, "caseSensitive": false, "custom": false,
                    "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": false, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 0, "nameField": false, "namePointing": false, "nillable": true,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 18, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:double",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false,
                    "referenceTo": []
                },
                {
                    "name": "IsActive", "type": "boolean", "label": "Is Active", "createable": true,
                    "autoNumber": false, "calculated": false,
                    "aggregatable": true, "byteLength": 0,
                    "cascadeDelete": false, "caseSensitive": false, "custom": true,
                    "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 0, "nameField": false, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:boolean",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false,
                    "referenceTo": []
                }
            ]
        }"#;

        let describe: SObjectDescribe = serde_json::from_str(describe_json).must();
        let apex = generate_apex_class(&describe);

        assert!(apex.contains("public class Account {"));
        assert!(apex.contains("@AuraEnabled\n    public Id Id { get; set; }"));
        assert!(apex.contains("@AuraEnabled\n    public Decimal AnnualRevenue { get; set; }"));
        assert!(apex.contains("@AuraEnabled\n    public Boolean IsActive { get; set; }"));
        assert!(apex.contains("@AuraEnabled\n    public String Name { get; set; }"));
        assert!(apex.contains('}'));
    }
}
