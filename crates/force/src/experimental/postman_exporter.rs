//! Postman Collection Exporter.
//!
//! This module provides a utility to generate Postman collections for Salesforce SObjects,
//! pre-filled with mock data and the standard REST operations (Create, Read, Update, Delete, Query).

use crate::api::rest::describe::SObjectDescribe;
use crate::experimental::DataFaker;

/// Exporter for generating Postman collections from SObject describe metadata.
#[derive(Debug, Default)]
pub struct PostmanExporter;

impl PostmanExporter {
    /// Creates a new `PostmanExporter` instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Generates a Postman Collection v2.1.0 JSON representation for the given SObject.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn generate(&self, describe: &SObjectDescribe) -> String {
        let faker = DataFaker::new();
        let mock_record = faker.generate_mock_record(describe);

        let body_json = serde_json::to_value(&mock_record).unwrap_or(serde_json::Value::Null);

        let collection = serde_json::json!({
            "info": {
                "name": format!("Salesforce {} APIs", describe.label),
                "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
            },
            "variable": [
                {
                    "key": "base_url",
                    "value": "https://your-domain.my.salesforce.com",
                    "type": "string"
                },
                {
                    "key": "api_version",
                    "value": "v60.0",
                    "type": "string"
                },
                {
                    "key": "access_token",
                    "value": "your_access_token",
                    "type": "string"
                }
            ],
            "item": [
                {
                    "name": format!("Create {}", describe.label),
                    "request": {
                        "method": "POST",
                        "header": [
                            { "key": "Authorization", "value": "Bearer {{access_token}}" },
                            { "key": "Content-Type", "value": "application/json" }
                        ],
                        "url": {
                            "raw": format!("{{{{base_url}}}}/services/data/{{{{api_version}}}}/sobjects/{}", describe.name),
                            "host": ["{{base_url}}"],
                            "path": ["services", "data", "{{api_version}}", "sobjects", &describe.name]
                        },
                        "body": {
                            "mode": "raw",
                            "raw": serde_json::to_string_pretty(&body_json).unwrap_or_default()
                        }
                    }
                },
                {
                    "name": format!("Get {}", describe.label),
                    "request": {
                        "method": "GET",
                        "header": [
                            { "key": "Authorization", "value": "Bearer {{access_token}}" }
                        ],
                        "url": {
                            "raw": format!("{{{{base_url}}}}/services/data/{{{{api_version}}}}/sobjects/{}/:id", describe.name),
                            "host": ["{{base_url}}"],
                            "path": ["services", "data", "{{api_version}}", "sobjects", &describe.name, ":id"],
                            "variable": [
                                { "key": "id", "value": "001000000000000AAA" }
                            ]
                        }
                    }
                },
                {
                    "name": format!("Update {}", describe.label),
                    "request": {
                        "method": "PATCH",
                        "header": [
                            { "key": "Authorization", "value": "Bearer {{access_token}}" },
                            { "key": "Content-Type", "value": "application/json" }
                        ],
                        "url": {
                            "raw": format!("{{{{base_url}}}}/services/data/{{{{api_version}}}}/sobjects/{}/:id", describe.name),
                            "host": ["{{base_url}}"],
                            "path": ["services", "data", "{{api_version}}", "sobjects", &describe.name, ":id"],
                            "variable": [
                                { "key": "id", "value": "001000000000000AAA" }
                            ]
                        },
                        "body": {
                            "mode": "raw",
                            "raw": serde_json::to_string_pretty(&body_json).unwrap_or_default()
                        }
                    }
                },
                {
                    "name": format!("Delete {}", describe.label),
                    "request": {
                        "method": "DELETE",
                        "header": [
                            { "key": "Authorization", "value": "Bearer {{access_token}}" }
                        ],
                        "url": {
                            "raw": format!("{{{{base_url}}}}/services/data/{{{{api_version}}}}/sobjects/{}/:id", describe.name),
                            "host": ["{{base_url}}"],
                            "path": ["services", "data", "{{api_version}}", "sobjects", &describe.name, ":id"],
                            "variable": [
                                { "key": "id", "value": "001000000000000AAA" }
                            ]
                        }
                    }
                },
                {
                    "name": format!("Query {}s", describe.label),
                    "request": {
                        "method": "GET",
                        "header": [
                            { "key": "Authorization", "value": "Bearer {{access_token}}" }
                        ],
                        "url": {
                            "raw": format!("{{{{base_url}}}}/services/data/{{{{api_version}}}}/query?q=SELECT+Id+FROM+{}", describe.name),
                            "host": ["{{base_url}}"],
                            "path": ["services", "data", "{{api_version}}", "query"],
                            "query": [
                                { "key": "q", "value": format!("SELECT Id FROM {}", describe.name) }
                            ]
                        }
                    }
                }
            ]
        });

