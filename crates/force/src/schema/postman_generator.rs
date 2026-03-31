#[cfg(feature = "schema")]
use crate::api::rest::describe::SObjectDescribe;
use serde_json::Value;

/// Generates a Postman Collection (v2.1.0) JSON for the given SObject describe.
#[cfg(feature = "schema")]
#[must_use]
pub fn generate_postman_collection(describe: &SObjectDescribe) -> Value {
    let name = &describe.name;

    // Generate mock payload using our data utility
    let mock_record = crate::data::generate_mock_record(describe);

    // Convert to Value and remove DynamicSObject specific wrappers like attributes and Id
    let mut mock_json = serde_json::to_value(mock_record).unwrap_or_else(|_| serde_json::json!({}));
    if let Some(obj) = mock_json.as_object_mut() {
        obj.remove("attributes");
        obj.remove("Id");
    }

    let mock_json_str =
        serde_json::to_string_pretty(&mock_json).unwrap_or_else(|_| "{}".to_string());

    let base_url = format!("{{{{instance_url}}}}/services/data/v60.0/sobjects/{}", name);

    serde_json::json!({
        "info": {
            "name": format!("Salesforce - {} API", describe.label),
            "description": format!("Auto-generated collection for the {} Salesforce object.", name),
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
        },
        "item": [
            {
                "name": format!("Create {}", describe.label),
                "request": {
                    "method": "POST",
                    "header": [
                        { "key": "Content-Type", "value": "application/json" }
                    ],
                    "body": {
                        "mode": "raw",
                        "raw": mock_json_str
                    },
                    "url": {
                        "raw": base_url,
                        "host": ["{{instance_url}}"],
                        "path": ["services", "data", "v60.0", "sobjects", name]
                    }
                }
            },
            {
                "name": format!("Retrieve {}", describe.label),
                "request": {
                    "method": "GET",
                    "header": [],
                    "url": {
                        "raw": format!("{}/{{{{record_id}}}}", base_url),
                        "host": ["{{instance_url}}"],
                        "path": ["services", "data", "v60.0", "sobjects", name, "{{record_id}}"]
                    }
                }
            },
            {
                "name": format!("Update {}", describe.label),
                "request": {
                    "method": "PATCH",
                    "header": [
                        { "key": "Content-Type", "value": "application/json" }
                    ],
                    "body": {
                        "mode": "raw",
                        "raw": mock_json_str
                    },
                    "url": {
                        "raw": format!("{}/{{{{record_id}}}}", base_url),
                        "host": ["{{instance_url}}"],
                        "path": ["services", "data", "v60.0", "sobjects", name, "{{record_id}}"]
                    }
                }
            },
            {
                "name": format!("Delete {}", describe.label),
                "request": {
                    "method": "DELETE",
                    "header": [],
                    "url": {
                        "raw": format!("{}/{{{{record_id}}}}", base_url),
                        "host": ["{{instance_url}}"],
                        "path": ["services", "data", "v60.0", "sobjects", name, "{{record_id}}"]
                    }
                }
            }
        ]
    })
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_support::Must;
    use serde_json::json;

    fn create_mock_describe() -> SObjectDescribe {
        // Construct fields separately to avoid recursion limit
        let id_field = json!({
            "name": "Id", "type": "id", "label": "Account ID", "createable": false,
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
        });

        let name_field = json!({
            "name": "Name", "type": "string", "label": "Account Name", "createable": true,
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
        });

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
            "fields": [id_field, name_field]
        });

        serde_json::from_value(describe_json).must()
    }

    #[test]
    fn test_generate_postman_collection() {
        let describe = create_mock_describe();
        let collection = generate_postman_collection(&describe);

        // Assert base structure
        assert_eq!(
            collection["info"]["name"].as_str(),
            Some("Salesforce - Account API")
        );
        assert_eq!(
            collection["info"]["schema"].as_str(),
            Some("https://schema.getpostman.com/json/collection/v2.1.0/collection.json")
        );

        let items = collection["item"].as_array().must();
        assert_eq!(
            items.len(),
            4,
            "Should have 4 requests (Create, Retrieve, Update, Delete)"
        );

        // 1. Create
        let req_create = &items[0];
        assert_eq!(req_create["name"].as_str(), Some("Create Account"));
        assert_eq!(req_create["request"]["method"].as_str(), Some("POST"));
        assert_eq!(
            req_create["request"]["url"]["raw"].as_str(),
            Some("{{instance_url}}/services/data/v60.0/sobjects/Account")
        );

        // Assert payload exists and does not contain Id or attributes
        let body = req_create["request"]["body"]["raw"].as_str().must();
        let parsed_body: Value = serde_json::from_str(body).must();
        assert!(!parsed_body.as_object().must().contains_key("Id"));
        assert!(!parsed_body.as_object().must().contains_key("attributes"));
        assert_eq!(parsed_body["Name"].as_str(), Some("Mock Account Name"));

        // 2. Retrieve
        let req_retrieve = &items[1];
        assert_eq!(req_retrieve["name"].as_str(), Some("Retrieve Account"));
        assert_eq!(req_retrieve["request"]["method"].as_str(), Some("GET"));
        assert_eq!(
            req_retrieve["request"]["url"]["raw"].as_str(),
            Some("{{instance_url}}/services/data/v60.0/sobjects/Account/{{record_id}}")
        );

        // 3. Update
        let req_update = &items[2];
        assert_eq!(req_update["name"].as_str(), Some("Update Account"));
        assert_eq!(req_update["request"]["method"].as_str(), Some("PATCH"));
        assert_eq!(
            req_update["request"]["url"]["raw"].as_str(),
            Some("{{instance_url}}/services/data/v60.0/sobjects/Account/{{record_id}}")
        );

        // 4. Delete
        let req_delete = &items[3];
        assert_eq!(req_delete["name"].as_str(), Some("Delete Account"));
        assert_eq!(req_delete["request"]["method"].as_str(), Some("DELETE"));
        assert_eq!(
            req_delete["request"]["url"]["raw"].as_str(),
            Some("{{instance_url}}/services/data/v60.0/sobjects/Account/{{record_id}}")
        );
    }
}
