#[cfg(feature = "schema")]
use crate::schema::mock_data_generator::generate_mock_data;
#[cfg(feature = "schema")]
use crate::types::describe::SObjectDescribe;
#[cfg(feature = "schema")]
use serde_json::Value;

/// Options for CSV template generation.
#[cfg(feature = "schema")]
#[derive(Debug, Clone, Default)]
pub struct CsvTemplateOptions {
    /// Include a row of mock data.
    pub include_mock_data: bool,
}

/// Generates a CSV template for bulk ingest of an SObject.
#[cfg(feature = "schema")]
#[must_use]
pub fn generate_csv_template(describe: &SObjectDescribe, options: &CsvTemplateOptions) -> String {
    let mut headers = Vec::new();

    for field in &describe.fields {
        if field.createable {
            headers.push(field.name.as_str());
        }
    }

    let mut out = headers.join(",");
    out.push('\n');

    if options.include_mock_data {
        let mock_data = generate_mock_data(describe);
        if let Value::Object(map) = mock_data {
            let mut row = Vec::new();
            for header in headers {
                let val_str = match map.get(header) {
                    Some(Value::String(s)) => format!("\"{}\"", s), // Basic escaping for strings
                    Some(Value::Number(n)) => n.to_string(),
                    Some(Value::Bool(b)) => b.to_string(),
                    _ => String::new(),
                };
                row.push(val_str);
            }
            out.push_str(&row.join(","));
            out.push('\n');
        }
    }

    out
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
        let result: Result<SObjectDescribe, _> = serde_json::from_value(describe_json);
        match result {
            Ok(desc) => desc,
            Err(e) => panic!("Failed to deserialize mock SObjectDescribe: {}", e),
        }
    }

    fn mock_field(name: &str, field_type: &str, createable: bool) -> serde_json::Value {
        json!({
            "name": name,
            "type": field_type,
            "label": format!("{} Label", name),
            "referenceTo": [],
            "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
            "cascadeDelete": false, "caseSensitive": false, "createable": createable, "custom": false,
            "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:ID",
            "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false
        })
    }

    #[test]
    fn test_generate_csv_template() {
        let describe = create_mock_describe(&json!([
            mock_field("Id", "id", false),
            mock_field("Name", "string", true),
            mock_field("IsActive", "boolean", true),
            mock_field("NumberOfEmployees", "int", true)
        ]));

        let options = CsvTemplateOptions {
            include_mock_data: false,
        };

        let csv = generate_csv_template(&describe, &options);
        assert_eq!(csv, "Name,IsActive,NumberOfEmployees\n");

        let options_with_mock = CsvTemplateOptions {
            include_mock_data: true,
        };

        let csv_mock = generate_csv_template(&describe, &options_with_mock);
        assert_eq!(
            csv_mock,
            "Name,IsActive,NumberOfEmployees\n\"mock_string\",true,42\n"
        );
    }
}
