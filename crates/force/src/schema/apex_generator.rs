//! Apex class generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates an Apex class definition from an SObject describe result.
#[cfg(feature = "schema")]
#[must_use]
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
        let apex_type = map_type(&field.type_);
        out.push_str("    @AuraEnabled\n");
        let _ = writeln!(out, "    public {} {} {{ get; set; }}", apex_type, field.name);
    }

    out.push_str("}\n");
}

/// Maps a Salesforce `FieldType` to an Apex type.
#[cfg(feature = "schema")]
fn map_type(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Boolean => "Boolean",
        FieldType::Int => "Integer",
        FieldType::Double | FieldType::Currency | FieldType::Percent => "Decimal",
        FieldType::Date => "Date",
        FieldType::Datetime => "Datetime",
        FieldType::Id | FieldType::Reference => "Id",
        _ => "String",
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use serde_json::json;

    fn mock_field(name: &str, field_type: &str) -> serde_json::Value {
        json!({
            "name": name,
            "type": field_type,
            "label": format!("{} Label", name),
            "createable": true,
            "autoNumber": false, "calculated": false,
            "aggregatable": true, "byteLength": 18,
            "cascadeDelete": false, "caseSensitive": false, "custom": false,
            "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": name == "Id", "length": 18, "nameField": name == "Name", "namePointing": false, "nillable": true,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
            "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false,
            "referenceTo": []
        })
    }

    #[test]
    fn test_apex_generator() {
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
            "fields": [
                mock_field("Id", "id"),
                mock_field("Name", "string"),
                mock_field("NumberOfEmployees", "int"),
                mock_field("AnnualRevenue", "currency"),
                mock_field("IsActive", "boolean")
            ]
        });

        let describe: SObjectDescribe = serde_json::from_value(describe_json).unwrap();

        let apex_code = generate_apex_class(&describe);

        let expected = "/**\n * Account\n */\npublic class Account {\n    @AuraEnabled\n    public Id Id { get; set; }\n    @AuraEnabled\n    public Decimal AnnualRevenue { get; set; }\n    @AuraEnabled\n    public Boolean IsActive { get; set; }\n    @AuraEnabled\n    public String Name { get; set; }\n    @AuraEnabled\n    public Integer NumberOfEmployees { get; set; }\n}\n";
        assert_eq!(apex_code, expected);
    }
}
