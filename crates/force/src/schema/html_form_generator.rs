#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates an HTML `<form>` definition from an SObject describe result.
#[cfg(feature = "schema")]
pub fn generate_html_form(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_html_form(&mut out, describe);
    out
}

/// Writes an HTML `<form>` definition from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_html_form(out: &mut String, describe: &SObjectDescribe) {
    let _ = writeln!(
        out,
        "<form id=\"{}_form\" action=\"/submit\" method=\"POST\">",
        describe.name
    );
    let _ = writeln!(out, "  <h2>{} Form</h2>", describe.label);

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in fields {
        if !field.createable {
            continue;
        }

        let _ = writeln!(out, "  <div class=\"form-group\">");
        let required_attr = if field.nillable { "" } else { " required" };
        let _ = writeln!(
            out,
            "    <label for=\"{}\">{}</label>",
            field.name, field.label
        );

        match field.type_ {
            FieldType::Picklist | FieldType::Multipicklist | FieldType::Combobox => {
                let multiple = if field.type_ == FieldType::Multipicklist {
                    " multiple"
                } else {
                    ""
                };
                let _ = writeln!(
                    out,
                    "    <select id=\"{}\" name=\"{}\"{}{}>",
                    field.name, field.name, multiple, required_attr
                );
                if let Some(picklist_values) = &field.picklist_values {
                    for pv in picklist_values {
                        if pv.active {
                            let _ = writeln!(
                                out,
                                "      <option value=\"{}\">{}</option>",
                                pv.value, pv.label
                            );
                        }
                    }
                }
                let _ = writeln!(out, "    </select>");
            }
            FieldType::Textarea => {
                let max_len = if field.length > 0 {
                    format!(" maxlength=\"{}\"", field.length)
                } else {
                    String::new()
                };
                let _ = writeln!(
                    out,
                    "    <textarea id=\"{}\" name=\"{}\"{}{}></textarea>",
                    field.name, field.name, max_len, required_attr
                );
            }
            FieldType::Boolean => {
                let _ = writeln!(
                    out,
                    "    <input type=\"checkbox\" id=\"{}\" name=\"{}\"{}>",
                    field.name, field.name, required_attr
                );
            }
            _ => {
                let input_type = map_input_type(&field.type_);
                let max_len = if field.length > 0 && is_text_input(input_type) {
                    format!(" maxlength=\"{}\"", field.length)
                } else {
                    String::new()
                };
                let _ = writeln!(
                    out,
                    "    <input type=\"{}\" id=\"{}\" name=\"{}\"{}{}>",
                    input_type, field.name, field.name, max_len, required_attr
                );
            }
        }
        let _ = writeln!(out, "  </div>");
    }

    let _ = writeln!(out, "  <button type=\"submit\">Submit</button>");
    let _ = writeln!(out, "</form>");
}

/// Maps a Salesforce `FieldType` to an HTML input type.
#[cfg(feature = "schema")]
fn map_input_type(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Email => "email",
        FieldType::Phone => "tel",
        FieldType::Url => "url",
        FieldType::Date => "date",
        FieldType::Datetime => "datetime-local",
        FieldType::Time => "time",
        FieldType::Int | FieldType::Double | FieldType::Currency | FieldType::Percent => "number",
        _ => "text",
    }
}

#[cfg(feature = "schema")]
fn is_text_input(input_type: &str) -> bool {
    matches!(input_type, "text" | "email" | "tel" | "url")
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::types::describe::{FieldDescribe, PicklistValue};

    fn mock_field(
        name: &str,
        label: &str,
        type_: FieldType,
        nillable: bool,
        createable: bool,
        length: i32,
    ) -> FieldDescribe {
        FieldDescribe {
            aggregatable: true,
            auto_number: false,
            byte_length: 18,
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
            id_lookup: name == "Id",
            inline_help_text: None,
            label: label.to_string(),
            length,
            mask: None,
            mask_type: None,
            name: name.to_string(),
            name_field: name == "Name",
            name_pointing: false,
            nillable,
            permissionable: false,
            picklist_values: None,
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
            updateable: createable,
            write_requires_master_read: false,
        }
    }

    #[test]
    fn test_html_form_generator() {
        let mut picklist_field = mock_field(
            "Status",
            "Account Status",
            FieldType::Picklist,
            true,
            true,
            40,
        );
        picklist_field.picklist_values = Some(vec![
            PicklistValue {
                active: true,
                default_value: false,
                label: "Active".to_string(),
                valid_for: None,
                value: "Active".to_string(),
            },
            PicklistValue {
                active: true,
                default_value: false,
                label: "Inactive".to_string(),
                valid_for: None,
                value: "Inactive".to_string(),
            },
        ]);

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
                mock_field("Id", "Account ID", FieldType::Id, false, false, 18),
                mock_field("Name", "Account Name", FieldType::String, false, true, 255),
                mock_field(
                    "NumberOfEmployees",
                    "Employees",
                    FieldType::Int,
                    true,
                    true,
                    0,
                ),
                mock_field("IsActive", "Active", FieldType::Boolean, true, true, 0),
                mock_field(
                    "Description",
                    "Description",
                    FieldType::Textarea,
                    true,
                    true,
                    1000,
                ),
                picklist_field,
            ],
        };

        let html = generate_html_form(&describe);

        assert!(html.contains("<form id=\"Account_form\" action=\"/submit\" method=\"POST\">"));
        assert!(html.contains("<h2>Account Form</h2>"));

        // Id should not be there
        assert!(!html.contains("name=\"Id\""));

        // Name should be there, text, required, maxlength
        assert!(html.contains(
            "<input type=\"text\" id=\"Name\" name=\"Name\" maxlength=\"255\" required>"
        ));

        // NumberOfEmployees should be there, number
        assert!(html.contains(
            "<input type=\"number\" id=\"NumberOfEmployees\" name=\"NumberOfEmployees\">"
        ));

        // IsActive boolean
        assert!(html.contains("<input type=\"checkbox\" id=\"IsActive\" name=\"IsActive\">"));

        // Description textarea
        assert!(html.contains(
            "<textarea id=\"Description\" name=\"Description\" maxlength=\"1000\"></textarea>"
        ));

        // Picklist
        assert!(html.contains("<select id=\"Status\" name=\"Status\">"));
        assert!(html.contains("<option value=\"Active\">Active</option>"));
        assert!(html.contains("<option value=\"Inactive\">Inactive</option>"));
    }
}
