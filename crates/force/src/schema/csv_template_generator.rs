#[cfg(feature = "schema")]
use crate::types::describe::SObjectDescribe;

/// Generates a CSV template header string from an SObject describe result.
/// It includes only fields that are createable, which is useful for bulk ingest templates.
#[cfg(feature = "schema")]
#[must_use]
pub fn generate_csv_template(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(1024);
    write_csv_template(&mut out, describe);
    out
}

/// Writes a CSV template header string directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_csv_template(out: &mut String, describe: &SObjectDescribe) {
    let mut fields: Vec<&_> = describe.fields.iter().filter(|f| f.createable).collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for (i, field) in fields.iter().enumerate() {
        out.push_str(&field.name);
        if i < fields.len() - 1 {
            out.push(',');
        }
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
        crate::test_utils::must::Must::must(serde_json::from_value(describe_json))
    }

    #[allow(clippy::fn_params_excessive_bools)]
    fn mock_field(name: &str, field_type: &str, createable: bool) -> serde_json::Value {
        json!({
            "name": name,
            "type": field_type,
            "label": format!("{} Label", name),
            "referenceTo": [],
            "custom": false,
            "nillable": true,
            "defaultedOnCreate": false,
            "calculated": false,
            "createable": createable, "autoNumber": false, "aggregatable": true, "byteLength": 18,
            "cascadeDelete": false, "caseSensitive": false,
            "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": false, "length": 18, "nameField": false, "namePointing": false,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
            "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false
        })
    }

    #[test]
    fn test_generate_csv_template() {
        let describe = create_mock_describe(&json!([
            mock_field("Id", "id", false),
            mock_field("Name", "string", true),
            mock_field("Website", "url", true),
            mock_field("CreatedDate", "datetime", false),
            mock_field("Custom__c", "string", true)
        ]));

        let csv = generate_csv_template(&describe);
        assert_eq!(csv, "Custom__c,Name,Website\n");
    }
}
