use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates an Apex Test Data Factory class from an SObject describe result.
pub fn generate_apex_test_factory(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    let class_name = format!("{}TestDataFactory", describe.name);

    let _ = writeln!(out, "@isTest");
    let _ = writeln!(out, "public class {} {{", class_name);
    let _ = writeln!(
        out,
        "    public static {} create{}(Boolean doInsert) {{",
        describe.name, describe.name
    );
    let _ = writeln!(
        out,
        "        {} obj = new {}();",
        describe.name, describe.name
    );

    for field in &describe.fields {
        if !field.createable || field.name == "Id" || field.defaulted_on_create || field.nillable {
            continue;
        }

        let value = match field.type_ {
            FieldType::String
            | FieldType::Textarea
            | FieldType::Url
            | FieldType::Phone
            | FieldType::Email => "'Test String'",
            FieldType::Boolean => "true",
            FieldType::Int | FieldType::Double | FieldType::Percent | FieldType::Currency => "42",
            FieldType::Date => "Date.today()",
            FieldType::Datetime => "Datetime.now()",
            FieldType::Picklist | FieldType::Multipicklist | FieldType::Combobox => "'Test'",
            _ => "null",
        };

        let _ = writeln!(out, "        obj.{} = {};", field.name, value);
    }

    let _ = writeln!(out, "        if (doInsert) {{");
    let _ = writeln!(out, "            insert obj;");
    let _ = writeln!(out, "        }}");
    let _ = writeln!(out, "        return obj;");
    let _ = writeln!(out, "    }}");
    let _ = writeln!(out, "}}");

    out
}

#[cfg(test)]
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
    fn mock_field(
        name: &str,
        field_type: &str,
        createable: bool,
        nillable: bool,
        defaulted: bool,
    ) -> serde_json::Value {
        json!({
            "name": name,
            "type": field_type,
            "label": format!("{} Label", name),
            "createable": createable,
            "nillable": nillable,
            "defaultedOnCreate": defaulted,
            "referenceTo": [],
            "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
            "cascadeDelete": false, "caseSensitive": false, "custom": false,
            "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": false, "length": 18, "nameField": false, "namePointing": false,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:ID",
            "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false
        })
    }

    #[test]
    fn test_generate_apex_test_factory() {
        let describe = create_mock_describe(&json!([
            mock_field("Id", "id", false, false, true),
            mock_field("NotCreateable", "string", false, false, false),
            mock_field("Nillable", "string", true, true, false),
            mock_field("Defaulted", "string", true, false, true),
            mock_field("RequiredString", "string", true, false, false),
            mock_field("RequiredInt", "int", true, false, false),
            mock_field("RequiredDate", "date", true, false, false),
        ]));

        let code = generate_apex_test_factory(&describe);

        assert!(code.contains("@isTest"));
        assert!(code.contains("public class AccountTestDataFactory"));
        assert!(code.contains("public static Account createAccount(Boolean doInsert)"));
        assert!(code.contains("Account obj = new Account();"));

        assert!(code.contains("obj.RequiredString = 'Test String';"));
        assert!(code.contains("obj.RequiredInt = 42;"));
        assert!(code.contains("obj.RequiredDate = Date.today();"));

        assert!(!code.contains("obj.Id"));
        assert!(!code.contains("obj.NotCreateable"));
        assert!(!code.contains("obj.Nillable"));
        assert!(!code.contains("obj.Defaulted"));

        assert!(code.contains("insert obj;"));
    }
}
