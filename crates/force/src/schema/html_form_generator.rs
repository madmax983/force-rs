//! HTML form generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates an HTML form definition from an SObject describe result.
#[cfg(feature = "schema")]
pub fn generate_html_form(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 256);
    write_html_form(&mut out, describe);
    out
}

/// Writes an HTML form definition from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
#[allow(clippy::too_many_lines)]
pub fn write_html_form(out: &mut String, describe: &SObjectDescribe) {
    let _ = writeln!(out, "<!-- Form for {} -->", describe.label);
    let _ = writeln!(out, "<form id=\"{}-form\">", describe.name);

    let mut fields: Vec<&_> = describe.fields.iter().filter(|f| f.createable || f.updateable).collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in fields {
        let required = if field.nillable { "" } else { " required" };
        let _ = writeln!(out, "  <div class=\"form-group\">");
        let _ = writeln!(out, "    <label for=\"{}\">{}</label>", field.name, field.label);

        match field.type_ {
            FieldType::Boolean => {
                let _ = writeln!(out, "    <input type=\"checkbox\" id=\"{}\" name=\"{}\"{}>", field.name, field.name, required);
            }
            FieldType::Date => {
                let _ = writeln!(out, "    <input type=\"date\" id=\"{}\" name=\"{}\"{}>", field.name, field.name, required);
            }
            FieldType::Datetime => {
                let _ = writeln!(out, "    <input type=\"datetime-local\" id=\"{}\" name=\"{}\"{}>", field.name, field.name, required);
            }
            FieldType::Time => {
                let _ = writeln!(out, "    <input type=\"time\" id=\"{}\" name=\"{}\"{}>", field.name, field.name, required);
            }
            FieldType::Int | FieldType::Double | FieldType::Currency | FieldType::Percent => {
                let _ = writeln!(out, "    <input type=\"number\" id=\"{}\" name=\"{}\"{}>", field.name, field.name, required);
            }
            FieldType::Picklist | FieldType::Multipicklist => {
                let multiple = if field.type_ == FieldType::Multipicklist { " multiple" } else { "" };
                let _ = writeln!(out, "    <select id=\"{}\" name=\"{}\"{}{}>", field.name, field.name, required, multiple);
                if let Some(picklist_values) = &field.picklist_values {
                    for pv in picklist_values {
                        if pv.active {
                            let _ = writeln!(out, "      <option value=\"{}\">{}</option>", pv.value, pv.label);
                        }
                    }
                }
                let _ = writeln!(out, "    </select>");
            }
            FieldType::Textarea => {
                let _ = writeln!(out, "    <textarea id=\"{}\" name=\"{}\"{}></textarea>", field.name, field.name, required);
            }
            FieldType::Email => {
                let _ = writeln!(out, "    <input type=\"email\" id=\"{}\" name=\"{}\"{}>", field.name, field.name, required);
            }
            FieldType::Phone => {
                let _ = writeln!(out, "    <input type=\"tel\" id=\"{}\" name=\"{}\"{}>", field.name, field.name, required);
            }
            FieldType::Url => {
                let _ = writeln!(out, "    <input type=\"url\" id=\"{}\" name=\"{}\"{}>", field.name, field.name, required);
            }
            _ => {
                let _ = writeln!(out, "    <input type=\"text\" id=\"{}\" name=\"{}\"{}>", field.name, field.name, required);
            }
        }

        if let Some(help) = &field.inline_help_text {
            let _ = writeln!(out, "    <small class=\"help-text\">{}</small>", help);
        }
        let _ = writeln!(out, "  </div>");
    }

    let _ = writeln!(out, "  <button type=\"submit\">Submit</button>");
    out.push_str("</form>\n");
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;
    use serde_json::json;

    #[test]
    fn test_html_form_generator() {
        let describe_json = json!({
            "name": "Account",
            "label": "Account",
            "custom": false,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "labelPlural": "Accounts", "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "queryable": true, "searchable": false, "triggerable": false, "undeletable": false,
            "updateable": true,
            "urls": {},
            "childRelationships": [],
            "recordTypeInfos": [],
            "fields": [
                {
                    "name": "Name", "type": "string", "label": "Name", "custom": false, "nillable": false,
                    "aggregatable": true, "autoNumber": false, "byteLength": 80, "cascadeDelete": false,
                    "caseSensitive": false, "createable": true, "defaultedOnCreate": false, "dependentPicklist": false,
                    "deprecatedAndHidden": false, "digits": 0, "displayLocationInDecimal": false, "encrypted": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 80, "nameField": true, "namePointing": false, "permissionable": false,
                    "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false, "restrictedDelete": false,
                    "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string", "sortable": true, "updateable": true,
                    "writeRequiresMasterRead": false,
                    "referenceTo": [],
                    "externalId": false,
                    "unique": false,
                    "calculated": false
                },
                {
                    "name": "IsActive", "type": "boolean", "label": "IsActive", "custom": false, "nillable": true,
                    "aggregatable": true, "autoNumber": false, "byteLength": 0, "cascadeDelete": false,
                    "caseSensitive": false, "createable": true, "defaultedOnCreate": false, "dependentPicklist": false,
                    "deprecatedAndHidden": false, "digits": 0, "displayLocationInDecimal": false, "encrypted": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 0, "nameField": false, "namePointing": false, "permissionable": false,
                    "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false, "restrictedDelete": false,
                    "restrictedPicklist": false, "scale": 0, "soapType": "xsd:boolean", "sortable": true, "updateable": true,
                    "writeRequiresMasterRead": false,
                    "referenceTo": [],
                    "externalId": false,
                    "unique": false,
                    "calculated": false
                },
                {
                    "name": "Industry", "type": "picklist", "label": "Industry", "custom": false, "nillable": true,
                    "aggregatable": true, "autoNumber": false, "byteLength": 255, "cascadeDelete": false,
                    "caseSensitive": false, "createable": true, "defaultedOnCreate": false, "dependentPicklist": false,
                    "deprecatedAndHidden": false, "digits": 0, "displayLocationInDecimal": false, "encrypted": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 255, "nameField": false, "namePointing": false, "permissionable": false,
                    "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false, "restrictedDelete": false,
                    "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string", "sortable": true, "updateable": true,
                    "writeRequiresMasterRead": false,
                    "referenceTo": [],
                    "externalId": false,
                    "unique": false,
                    "calculated": false,
                    "picklistValues": [
                        { "active": true, "defaultValue": false, "label": "Tech", "value": "Technology" }
                    ]
                }
            ]
        });
        let describe: SObjectDescribe = serde_json::from_value(describe_json).must();

        let html = generate_html_form(&describe);

        assert!(html.contains("<form id=\"Account-form\">"));
        assert!(html.contains("<input type=\"text\" id=\"Name\" name=\"Name\" required>"));
        assert!(html.contains("<input type=\"checkbox\" id=\"IsActive\" name=\"IsActive\">"));
        assert!(html.contains("<select id=\"Industry\" name=\"Industry\">"));
        assert!(html.contains("<option value=\"Technology\">Tech</option>"));
    }
}
