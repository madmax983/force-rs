//! CSV template generator for Salesforce Bulk API.

#[cfg(all(feature = "schema", feature = "bulk"))]
use crate::types::describe::SObjectDescribe;

/// Generates a CSV template for bulk inserting an SObject.
///
/// Creates a CSV with a header row of all createable fields
/// and a single row of mock data.
#[cfg(all(feature = "schema", feature = "bulk"))]
pub fn generate_csv_template(
    describe: &SObjectDescribe,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut writer = csv::WriterBuilder::new().from_writer(vec![]);

    // Sort fields alphabetically to ensure stable output
    let mut fields: Vec<&_> = describe.fields.iter().filter(|f| f.createable).collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    // Write header
    let headers: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
    writer.write_record(&headers)?;

    // Write mock data row
    let mock_data = crate::schema::mock_data_generator::generate_mock_data(describe);
    if let Some(obj) = mock_data.as_object() {
        let mut row = Vec::new();
        for field in &fields {
            if let Some(val) = obj.get(&field.name) {
                match val {
                    serde_json::Value::String(s) => row.push(s.clone()),
                    serde_json::Value::Bool(b) => row.push(b.to_string()),
                    serde_json::Value::Number(n) => row.push(n.to_string()),
                    _ => row.push(String::new()),
                }
            } else {
                row.push(String::new());
            }
        }
        writer.write_record(&row)?;
    }

    let bytes = writer
        .into_inner()
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let csv_string =
        String::from_utf8(bytes).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    Ok(csv_string)
}

#[cfg(test)]
#[cfg(all(feature = "schema", feature = "bulk"))]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;
    use crate::types::describe::SObjectDescribe;

    #[test]
    fn test_generate_csv_template() {
        let describe_json = serde_json::json!({
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
                    "name": "Id",
                    "type": "id",
                    "label": "Account ID",
                    "createable": false,
                    "autoNumber": false, "calculated": false, "custom": false, "nillable": false,
                    "defaultedOnCreate": true, "referenceTo": [],
                    "aggregatable": true, "byteLength": 18,
                    "cascadeDelete": false, "caseSensitive": false,
                    "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 18, "nameField": false, "namePointing": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:id",
                    "sortable": true, "unique": true, "updateable": false, "writeRequiresMasterRead": false
                },
                {
                    "name": "Name",
                    "type": "string",
                    "label": "Account Name",
                    "createable": true,
                    "autoNumber": false, "calculated": false, "custom": false, "nillable": false,
                    "defaultedOnCreate": false, "referenceTo": [],
                    "aggregatable": true, "byteLength": 255,
                    "cascadeDelete": false, "caseSensitive": false,
                    "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 255, "nameField": true, "namePointing": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false
                },
                {
                    "name": "AnnualRevenue",
                    "type": "currency",
                    "label": "Annual Revenue",
                    "createable": true,
                    "autoNumber": false, "calculated": false, "custom": false, "nillable": true,
                    "defaultedOnCreate": false, "referenceTo": [],
                    "aggregatable": true, "byteLength": 0,
                    "cascadeDelete": false, "caseSensitive": false,
                    "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 0, "nameField": false, "namePointing": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:double",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false
                },
                {
                    "name": "IsActive",
                    "type": "boolean",
                    "label": "Active",
                    "createable": true,
                    "autoNumber": false, "calculated": false, "custom": false, "nillable": false,
                    "defaultedOnCreate": false, "referenceTo": [],
                    "aggregatable": true, "byteLength": 0,
                    "cascadeDelete": false, "caseSensitive": false,
                    "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 0, "nameField": false, "namePointing": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:boolean",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false
                }
            ]
        });
        let describe: SObjectDescribe = serde_json::from_value(describe_json).must();

        let csv_result = generate_csv_template(&describe).must();
        assert!(csv_result.contains("AnnualRevenue,IsActive,Name"));
        assert!(csv_result.contains("42.0,true,mock_string"));
    }
}
