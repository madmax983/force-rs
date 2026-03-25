//! JSON Schema Generator for Salesforce SObject Describe metadata.
//!
//! Generates JSON Schema draft-07 schemas based on Salesforce object schemas.

#[cfg(feature = "schema")]
use crate::api::rest::describe::{FieldDescribe, FieldType, SObjectDescribe};
#[cfg(feature = "schema")]
use serde_json::{Value, json};

/// Experimental utility to generate JSON Schema specifications from SObject metadata.
#[cfg(feature = "schema")]
pub struct JsonSchemaGenerator;

#[cfg(feature = "schema")]
impl JsonSchemaGenerator {
    /// Generates a JSON Schema draft-07 definition for a given SObject.
    pub fn generate_schema(describe: &SObjectDescribe) -> Value {
        let mut properties = serde_json::Map::new();

        // Find required fields
        let required_fields: Vec<Value> = describe
            .fields
            .iter()
            .filter(|f| {
                !f.nillable
                    && !f.defaulted_on_create
                    && f.createable
                    && f.type_ != FieldType::Id
                    && f.type_ != FieldType::Boolean
            })
            .map(|f| Value::String(f.name.clone()))
            .collect();

        for field in &describe.fields {
            properties.insert(field.name.clone(), Self::generate_field_schema(field));
        }

        let mut schema = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "title": describe.name,
            "type": "object",
            "properties": properties,
        });

        if let Some(obj) = schema.as_object_mut() {
            if !describe.label.is_empty() {
                obj.insert(
                    "description".to_string(),
                    Value::String(describe.label.clone()),
                );
            }

            if !required_fields.is_empty() {
                obj.insert("required".to_string(), Value::Array(required_fields));
            }
        }

        schema
    }

    fn generate_field_schema(field: &FieldDescribe) -> Value {
        let mut schema = serde_json::Map::new();

        if let Some(help) = &field.inline_help_text {
            schema.insert("description".to_string(), Value::String(help.clone()));
        } else {
            schema.insert(
                "description".to_string(),
                Value::String(field.label.clone()),
            );
        }

        if !field.updateable && !field.createable {
            schema.insert("readOnly".to_string(), Value::Bool(true));
        }

        match field.type_ {
            FieldType::String
            | FieldType::Email
            | FieldType::Url
            | FieldType::Phone
            | FieldType::Id
            | FieldType::Reference
            | FieldType::Combobox => {
                schema.insert("type".to_string(), Value::String("string".to_string()));
                if field.length > 0 {
                    schema.insert(
                        "maxLength".to_string(),
                        Value::Number(serde_json::Number::from(field.length)),
                    );
                }
            }

            FieldType::Picklist | FieldType::Multipicklist => {
                schema.insert("type".to_string(), Value::String("string".to_string()));
                if let Some(values) = &field.picklist_values {
                    if !values.is_empty() {
                        let enum_values: Vec<Value> = values
                            .iter()
                            .map(|pv| Value::String(pv.value.clone()))
                            .collect();
                        schema.insert("enum".to_string(), Value::Array(enum_values));
                    }
                }
            }
            FieldType::Boolean => {
                schema.insert("type".to_string(), Value::String("boolean".to_string()));
            }
            FieldType::Int => {
                schema.insert("type".to_string(), Value::String("integer".to_string()));
            }
            FieldType::Double | FieldType::Percent | FieldType::Currency => {
                schema.insert("type".to_string(), Value::String("number".to_string()));
            }
            FieldType::Date => {
                schema.insert("type".to_string(), Value::String("string".to_string()));
                schema.insert("format".to_string(), Value::String("date".to_string()));
            }
            FieldType::Datetime => {
                schema.insert("type".to_string(), Value::String("string".to_string()));
                schema.insert("format".to_string(), Value::String("date-time".to_string()));
            }
            FieldType::Base64 => {
                schema.insert("type".to_string(), Value::String("string".to_string()));
                schema.insert(
                    "contentEncoding".to_string(),
                    Value::String("base64".to_string()),
                );
            }
            _ => {
                schema.insert("type".to_string(), Value::String("string".to_string()));
            }
        }

        Value::Object(schema)
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_support::Must;

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_json_schema_generator_basic() {
        let json = r#"{
            "activateable": false,
            "createable": true,
            "custom": false,
            "customSetting": false,
            "deletable": true,
            "deprecatedAndHidden": false,
            "feedEnabled": false,
            "hasSubtypes": false,
            "isSubtype": false,
            "label": "Account",
            "labelPlural": "Accounts",
            "layoutable": true,
            "mergeable": true,
            "mruEnabled": true,
            "name": "Account",
            "queryable": true,
            "replicateable": true,
            "retrieveable": true,
            "searchable": true,
            "triggerable": true,
            "undeletable": true,
            "updateable": true,
            "urls": {},
            "childRelationships": [], "recordTypeInfos": [], "supportedScopes": [], "fields": [
                {
                    "aggregatable": true,
                    "autoNumber": false,
                    "byteLength": 18,
                    "calculated": false,
                    "cascadeDelete": false,
                    "caseSensitive": false,
                    "createable": false,
                    "custom": false,
                    "defaultedOnCreate": true,
                    "dependentPicklist": false,
                    "deprecatedAndHidden": false,
                    "digits": 0,
                    "displayLocationInDecimal": false,
                    "encrypted": false,
                    "externalId": false,
                    "filterable": true,
                    "groupable": true,
                    "highScaleNumber": false,
                    "htmlFormatted": false,
                    "idLookup": true,
                    "label": "Account ID",
                    "length": 18,
                    "name": "Id",
                    "nameField": false,
                    "namePointing": false,
                    "nillable": false,
                    "permissionable": false,
                    "polymorphicForeignKey": false,
                    "precision": 0,
                    "queryByDistance": false,
                    "referenceTo": [],
                    "restrictedDelete": false,
                    "restrictedPicklist": false,
                    "scale": 0,
                    "soapType": "tns:ID",
                    "sortable": true,
                    "type": "id",
                    "unique": false,
                    "updateable": false,
                    "writeRequiresMasterRead": false
                },
                {
                    "aggregatable": true,
                    "autoNumber": false,
                    "byteLength": 765,
                    "calculated": false,
                    "cascadeDelete": false,
                    "caseSensitive": false,
                    "createable": true,
                    "custom": false,
                    "defaultedOnCreate": false,
                    "dependentPicklist": false,
                    "deprecatedAndHidden": false,
                    "digits": 0,
                    "displayLocationInDecimal": false,
                    "encrypted": false,
                    "externalId": false,
                    "filterable": true,
                    "groupable": true,
                    "highScaleNumber": false,
                    "htmlFormatted": false,
                    "idLookup": false,
                    "label": "Account Name",
                    "length": 255,
                    "name": "Name",
                    "nameField": true,
                    "namePointing": false,
                    "nillable": false,
                    "permissionable": true,
                    "polymorphicForeignKey": false,
                    "precision": 0,
                    "queryByDistance": false,
                    "referenceTo": [],
                    "restrictedDelete": false,
                    "restrictedPicklist": false,
                    "scale": 0,
                    "soapType": "xsd:string",
                    "sortable": true,
                    "type": "string",
                    "unique": false,
                    "updateable": true,
                    "writeRequiresMasterRead": false
                }
            ]
        }"#;

        let describe: SObjectDescribe = serde_json::from_str(json).must();
        let schema = JsonSchemaGenerator::generate_schema(&describe);

        assert_eq!(schema["$schema"], "http://json-schema.org/draft-07/schema#");
        assert_eq!(schema["title"], "Account");
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["description"], "Account");
        assert_eq!(schema["required"][0], "Name");

        let props = &schema["properties"];
        assert_eq!(props["Id"]["type"], "string");
        assert_eq!(props["Id"]["maxLength"], 18);
        assert_eq!(props["Id"]["readOnly"], true);

        assert_eq!(props["Name"]["type"], "string");
        assert_eq!(props["Name"]["maxLength"], 255);
    }
}
