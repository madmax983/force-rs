//! HTML Form Generator.
//!
//! This module provides a utility to generate an HTML form string
//! from a Salesforce `SObjectDescribe`. This can be used to quickly
//! scaffold a web frontend for inserting or updating Salesforce records.

#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Options for generating the HTML form.
#[derive(Debug, Clone, Default)]
pub struct HtmlFormOptions {
    /// Whether to include a submit button. Defaults to `true`.
    pub include_submit_button: bool,
    /// The CSS class to apply to the form element.
    pub form_class: Option<String>,
    /// The action URL for the form.
    pub action_url: Option<String>,
    /// The method for the form (e.g., "POST"). Defaults to "POST".
    pub method: Option<String>,
}

impl HtmlFormOptions {
    /// Creates a new `HtmlFormOptions` with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            include_submit_button: true,
            form_class: None,
            action_url: None,
            method: Some("POST".to_string()),
        }
    }

    /// Sets whether to include a submit button.
    #[must_use]
    pub fn include_submit_button(mut self, include: bool) -> Self {
        self.include_submit_button = include;
        self
    }

    /// Sets the form CSS class.
    #[must_use]
    pub fn form_class(mut self, class: impl Into<String>) -> Self {
        self.form_class = Some(class.into());
        self
    }

    /// Sets the form action URL.
    #[must_use]
    pub fn action_url(mut self, url: impl Into<String>) -> Self {
        self.action_url = Some(url.into());
        self
    }

    /// Sets the form method.
    #[must_use]
    pub fn method(mut self, method: impl Into<String>) -> Self {
        self.method = Some(method.into());
        self
    }
}

/// Generates an HTML form string from an `SObjectDescribe`.
/// Only includes fields that are `createable`.
#[cfg(feature = "schema")]
pub fn generate_html_form(describe: &SObjectDescribe, options: &HtmlFormOptions) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 256);
    write_html_form(&mut out, describe, options);
    out
}

/// Writes an HTML form directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_html_form(out: &mut String, describe: &SObjectDescribe, options: &HtmlFormOptions) {
    let _ = write!(out, "<form");

    if let Some(ref action) = options.action_url {
        let _ = write!(out, " action=\"{}\"", action);
    }

    let method = options.method.as_deref().unwrap_or("POST");
    let _ = write!(out, " method=\"{}\"", method);

    if let Some(ref class) = options.form_class {
        let _ = write!(out, " class=\"{}\"", class);
    }

    let _ = writeln!(out, ">");
    let _ = writeln!(out, "  <h2>Create {}</h2>", describe.label);

    let mut fields: Vec<&_> = describe.fields.iter().filter(|f| f.createable).collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in fields {
        let _ = writeln!(out, "  <div class=\"form-group\">");
        let _ = write!(out, "    <label for=\"{}\">{}</label>", field.name, field.label);

        // required attribute if not nillable and no default
        let required_attr = if !field.nillable && !field.defaulted_on_create { " required" } else { "" };
        let max_length_attr = if field.length > 0 { format!(" maxlength=\"{}\"", field.length) } else { "".to_string() };

        match field.type_ {
            FieldType::Boolean => {
                let _ = writeln!(out, "\n    <input type=\"checkbox\" id=\"{}\" name=\"{}\"{}>", field.name, field.name, required_attr);
            },
            FieldType::Int | FieldType::Double | FieldType::Currency | FieldType::Percent => {
                let _ = writeln!(out, "\n    <input type=\"number\" id=\"{}\" name=\"{}\"{}{}>", field.name, field.name, required_attr, max_length_attr);
            },
            FieldType::Date => {
                 let _ = writeln!(out, "\n    <input type=\"date\" id=\"{}\" name=\"{}\"{}>", field.name, field.name, required_attr);
            },
            FieldType::Datetime => {
                 let _ = writeln!(out, "\n    <input type=\"datetime-local\" id=\"{}\" name=\"{}\"{}>", field.name, field.name, required_attr);
            },
            FieldType::Email => {
                 let _ = writeln!(out, "\n    <input type=\"email\" id=\"{}\" name=\"{}\"{}{}>", field.name, field.name, required_attr, max_length_attr);
            },
            FieldType::Phone => {
                 let _ = writeln!(out, "\n    <input type=\"tel\" id=\"{}\" name=\"{}\"{}{}>", field.name, field.name, required_attr, max_length_attr);
            },
            FieldType::Url => {
                 let _ = writeln!(out, "\n    <input type=\"url\" id=\"{}\" name=\"{}\"{}{}>", field.name, field.name, required_attr, max_length_attr);
            },
            FieldType::Textarea => {
                 let _ = writeln!(out, "\n    <textarea id=\"{}\" name=\"{}\"{}{}></textarea>", field.name, field.name, required_attr, max_length_attr);
            },
            _ => {
                let _ = writeln!(out, "\n    <input type=\"text\" id=\"{}\" name=\"{}\"{}{}>", field.name, field.name, required_attr, max_length_attr);
            }
        }

        let _ = writeln!(out, "  </div>");
    }

    if options.include_submit_button {
        let _ = writeln!(out, "  <button type=\"submit\">Submit</button>");
    }

    let _ = writeln!(out, "</form>");
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_utils::mock_describe::{
        MockFieldDescribeBuilder, MockSObjectDescribeBuilder,
    };

    #[test]
    fn test_generate_html_form() {
        let describe = MockSObjectDescribeBuilder::new("Account")
            .field(
                MockFieldDescribeBuilder::new("Id", FieldType::Id)
                    .label("Account ID")
                    .length(18)
                    .nillable(false)
                    .createable(false) // Should be excluded
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("Name", FieldType::String)
                    .label("Account Name")
                    .length(255)
                    .nillable(false)
                    .createable(true)
                    .defaulted_on_create(false)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("IsActive", FieldType::Boolean)
                    .label("Active")
                    .length(0)
                    .nillable(false)
                    .createable(true)
                    .defaulted_on_create(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("NumberOfEmployees", FieldType::Int)
                    .label("Employees")
                    .length(0)
                    .nillable(true)
                    .createable(true)
                    .build(),
            )
            .build();

        let mut options = HtmlFormOptions::new();
        options = options.action_url("/api/accounts").form_class("sf-form");

        let form = generate_html_form(&describe, &options);

        // Basic structure
        assert!(form.contains("<form action=\"/api/accounts\" method=\"POST\" class=\"sf-form\">"));
        assert!(form.contains("<h2>Create Account</h2>"));

        // Excluded uncreateable field
        assert!(!form.contains("Account ID"));

        // Required field string with length
        assert!(form.contains("<label for=\"Name\">Account Name</label>"));
        assert!(form.contains("<input type=\"text\" id=\"Name\" name=\"Name\" required maxlength=\"255\">"));

        // Boolean field, not required because it has default
        assert!(form.contains("<label for=\"IsActive\">Active</label>"));
        assert!(form.contains("<input type=\"checkbox\" id=\"IsActive\" name=\"IsActive\">"));

        // Integer field, not required because nillable
        assert!(form.contains("<label for=\"NumberOfEmployees\">Employees</label>"));
        assert!(form.contains("<input type=\"number\" id=\"NumberOfEmployees\" name=\"NumberOfEmployees\">"));

        // Submit button
        assert!(form.contains("<button type=\"submit\">Submit</button>"));
        assert!(form.contains("</form>"));
    }
}
