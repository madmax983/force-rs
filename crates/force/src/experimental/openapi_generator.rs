//! OpenAPI 3.0 Specification Generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::api::rest::describe::{FieldType, SObjectDescribe};
use serde_json::{Value, json};

/// Experimental utility to generate an OpenAPI 3.0 specification from SObject describe metadata.
#[cfg(feature = "schema")]
pub struct OpenApiGenerator;

#[cfg(feature = "schema")]
impl OpenApiGenerator {
    /// Generates an OpenAPI 3.0 JSON specification from an SObject describe result.
    #[must_use]
    pub fn generate(describe: &SObjectDescribe) -> Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

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
            let mut prop = serde_json::Map::new();

            let (type_str, format_str) = Self::map_type(&field.type_);
            prop.insert("type".to_string(), json!(type_str));
            if let Some(f) = format_str {
                prop.insert("format".to_string(), json!(f));
            }
            prop.insert("description".to_string(), json!(field.label));
            if !field.updateable {
                prop.insert("readOnly".to_string(), json!(true));
            }

            properties.insert(field.name.clone(), Value::Object(prop));

            if !field.nillable && !field.defaulted_on_create {
                required.push(json!(field.name));
            }
        }

        let mut schema = json!({
            "type": "object",
            "properties": properties,
        });

        if !required.is_empty() {
            if let Some(obj) = schema.as_object_mut() {
                obj.insert("required".to_string(), Value::Array(required));
            }
        }

        json!({
            "openapi": "3.0.3",
            "info": {
                "title": format!("Salesforce {} API", describe.label),
                "version": "1.0.0",
                "description": format!("Auto-generated OpenAPI spec for {}", describe.name)
            },
            "components": {
                "schemas": {
                    describe.name.clone(): schema
                }
            },
            "paths": {
                format!("/sobjects/{}", describe.name): {
                    "get": {
                        "summary": format!("Get {} metadata", describe.label),
                        "responses": {
                            "200": {
                                "description": "Successful response",
                                "content": {
                                    "application/json": {
                                        "schema": {
                                            "$ref": format!("#/components/schemas/{}", describe.name)
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        })
    }

    fn map_type(ft: &FieldType) -> (&'static str, Option<&'static str>) {
        match ft {
            FieldType::Boolean => ("boolean", None),
            FieldType::Int => ("integer", Some("int32")),
            FieldType::Double | FieldType::Currency | FieldType::Percent => {
                ("number", Some("double"))
            }
            FieldType::Date => ("string", Some("date")),
            FieldType::Datetime => ("string", Some("date-time")),
            _ => ("string", None),
        }
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_support::Must;
    use serde_json::json;

    fn create_mock_describe(fields_json: &serde_json::Value) -> SObjectDescribe {
        let describe_json = json!({
            "name": "Account",
            "label": "Account Label",
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
        field_type: &str,
        updateable: bool,
        nillable: bool,
        defaulted: bool,
    ) -> serde_json::Value {
        json!({
            "name": name,
            "type": field_type,
            "label": format!("{} Label", name),
            "referenceTo": [],
            "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
            "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": false,
            "defaultedOnCreate": defaulted, "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": nillable,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:ID",
            "sortable": true, "unique": false, "updateable": updateable, "writeRequiresMasterRead": false
        })
    }

    #[test]
    fn test_openapi_generator() {
        let describe = create_mock_describe(&json!([
            mock_field("Id", "id", false, false, true),
            mock_field("Name", "string", true, false, false),
            mock_field("NumberOfEmployees", "int", true, true, false)
        ]));

        let spec = OpenApiGenerator::generate(&describe);

        assert_eq!(spec["openapi"], "3.0.3");
        assert_eq!(spec["info"]["title"], "Salesforce Account Label API");

        let schema = &spec["components"]["schemas"]["Account"];
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"]["Id"]["type"], "string");
        assert_eq!(schema["properties"]["Id"]["readOnly"], true);
        assert_eq!(schema["properties"]["Name"]["type"], "string");
        assert_eq!(schema["properties"]["NumberOfEmployees"]["type"], "integer");
        assert_eq!(schema["properties"]["NumberOfEmployees"]["format"], "int32");

        let required = schema["required"].as_array().must();
        assert_eq!(required.len(), 1);
        assert_eq!(required[0], "Name");
    }
}