        serde_json::to_string_pretty(&collection).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Must;
    use serde_json::json;

    fn mock_field(name: &str, field_type: &str, createable: bool) -> serde_json::Value {
        json!({
            "name": name,
            "type": field_type,
            "label": format!("{} Label", name),
            "referenceTo": [],
            "createable": createable,
            "autoNumber": false,
            "calculated": false,
            // Mandatory fields filler
            "aggregatable": true, "byteLength": 18,
            "cascadeDelete": false, "caseSensitive": false, "custom": false,
            "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": name == "Id", "length": 18, "nameField": name == "Name", "namePointing": false, "nillable": false,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:ID",
            "sortable": true, "unique": false, "updateable": createable, "writeRequiresMasterRead": false
        })
    }

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

    #[test]
    fn test_postman_collection_structure() {
        let describe = create_mock_describe(&json!([
            mock_field("Id", "id", false),
            mock_field("Name", "string", true),
            mock_field("Website", "url", true)
        ]));

        let exporter = PostmanExporter::new();
        let collection_json = exporter.generate(&describe);

        let parsed: serde_json::Value = serde_json::from_str(&collection_json).must();

        // 1. Verify Postman v2.1.0 Info block
        assert_eq!(
            parsed["info"]["schema"].as_str().unwrap_or_default(),
            "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
        );
        assert_eq!(
            parsed["info"]["name"].as_str().unwrap_or_default(),
            "Salesforce Account APIs"
        );

        // 2. Verify variables exist
        let Some(variables) = parsed["variable"].as_array() else {
            panic!("variable array missing")
        };
        assert!(variables.iter().any(|v| v["key"] == "base_url"));
        assert!(variables.iter().any(|v| v["key"] == "api_version"));
        assert!(variables.iter().any(|v| v["key"] == "access_token"));

        // 3. Verify items (requests)
        let Some(items) = parsed["item"].as_array() else {
            panic!("item array missing")
        };
        assert_eq!(items.len(), 5);

        let item_names: Vec<&str> = items
            .iter()
            .map(|i| i["name"].as_str().unwrap_or_default())
            .collect();
        assert!(item_names.contains(&"Create Account"));
        assert!(item_names.contains(&"Get Account"));
        assert!(item_names.contains(&"Update Account"));
        assert!(item_names.contains(&"Delete Account"));
        assert!(item_names.contains(&"Query Accounts"));

        // 4. Verify Create body uses mock data
        let Some(create_req) = items.iter().find(|i| i["name"] == "Create Account") else {
            panic!("Create Account req missing")
        };
        assert_eq!(create_req["request"]["method"], "POST");
        let body_str = create_req["request"]["body"]["raw"]
            .as_str()
            .unwrap_or_default();
        let Ok(body) = serde_json::from_str::<serde_json::Value>(body_str) else {
            panic!("Failed to parse body")
        };
        assert_eq!(body["Name"], "Mock Name Label");
    }
}
