//! CSV template generator for Salesforce SObject Describe metadata.

#[cfg(feature = "schema")]
use crate::types::describe::SObjectDescribe;
#[cfg(feature = "schema")]
use std::fmt::Write;

/// Generates a CSV template header string from an SObject describe result, including only createable fields.
#[cfg(feature = "schema")]
#[must_use]
pub fn generate_csv_template(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 16);
    write_csv_template(&mut out, describe);
    out
}

/// Writes a CSV template header string from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_csv_template(out: &mut String, describe: &SObjectDescribe) {
    let mut sorted_fields: Vec<&_> = describe.fields.iter().filter(|f| f.createable).collect();
    sorted_fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    let mut first = true;
    for field in sorted_fields {
        if first {
            first = false;
        } else {
            out.push(',');
        }
        let _ = write!(out, "{}", field.name);
    }
    out.push('\n');
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
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
        serde_json::from_value(describe_json).expect("valid json")
    }

    #[allow(clippy::fn_params_excessive_bools)]
    fn mock_field(
        name: &str,
        field_type: &str,
        createable: bool,
    ) -> serde_json::Value {
        json!({
            "name": name,
            "type": field_type,
            "label": format!("{} Label", name),
            "createable": createable,
            "aggregatable": true, "autoNumber": false, "byteLength": 18,
            "calculated": false, "cascadeDelete": false, "caseSensitive": false,
            "custom": false, "defaultedOnCreate": false, "dependentPicklist": false,
            "deprecatedAndHidden": false, "digits": 0, "displayLocationInDecimal": false,
            "encrypted": false, "externalId": false, "filterable": true, "groupable": true,
            "highScaleNumber": false, "htmlFormatted": false, "idLookup": true, "length": 18,
            "nameField": false, "namePointing": false, "nillable": true, "permissionable": false,
            "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "referenceTo": [], "restrictedDelete": false, "restrictedPicklist": false,
            "scale": 0, "soapType": "tns:ID", "sortable": true, "unique": false,
            "updateable": false, "writeRequiresMasterRead": false
        })
    }

    #[test]
    fn test_generate_csv_template() {
        let describe = create_mock_describe(&json!([
            mock_field("Id", "id", false),
            mock_field("Name", "string", true),
            mock_field("AnnualRevenue", "currency", true)
        ]));

        let csv = generate_csv_template(&describe);
        assert_eq!(csv, "AnnualRevenue,Name\n");
    }
}
