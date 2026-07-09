#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Options for HTML form generation.
#[cfg(feature = "schema")]
#[derive(Debug, Clone)]
pub struct HtmlFormOptions {
    /// CSS class to apply to the form element
    pub form_class: Option<String>,
    /// CSS class to apply to form groups (wrappers around label+input)
    pub group_class: Option<String>,
    /// CSS class to apply to input elements
    pub input_class: Option<String>,
    /// CSS class to apply to the submit button
    pub button_class: Option<String>,
    /// Text for the submit button
    pub submit_text: String,
}

#[cfg(feature = "schema")]
impl Default for HtmlFormOptions {
    fn default() -> Self {
        Self {
            form_class: Some("sfdc-form".to_string()),
            group_class: Some("form-group".to_string()),
            input_class: Some("form-control".to_string()),
            button_class: Some("btn btn-primary".to_string()),
            submit_text: "Submit".to_string(),
        }
    }
}

/// Generates an HTML form based on an SObject describe.
#[cfg(feature = "schema")]
pub fn generate_html_form(describe: &SObjectDescribe, options: &HtmlFormOptions) -> String {
    let mut out = String::with_capacity(1024);

    let form_attr = options
        .form_class
        .as_deref()
        .map_or(String::new(), |c| format!(" class=\"{}\"", c));
    let _ = writeln!(out, "<form id=\"{}Form\"{}>", describe.name, form_attr);

    let mut sorted_fields: Vec<&_> = describe.fields.iter().collect();
    sorted_fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in sorted_fields {
        if !field.createable && !field.updateable {
            continue;
        }

        let group_attr = options
            .group_class
            .as_deref()
            .map_or(String::new(), |c| format!(" class=\"{}\"", c));
        let _ = writeln!(out, "  <div{}>", group_attr);

        // Label
        let _ = writeln!(
            out,
            "    <label for=\"{}\">{}</label>",
            field.name, field.label
        );

        let input_attr = options
            .input_class
            .as_deref()
            .map_or(String::new(), |c| format!(" class=\"{}\"", c));
        let required_attr =
            if !field.nillable && field.createable && field.type_ != FieldType::Boolean {
                " required"
            } else {
                ""
            };

        match field.type_ {
            FieldType::Boolean => {
                let _ = writeln!(
                    out,
                    "    <input type=\"checkbox\" id=\"{0}\" name=\"{0}\"{1} />",
                    field.name, input_attr
                );
            }
            FieldType::Picklist | FieldType::Combobox | FieldType::Multipicklist => {
                // In a real scenario, we might iterate over field.picklist_values
                let _ = writeln!(
                    out,
                    "    <select id=\"{0}\" name=\"{0}\"{1}{2}>",
                    field.name, input_attr, required_attr
                );
                let _ = writeln!(out, "      <option value=\"\">-- Select --</option>");
                let _ = writeln!(out, "    </select>");
            }
            FieldType::Textarea => {
                let _ = writeln!(
                    out,
                    "    <textarea id=\"{0}\" name=\"{0}\"{1}{2}></textarea>",
                    field.name, input_attr, required_attr
                );
            }
            _ => {
                let input_type = match field.type_ {
                    FieldType::Email => "email",
                    FieldType::Phone => "tel",
                    FieldType::Url => "url",
                    FieldType::Int
                    | FieldType::Double
                    | FieldType::Currency
                    | FieldType::Percent => "number",
                    FieldType::Date => "date",
                    FieldType::Datetime => "datetime-local",
                    _ => "text",
                };
                let _ = writeln!(
                    out,
                    "    <input type=\"{}\" id=\"{1}\" name=\"{1}\"{2}{3} />",
                    input_type, field.name, input_attr, required_attr
                );
            }
        }

        let _ = writeln!(out, "  </div>");
    }

    let btn_attr = options
        .button_class
        .as_deref()
        .map_or(String::new(), |c| format!(" class=\"{}\"", c));
    let _ = writeln!(
        out,
        "  <button type=\"submit\"{}>{}</button>",
        btn_attr, options.submit_text
    );
    let _ = write!(out, "</form>");

    out
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_utils::mock_describe::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};

    #[test]
    fn test_generate_html_form() {
        let describe = MockSObjectDescribeBuilder::new("Contact")
            .field(
                MockFieldDescribeBuilder::new("LastName", FieldType::String)
                    .label("Last Name")
                    .nillable(false)
                    .createable(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("Email", FieldType::Email)
                    .label("Email Address")
                    .nillable(true)
                    .createable(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("IsActive", FieldType::Boolean)
                    .label("Active")
                    .nillable(false)
                    .createable(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("Description", FieldType::Textarea)
                    .label("Description")
                    .nillable(true)
                    .createable(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("Level", FieldType::Picklist)
                    .label("Level")
                    .nillable(true)
                    .createable(true)
                    .build(),
            )
            .build();

        let options = HtmlFormOptions::default();
        let html = generate_html_form(&describe, &options);

        assert!(html.contains("<form id=\"ContactForm\" class=\"sfdc-form\">"));
        assert!(html.contains("<label for=\"LastName\">Last Name</label>"));
        assert!(html.contains("<input type=\"text\" id=\"LastName\" name=\"LastName\" class=\"form-control\" required />"));
        assert!(html.contains(
            "<input type=\"email\" id=\"Email\" name=\"Email\" class=\"form-control\" />"
        ));
        assert!(html.contains(
            "<input type=\"checkbox\" id=\"IsActive\" name=\"IsActive\" class=\"form-control\" />"
        ));
        assert!(html.contains(
            "<textarea id=\"Description\" name=\"Description\" class=\"form-control\"></textarea>"
        ));
        assert!(html.contains("<select id=\"Level\" name=\"Level\" class=\"form-control\">"));
        assert!(html.contains("<button type=\"submit\" class=\"btn btn-primary\">Submit</button>"));
    }
}
