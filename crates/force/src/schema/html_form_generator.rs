//! HTML Form Generator.
//!
//! Generates a ready-to-use HTML5 `<form>` for a given Salesforce SObject.
#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates an HTML5 form for the given SObject describe metadata.
#[cfg(feature = "schema")]
pub fn generate_html_form(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 256);
    write_html_form(&mut out, describe);
    out
}

/// Writes an HTML5 form for the given SObject describe metadata directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_html_form(out: &mut String, describe: &SObjectDescribe) {
    let _ = writeln!(
        out,
        "<form id=\"{}_form\" method=\"POST\" action=\"/submit\">",
        describe.name
    );
    let _ = writeln!(out, "  <h2>Create {}</h2>", describe.label);

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in fields {
        if !field.createable || field.auto_number || field.calculated {
            continue;
        }

        let required = if !field.nillable && !field.defaulted_on_create {
            " required"
        } else {
            ""
        };
        let input_id = format!("field_{}", field.name);

        let _ = writeln!(out, "  <div class=\"form-group\">");
        let _ = writeln!(
            out,
            "    <label for=\"{}\">{}{}</label>",
            input_id,
            field.label,
            if required.is_empty() { "" } else { " *" }
        );

        match field.type_ {
            FieldType::Textarea => {
                let _ = writeln!(
                    out,
                    "    <textarea id=\"{}\" name=\"{}\" maxlength=\"{}\"{}></textarea>",
                    input_id, field.name, field.length, required
                );
            }
            FieldType::Boolean => {
                let _ = writeln!(
                    out,
                    "    <input type=\"checkbox\" id=\"{}\" name=\"{}\">",
                    input_id, field.name
                );
            }
            FieldType::Picklist | FieldType::Multipicklist | FieldType::Combobox => {
                let multiple = if matches!(field.type_, FieldType::Multipicklist) {
                    " multiple"
                } else {
                    ""
                };
                let _ = writeln!(
                    out,
                    "    <select id=\"{}\" name=\"{}\"{}{}>",
                    input_id, field.name, multiple, required
                );
                if required.is_empty() {
                    let _ = writeln!(out, "      <option value=\"\">-- None --</option>");
                }
                if let Some(values) = &field.picklist_values {
                    for val in values {
                        if val.active {
                            let selected = if val.default_value { " selected" } else { "" };
                            let _ = writeln!(
                                out,
                                "      <option value=\"{}\"{}>{}</option>",
                                val.value, selected, val.label
                            );
                        }
                    }
                }
                let _ = writeln!(out, "    </select>");
            }
            _ => {
                let input_type = match field.type_ {
                    FieldType::Email => "email",
                    FieldType::Url => "url",
                    FieldType::Phone => "tel",
                    FieldType::Int
                    | FieldType::Double
                    | FieldType::Currency
                    | FieldType::Percent => "number",
                    FieldType::Date => "date",
                    FieldType::Datetime => "datetime-local",
                    FieldType::Time => "time",
                    _ => "text",
                };
                let mut attrs = format!(
                    "type=\"{}\" id=\"{}\" name=\"{}\"{}",
                    input_type, input_id, field.name, required
                );
                if matches!(
                    field.type_,
                    FieldType::String | FieldType::Email | FieldType::Url | FieldType::Phone
                ) && field.length > 0
                {
                    let _ = write!(attrs, " maxlength=\"{}\"", field.length);
                }
                let _ = writeln!(out, "    <input {}>", attrs);
            }
        }
        if let Some(help) = &field.inline_help_text {
            let _ = writeln!(
                out,
                "    <small class=\"form-text text-muted\">{}</small>",
                help
            );
        }
        let _ = writeln!(out, "  </div>");
    }

    let _ = writeln!(out, "  <button type=\"submit\">Submit</button>");
    let _ = writeln!(out, "</form>");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::describe::FieldDescribe;

    fn mock_field(
        name: &str,
        type_: FieldType,
        length: i32,
        createable: bool,
        picklist: Option<Vec<crate::types::describe::PicklistValue>>,
    ) -> FieldDescribe {
        FieldDescribe {
            aggregatable: true,
            auto_number: false,
            byte_length: length,
            calculated: false,
            calculated_formula: None,
            cascade_delete: false,
            case_sensitive: false,
            compound_field_name: None,
            controller_name: None,
            createable,
            custom: false,
            default_value: None,
            default_value_formula: None,
            defaulted_on_create: false,
            dependent_picklist: false,
            deprecated_and_hidden: false,
            digits: 0,
            display_location_in_decimal: false,
            encrypted: false,
            external_id: false,
            extra_type_info: None,
            filterable: true,
            filtered_lookup_info: None,
            formula_treat_blanks_as: None,
            groupable: true,
            high_scale_number: false,
            html_formatted: false,
            id_lookup: false,
            inline_help_text: None,
            label: name.to_string(),
            length,
            mask: None,
            mask_type: None,
            name: name.to_string(),
            name_field: name == "Name",
            name_pointing: false,
            nillable: true,
            permissionable: false,
            picklist_values: picklist,
            polymorphic_foreign_key: false,
            precision: 0,
            query_by_distance: false,
            reference_target_field: None,
            reference_to: vec![],
            relationship_name: None,
            relationship_order: None,
            restricted_delete: false,
            restricted_picklist: false,
            scale: 0,
            search_prefixes_supported: None,
            soap_type: "xsd:string".to_string(),
            sortable: true,
            type_,
            unique: false,
            updateable: true,
            write_requires_master_read: false,
        }
    }

    #[test]
    fn test_generate_html_form() {
        let describe = SObjectDescribe {
            activateable: false,
            createable: true,
            custom: false,
            custom_setting: false,
            deletable: true,
            deprecated_and_hidden: false,
            feed_enabled: false,
            has_subtypes: false,
            is_subtype: false,
            key_prefix: Some("001".to_string()),
            label: "Account".to_string(),
            label_plural: "Accounts".to_string(),
            layoutable: true,
            mergeable: true,
            mru_enabled: true,
            name: "Account".to_string(),
            queryable: true,
            replicateable: true,
            retrieveable: true,
            searchable: true,
            triggerable: true,
            undeletable: true,
            updateable: true,
            urls: std::collections::HashMap::new(),
            child_relationships: vec![],
            record_type_infos: vec![],
            fields: vec![
                mock_field("Name", FieldType::String, 255, true, None),
                mock_field("IsActive", FieldType::Boolean, 0, true, None),
                mock_field("AutoNum", FieldType::String, 0, false, None),
            ],
        };

        let form = generate_html_form(&describe);
        assert!(form.contains("<form id=\"Account_form\" method=\"POST\" action=\"/submit\">"));
        assert!(form.contains("<h2>Create Account</h2>"));
        assert!(
            form.contains(
                "<input type=\"text\" id=\"field_Name\" name=\"Name\" maxlength=\"255\">"
            )
        );
        assert!(form.contains("<input type=\"checkbox\" id=\"field_IsActive\" name=\"IsActive\">"));
        assert!(!form.contains("AutoNum")); // Not createable
    }
}
