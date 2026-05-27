#[cfg(feature = "schema")]
use crate::types::describe::SObjectDescribe;
use std::fmt::Write;

/// Generates a CSV data dictionary from an SObject describe result.
#[cfg(feature = "schema")]
#[must_use]
pub fn generate_csv_dictionary(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 64);
    write_csv_dictionary(&mut out, describe);
    out
}

/// Writes a CSV data dictionary from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_csv_dictionary(out: &mut String, describe: &SObjectDescribe) {
    let _ = writeln!(out, "API Name,Label,Type,Length,Required,Custom");

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
        let required = !field.nillable && !field.defaulted_on_create && field.name != "Id";
        let _ = writeln!(
            out,
            "\"{}\",\"{}\",\"{:?}\",{},{},{}",
            field.name,
            field.label.replace('"', "\"\""),
            field.type_,
            field.length,
            required,
            field.custom
        );
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;
    use crate::types::describe::SObjectDescribe;
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
            "fields": fields_json.clone()
        });
        serde_json::from_value(describe_json).must()
    }

    fn mock_field(
        name: &str,
        label: &str,
        field_type: &str,
        length: i32,
        nillable: bool,
        defaulted: bool,
        custom: bool,
    ) -> serde_json::Value {
        json!({
            "name": name,
            "type": field_type,
            "label": label,
            "createable": true,
            "autoNumber": false,
            "calculated": false,
            "aggregatable": true, "byteLength": length,
            "cascadeDelete": false, "caseSensitive": false, "custom": custom,
            "defaultedOnCreate": defaulted, "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": name == "Id", "length": length, "nameField": false, "namePointing": false, "nillable": nillable,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "referenceTo": [], "restrictedDelete": false, "restrictedPicklist": false, "scale": 0,
            "soapType": "tns:ID", "sortable": true, "unique": false, "updateable": false,
            "writeRequiresMasterRead": false
        })
    }

    #[test]
    fn test_generate_csv_dictionary() {
        let describe = create_mock_describe(&json!([
            mock_field("Id", "Account ID", "id", 18, false, true, false),
            mock_field("Name", "Account Name", "string", 255, false, false, false),
            mock_field(
                "CustomField__c",
                "Custom \"Label\"",
                "string",
                100,
                true,
                false,
                true
            )
        ]));

        let csv = generate_csv_dictionary(&describe);

        let expected = "API Name,Label,Type,Length,Required,Custom\n\"Id\",\"Account ID\",\"Id\",18,false,false\n\"CustomField__c\",\"Custom \"\"Label\"\"\",\"String\",100,false,true\n\"Name\",\"Account Name\",\"String\",255,true,false\n";
        assert_eq!(csv, expected);
    }
}
