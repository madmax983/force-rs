//! Apex class generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates an Apex wrapper class definition from an SObject describe result.
#[cfg(feature = "schema")]
#[must_use]
pub fn generate_apex_wrapper(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_apex_wrapper(&mut out, describe);
    out
}

/// Writes an Apex wrapper class definition from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_apex_wrapper(out: &mut String, describe: &SObjectDescribe) {
    out.push_str("/**\n");
    let _ = writeln!(out, " * Wrapper class for {}", describe.label);
    out.push_str(" */\n");
    let _ = writeln!(out, "public class {}Wrapper {{", describe.name);

    // Sort fields alphabetically, Id first
    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| {
        if a.name == "Id" {
            std::cmp::Ordering::Less
        } else if b.name == "Id" {
            std::cmp::Ordering::Greater
        } else {
            a.name.cmp(&b.name)
        }
    });

    for field in fields {
        out.push_str("\n    @AuraEnabled\n");
        let ts_type = map_type(&field.type_);
        let _ = writeln!(out, "    public {} {} {{ get; set; }}", ts_type, field.name);
    }

    out.push_str("}\n");
}

/// Maps a Salesforce `FieldType` to an Apex type.
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
    use crate::test_utils::must::Must;
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
            "fields": fields_json
        });
        serde_json::from_value(describe_json).must()
    }

    #[allow(clippy::fn_params_excessive_bools)]
    fn mock_field(name: &str, field_type: &str) -> serde_json::Value {
        json!({
            "name": name,
            "type": field_type,
            "label": format!("{} Label", name),
            "referenceTo": if field_type == "reference" { vec!["Account"] } else { vec![] },
            "custom": false,
            "nillable": true,
            "defaultedOnCreate": false,
            "calculated": false,
            "createable": true, "autoNumber": false, "aggregatable": true, "byteLength": 18,
            "cascadeDelete": false, "caseSensitive": false,
            "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": name == "Id", "length": 18, "nameField": false, "namePointing": false,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:ID",
            "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false
        })
    }

    #[test]
    fn test_apex_wrapper_generator() {
        let describe = create_mock_describe(&json!([
            mock_field("Id", "id"),
            mock_field("Name", "string"),
            mock_field("NumberOfEmployees", "int"),
            mock_field("IsActive", "boolean"),
            mock_field("AnnualRevenue", "currency")
        ]));

        let apex = generate_apex_wrapper(&describe);

        let expected = "/**\n * Wrapper class for Account\n */\npublic class AccountWrapper {\n\n    @AuraEnabled\n    public Id Id { get; set; }\n\n    @AuraEnabled\n    public Decimal AnnualRevenue { get; set; }\n\n    @AuraEnabled\n    public Boolean IsActive { get; set; }\n\n    @AuraEnabled\n    public String Name { get; set; }\n\n    @AuraEnabled\n    public Integer NumberOfEmployees { get; set; }\n}\n";
        assert_eq!(apex, expected);
    }
}
