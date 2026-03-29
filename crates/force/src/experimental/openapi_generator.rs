//! OpenAPI Generator for Salesforce SObject Describe metadata.
//!
//! Generates OpenAPI 3.0 schemas and path operations based on Salesforce object schemas.

#[cfg(feature = "schema")]
use crate::api::rest::describe::{FieldDescribe, FieldType, SObjectDescribe};
#[cfg(feature = "schema")]
use std::fmt::Write;

/// Experimental utility to generate OpenAPI specifications from SObject metadata.
#[cfg(feature = "schema")]
pub struct OpenApiGenerator;

#[cfg(feature = "schema")]
impl OpenApiGenerator {
    /// Generates an OpenAPI 3.0 component schema definition for a given SObject.
    pub fn generate_schema(describe: &SObjectDescribe) -> String {
        let mut out = String::with_capacity(describe.fields.len() * 128);

        let _ = writeln!(out, "    {}:", describe.name);
        out.push_str("      type: object\n");
        if !describe.label.is_empty() {
            let _ = writeln!(out, "      description: {}", describe.label);
        }

        // Find required fields for the required array without intermediate Vec allocation
        // ⚡ Bolt: Iterate directly instead of `.collect()`-ing into a `Vec<&str>` to avoid heap allocation.
        let mut has_required = false;
        for field in &describe.fields {
            if !field.nillable
                && !field.defaulted_on_create
                && field.createable
                && field.type_ != FieldType::Id
                && field.type_ != FieldType::Boolean
            {
                if !has_required {
                    out.push_str("      required:\n");
                    has_required = true;
                }
                let _ = writeln!(out, "        - {}", field.name);
            }
        }

        out.push_str("      properties:\n");

        for field in &describe.fields {
            let _ = writeln!(out, "        {}:", field.name);
            Self::write_field_schema(&mut out, field);
        }

        out
    }

    fn write_field_schema(out: &mut String, field: &FieldDescribe) {
        if let Some(help) = &field.inline_help_text {
            let _ = writeln!(out, "          description: {}", help);
        } else {
            let _ = writeln!(out, "          description: {}", field.label);
        }

        if !field.updateable && !field.createable {
            out.push_str("          readOnly: true\n");
        }

        match field.type_ {
            FieldType::String
            | FieldType::Email
            | FieldType::Url
            | FieldType::Phone
            | FieldType::Id
            | FieldType::Reference
            | FieldType::Combobox => {
                out.push_str("          type: string\n");
                if field.length > 0 {
                    let _ = writeln!(out, "          maxLength: {}", field.length);
                }
            }
            FieldType::Textarea => {
                out.push_str("          type: string\n");
            }
            FieldType::Picklist | FieldType::Multipicklist => {
                out.push_str("          type: string\n");
                if let Some(values) = &field.picklist_values {
                    if !values.is_empty() {
                        out.push_str("          enum:\n");
                        for pv in values {
                            let _ = writeln!(out, "            - {}", pv.value);
                        }
                    }
                }
            }
            FieldType::Boolean => {
                out.push_str("          type: boolean\n");
            }
            FieldType::Int => {
                out.push_str("          type: integer\n");
            }
            FieldType::Double | FieldType::Percent | FieldType::Currency => {
                out.push_str("          type: number\n");
            }
            FieldType::Date => {
                out.push_str("          type: string\n");
                out.push_str("          format: date\n");
            }
            FieldType::Datetime => {
                out.push_str("          type: string\n");
                out.push_str("          format: date-time\n");
            }
            FieldType::Base64 => {
                out.push_str("          type: string\n");
                out.push_str("          format: byte\n");
            }
            _ => {
                // Fallback for any other type
                out.push_str("          type: string\n");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Must;

    #[allow(clippy::too_many_lines)]
    #[test]
    fn test_openapi_generator_basic() {
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
        let schema = OpenApiGenerator::generate_schema(&describe);

        assert!(schema.contains("Account:"));
        assert!(schema.contains("type: object"));
        assert!(schema.contains("description: Account"));
        assert!(schema.contains("required:"));
        assert!(schema.contains("- Name"));
        assert!(!schema.contains("- Id")); // Id shouldn't be required
        assert!(schema.contains("Id:"));
        assert!(schema.contains("readOnly: true"));
        assert!(schema.contains("maxLength: 18"));
    }
}
