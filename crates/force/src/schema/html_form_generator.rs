//! HTML Form Generator for Salesforce SObject Describe metadata.
//!
//! Generates a basic HTML form template for an SObject.

#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Basic HTML escaping
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Generates an HTML form from an SObject describe result.
#[cfg(feature = "schema")]
#[allow(clippy::too_many_lines)]
pub fn generate_html_form(describe: &SObjectDescribe) -> String {
    let mut out = String::new();
    let escaped_name = escape_html(&describe.name);
    let _ = writeln!(out, "<form id=\"{}_form\">", escaped_name);

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| a.name.cmp(&b.name));

    for field in fields {
        if !field.createable || field.auto_number || field.calculated {
            continue;
        }

        let escaped_field_name = escape_html(&field.name);
        let escaped_field_label = escape_html(&field.label);

        let _ = writeln!(out, "  <div class=\"form-group\">");
        let _ = writeln!(
            out,
            "    <label for=\"{}\">{}</label>",
            escaped_field_name, escaped_field_label
        );

        match field.type_ {
            FieldType::Boolean => {
                let _ = writeln!(
                    out,
                    "    <input type=\"checkbox\" id=\"{}\" name=\"{}\" />",
                    escaped_field_name, escaped_field_name
                );
            }
            FieldType::Picklist | FieldType::Multipicklist => {
                let _ = writeln!(
                    out,
                    "    <select id=\"{}\" name=\"{}\">",
                    escaped_field_name, escaped_field_name
                );
                if let Some(ref values) = field.picklist_values {
                    for pv in values {
                        if pv.active {
                            let escaped_val = escape_html(&pv.value);
                            let escaped_pv_label = escape_html(&pv.label);
                            let _ = writeln!(
                                out,
                                "      <option value=\"{}\">{}</option>",
                                escaped_val, escaped_pv_label
                            );
                        }
                    }
                }
                let _ = writeln!(out, "    </select>");
            }
            FieldType::Textarea => {
                let _ = writeln!(
                    out,
                    "    <textarea id=\"{}\" name=\"{}\"></textarea>",
                    escaped_field_name, escaped_field_name
                );
            }
            FieldType::Date => {
                let _ = writeln!(
                    out,
                    "    <input type=\"date\" id=\"{}\" name=\"{}\" />",
                    escaped_field_name, escaped_field_name
                );
            }
            FieldType::Datetime => {
                let _ = writeln!(
                    out,
                    "    <input type=\"datetime-local\" id=\"{}\" name=\"{}\" />",
                    escaped_field_name, escaped_field_name
                );
            }
            FieldType::Email => {
                let _ = writeln!(
                    out,
                    "    <input type=\"email\" id=\"{}\" name=\"{}\" />",
                    escaped_field_name, escaped_field_name
                );
            }
            FieldType::Phone => {
                let _ = writeln!(
                    out,
                    "    <input type=\"tel\" id=\"{}\" name=\"{}\" />",
                    escaped_field_name, escaped_field_name
                );
            }
            FieldType::Url => {
                let _ = writeln!(
                    out,
                    "    <input type=\"url\" id=\"{}\" name=\"{}\" />",
                    escaped_field_name, escaped_field_name
                );
            }
            FieldType::Int | FieldType::Double | FieldType::Percent | FieldType::Currency => {
                let _ = writeln!(
                    out,
                    "    <input type=\"number\" id=\"{}\" name=\"{}\" />",
                    escaped_field_name, escaped_field_name
                );
            }
            _ => {
                let _ = writeln!(
                    out,
                    "    <input type=\"text\" id=\"{}\" name=\"{}\" />",
                    escaped_field_name, escaped_field_name
                );
            }
        }
        let _ = writeln!(out, "  </div>");
    }
    let _ = writeln!(out, "  <button type=\"submit\">Submit</button>");
    let _ = writeln!(out, "</form>");

    out
}

#[cfg(all(test, feature = "schema"))]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;
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
        serde_json::from_value(describe_json).must()
    }

    fn mock_field(name: &str, field_type: &str, createable: bool) -> serde_json::Value {
        json!({
            "name": name,
            "type": field_type,
            "referenceTo": [],
            "label": format!("{} Label", name),
            "custom": false,
            "nillable": true,
            "defaultedOnCreate": false,
            "calculated": false,
            "createable": createable, "autoNumber": false, "aggregatable": true, "byteLength": 18,
            "cascadeDelete": false, "caseSensitive": false,
            "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": true, "length": 18, "nameField": false, "namePointing": false,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:ID",
            "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false
        })
    }

    #[test]
    fn test_generate_html_form() {
        let describe = create_mock_describe(&json!([
            mock_field("Id", "id", false),
            mock_field("Name<Script>", "string", true),
            mock_field("IsActive", "boolean", true)
        ]));

        let html = generate_html_form(&describe);
        assert!(html.contains("<form id=\"Account_form\">"));
        assert!(!html.contains("id=\"Id\"")); // createable = false
        assert!(html.contains("id=\"Name&lt;Script&gt;\""));
        assert!(html.contains("type=\"text\""));
        assert!(html.contains("id=\"IsActive\""));
        assert!(html.contains("type=\"checkbox\""));
    }
}
