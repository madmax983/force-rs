//! Destructive Changes Generator.
//!
//! Generates a `destructiveChanges.xml` manifest for removing unused or zombie
//! custom fields, based on field usage statistics and schema metadata.

use crate::schema::scanner::FieldUsage;
use crate::types::describe::SObjectDescribe;
use std::fmt::Write;

/// Generates a `destructiveChanges.xml` for custom fields whose usage falls below a given threshold.
///
/// Only custom fields (`__c`) are included, as standard fields cannot be deleted via the Metadata API.
#[must_use]
pub fn generate_destructive_changes(
    describe: &SObjectDescribe,
    usages: &[FieldUsage],
    usage_threshold_percent: f64,
) -> String {
    let mut out = String::new();
    let _ = writeln!(&mut out, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    let _ = writeln!(
        &mut out,
        "<Package xmlns=\"http://soap.sforce.com/2006/04/metadata\">"
    );

    let mut fields_to_delete = Vec::new();

    for field in &describe.fields {
        if !field.custom {
            continue;
        }

        if let Some(usage) = usages.iter().find(|u| u.name == field.name) {
            if usage.percentage <= usage_threshold_percent {
                fields_to_delete.push(format!("{}.{}", describe.name, field.name));
            }
        }
    }

    if !fields_to_delete.is_empty() {
        let _ = writeln!(&mut out, "    <types>");
        fields_to_delete.sort();
        for field_name in fields_to_delete {
            let _ = writeln!(&mut out, "        <members>{}</members>", field_name);
        }
        let _ = writeln!(&mut out, "        <name>CustomField</name>");
        let _ = writeln!(&mut out, "    </types>");
    }

    let _ = writeln!(&mut out, "    <version>60.0</version>");
    let _ = write!(&mut out, "</Package>");

    out
}

#[cfg(all(test, feature = "schema"))]
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
    fn mock_field(name: &str, field_type: &str, custom: bool) -> serde_json::Value {
        json!({
            "name": name,
            "type": field_type,
            "label": format!("{} Label", name),
            "referenceTo": if field_type == "reference" { vec!["Account"] } else { vec![] },
            "custom": custom,
            "nillable": true,
            "defaultedOnCreate": false,
            "calculated": false,
            "createable": true, "autoNumber": false, "aggregatable": true, "byteLength": 18,
            "cascadeDelete": false, "caseSensitive": false,
            "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": true, "length": 18, "nameField": false, "namePointing": false,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:ID",
            "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false
        })
    }

    #[test]
    fn test_generate_destructive_changes() {
        let describe = create_mock_describe(&json!([
            mock_field("StandardField", "string", false),
            mock_field("UnusedCustom__c", "string", true),
            mock_field("UsedCustom__c", "string", true),
        ]));

        let usages = vec![
            FieldUsage {
                name: "StandardField".to_string(),
                type_: "string".to_string(),
                populated_count: 0,
                total_count: 100,
                percentage: 0.0,
            },
            FieldUsage {
                name: "UnusedCustom__c".to_string(),
                type_: "string".to_string(),
                populated_count: 5,
                total_count: 100,
                percentage: 5.0,
            },
            FieldUsage {
                name: "UsedCustom__c".to_string(),
                type_: "string".to_string(),
                populated_count: 90,
                total_count: 100,
                percentage: 90.0,
            },
        ];

        let xml = generate_destructive_changes(&describe, &usages, 10.0);

        assert!(xml.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml.contains("<members>Account.UnusedCustom__c</members>"));
        assert!(!xml.contains("Account.StandardField"));
        assert!(!xml.contains("Account.UsedCustom__c"));
        assert!(xml.contains("<name>CustomField</name>"));
    }
}
