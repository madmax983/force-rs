#[cfg(feature = "schema")]
use crate::api::rest::describe::{FieldType, SObjectDescribe};
#[cfg(feature = "schema")]
use serde_json::{Value, json};

#[cfg(feature = "schema")]
/// Experimental utility to generate an OpenAPI 3.0 specification from SObject Describe metadata.
pub struct OpenApiGenerator {
    title: String,
    version: String,
}

#[cfg(feature = "schema")]
impl OpenApiGenerator {
    /// Creates a new generator with default title and version.
    #[must_use]
    pub fn new() -> Self {
        Self {
            title: "Salesforce REST API".to_string(),
            version: "v1.0.0".to_string(),
        }
    }

    /// Sets the title of the OpenAPI document.
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Sets the version of the OpenAPI document.
    #[must_use]
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// Generates the OpenAPI 3.0 specification for the given object.
    #[must_use]
    pub fn generate(&self, describe: &SObjectDescribe) -> Value {
        let mut properties = serde_json::Map::new();

        for field in &describe.fields {
            let mut field_schema = serde_json::Map::new();
            match &field.type_ {
                FieldType::Int => {
                    field_schema.insert("type".to_string(), json!("integer"));
                }
                FieldType::Double | FieldType::Currency | FieldType::Percent => {
                    field_schema.insert("type".to_string(), json!("number"));
                }
                FieldType::Boolean => {
                    field_schema.insert("type".to_string(), json!("boolean"));
                }
                FieldType::Date | FieldType::Datetime => {
                    field_schema.insert("type".to_string(), json!("string"));
                    field_schema.insert("format".to_string(), json!("date-time"));
                }
                _ => {
                    field_schema.insert("type".to_string(), json!("string"));
                }
            }

            if !field.nillable {
                field_schema.insert("nullable".to_string(), json!(false));
            }

            if let Some(help) = &field.inline_help_text {
                field_schema.insert("description".to_string(), json!(help));
            }

            properties.insert(field.name.clone(), Value::Object(field_schema));
        }

        json!({
            "openapi": "3.0.0",
            "info": {
                "title": self.title,
                "version": self.version,
            },
            "components": {
                "schemas": {
                    describe.name.clone(): {
                        "type": "object",
                        "properties": properties
                    }
                }
            },
            "paths": {
                format!("/sobjects/{}", describe.name): {
                    "get": {
                        "summary": format!("Retrieve {} record", describe.label),
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
}

#[cfg(feature = "schema")]
impl Default for OpenApiGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_support::Must;

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_openapi_generator() {
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
            "label": "Account Object",
            "labelPlural": "Accounts",
            "layoutable": true,
            "mergeable": true,
            "mruEnabled": false,
            "name": "Account",
            "queryable": true,
            "replicateable": true,
            "retrieveable": true,
            "searchable": true,
            "triggerable": true,
            "undeletable": true,
            "updateable": true,
            "urls": {},
            "fields": [
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
                    "byteLength": 255,
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
                    "inlineHelpText": "Account Name",
                    "label": "Account Name",
                    "length": 255,
                    "name": "Name",
                    "nameField": true,
                    "namePointing": false,
                    "nillable": true,
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
                },
                {
                    "aggregatable": true,
                    "autoNumber": false,
                    "byteLength": 0,
                    "calculated": false,
                    "cascadeDelete": false,
                    "caseSensitive": false,
                    "createable": true,
                    "custom": false,
                    "defaultedOnCreate": false,
                    "dependentPicklist": false,
                    "deprecatedAndHidden": false,
                    "digits": 18,
                    "displayLocationInDecimal": false,
                    "encrypted": false,
                    "externalId": false,
                    "filterable": true,
                    "groupable": false,
                    "highScaleNumber": false,
                    "htmlFormatted": false,
                    "idLookup": false,
                    "label": "Annual Revenue",
                    "length": 0,
                    "name": "AnnualRevenue",
                    "nameField": false,
                    "namePointing": false,
                    "nillable": true,
                    "permissionable": true,
                    "polymorphicForeignKey": false,
                    "precision": 18,
                    "queryByDistance": false,
                    "referenceTo": [],
                    "restrictedDelete": false,
                    "restrictedPicklist": false,
                    "scale": 0,
                    "soapType": "xsd:double",
                    "sortable": true,
                    "type": "currency",
                    "unique": false,
                    "updateable": true,
                    "writeRequiresMasterRead": false
                },
                {
                    "aggregatable": true,
                    "autoNumber": false,
                    "byteLength": 0,
                    "calculated": false,
                    "cascadeDelete": false,
                    "caseSensitive": false,
                    "createable": true,
                    "custom": false,
                    "defaultedOnCreate": false,
                    "dependentPicklist": false,
                    "deprecatedAndHidden": false,
                    "digits": 8,
                    "displayLocationInDecimal": false,
                    "encrypted": false,
                    "externalId": false,
                    "filterable": true,
                    "groupable": true,
                    "highScaleNumber": false,
                    "htmlFormatted": false,
                    "idLookup": false,
                    "label": "Employees",
                    "length": 0,
                    "name": "NumberOfEmployees",
                    "nameField": false,
                    "namePointing": false,
                    "nillable": true,
                    "permissionable": true,
                    "polymorphicForeignKey": false,
                    "precision": 8,
                    "queryByDistance": false,
                    "referenceTo": [],
                    "restrictedDelete": false,
                    "restrictedPicklist": false,
                    "scale": 0,
                    "soapType": "xsd:int",
                    "sortable": true,
                    "type": "int",
                    "unique": false,
                    "updateable": true,
                    "writeRequiresMasterRead": false
                },
                {
                    "aggregatable": true,
                    "autoNumber": false,
                    "byteLength": 0,
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
                    "idLookup": false,
                    "label": "Deleted",
                    "length": 0,
                    "name": "IsDeleted",
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
                    "soapType": "xsd:boolean",
                    "sortable": true,
                    "type": "boolean",
                    "unique": false,
                    "updateable": false,
                    "writeRequiresMasterRead": false
                }
            ],
            "childRelationships": [],
            "recordTypeInfos": []
        }"#;

        let describe: SObjectDescribe = serde_json::from_str(json).must();
        let generator = OpenApiGenerator::new()
            .with_title("Account API")
            .with_version("v2.0");

        let spec = generator.generate(&describe);

        assert_eq!(spec["openapi"], "3.0.0");
        assert_eq!(spec["info"]["title"], "Account API");
        assert_eq!(spec["info"]["version"], "v2.0");

        let account_schema = &spec["components"]["schemas"]["Account"]["properties"];
        assert_eq!(account_schema["Id"]["type"], "string");
        assert_eq!(account_schema["Id"]["nullable"], false);

        assert_eq!(account_schema["Name"]["type"], "string");
        assert_eq!(account_schema["Name"]["description"], "Account Name");
        assert!(account_schema["Name"]["nullable"].is_null()); // Since true is not explicitly set in the impl

        assert_eq!(account_schema["AnnualRevenue"]["type"], "number");
        assert_eq!(account_schema["NumberOfEmployees"]["type"], "integer");
        assert_eq!(account_schema["IsDeleted"]["type"], "boolean");

        let path = &spec["paths"]["/sobjects/Account"]["get"];
        assert_eq!(path["summary"], "Retrieve Account Object record");
        assert_eq!(
            path["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/Account"
        );
    }
}
