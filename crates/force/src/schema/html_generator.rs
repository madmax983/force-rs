//! HTML Exporter.
//!
//! This module provides a utility to generate an HTML page containing a Data Dictionary
//! from Salesforce SObjectDescribe metadata.

use crate::schema::analyze_schema;
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates an HTML data dictionary for the given SObject describe metadata.
#[allow(clippy::too_many_lines)]
#[must_use]
pub fn generate_html_dictionary(describe: &SObjectDescribe) -> String {
    let mut html = String::with_capacity(4096);
    let insights = analyze_schema(describe);

    let _ = write!(
        html,
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Schema Report: {label}</title>
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; line-height: 1.6; color: #333; max-width: 1200px; margin: 0 auto; padding: 2rem; }}
        h1, h2, h3 {{ color: #2c3e50; border-bottom: 1px solid #eee; padding-bottom: 0.5rem; }}
        .stats-grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 1rem; margin: 1.5rem 0; }}
        .stat-card {{ background: #f8f9fa; border: 1px solid #e9ecef; border-radius: 8px; padding: 1rem; text-align: center; }}
        .stat-value {{ font-size: 2rem; font-weight: bold; color: #0070d2; }}
        .stat-label {{ font-size: 0.875rem; color: #6c757d; text-transform: uppercase; letter-spacing: 0.5px; }}
        table {{ width: 100%; border-collapse: collapse; margin-top: 1rem; }}
        th, td {{ padding: 0.75rem; text-align: left; border-bottom: 1px solid #e9ecef; }}
        th {{ background-color: #f8f9fa; font-weight: 600; color: #495057; }}
        tr:hover {{ background-color: #f8f9fa; }}
        .tag {{ display: inline-block; padding: 0.25em 0.6em; font-size: 75%; font-weight: 700; line-height: 1; text-align: center; white-space: nowrap; vertical-align: baseline; border-radius: 0.25rem; }}
        .tag-required {{ color: #fff; background-color: #dc3545; }}
        .tag-custom {{ color: #fff; background-color: #28a745; }}
        .tag-standard {{ color: #212529; background-color: #e2e3e5; }}
        .code {{ font-family: SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace; font-size: 87.5%; color: #e83e8c; word-break: break-word; }}
    </style>
</head>
<body>
    <h1>Schema Report: {label} ({name})</h1>

    <h2>Insights</h2>
    <div class="stats-grid">
        <div class="stat-card">
            <div class="stat-value">{score}</div>
            <div class="stat-label">Complexity Score</div>
        </div>
        <div class="stat-card">
            <div class="stat-value">{total}</div>
            <div class="stat-label">Total Fields</div>
        </div>
        <div class="stat-card">
            <div class="stat-value">{custom}</div>
            <div class="stat-label">Custom Fields</div>
        </div>
        <div class="stat-card">
            <div class="stat-value">{formula}</div>
            <div class="stat-label">Formula Fields</div>
        </div>
    </div>

    <h2>Fields</h2>
    <table>
        <thead>
            <tr>
                <th>Label</th>
                <th>API Name</th>
                <th>Type</th>
                <th>Attributes</th>
            </tr>
        </thead>
        <tbody>
"#,
        label = describe.label,
        name = describe.name,
        score = insights.complexity_score,
        total = insights.total_fields,
        custom = insights.custom_field_count,
        formula = insights.formula_field_count
    );

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in fields {
        let type_str = match field.type_ {
            FieldType::Id | FieldType::Reference => "reference",
            FieldType::Boolean => "boolean",
            FieldType::Int => "int",
            FieldType::Double | FieldType::Currency | FieldType::Percent => "double",
            FieldType::Date => "date",
            FieldType::Datetime => "datetime",
            FieldType::Time => "time",
            _ => "string",
        };

        let mut tags = String::new();
        if !field.nillable && !field.defaulted_on_create && field.name != "Id" {
            tags.push_str(r#"<span class="tag tag-required">Required</span> "#);
        }
        if field.custom {
            tags.push_str(r#"<span class="tag tag-custom">Custom</span> "#);
        } else {
            tags.push_str(r#"<span class="tag tag-standard">Standard</span> "#);
        }

        let _ = write!(
            html,
            r#"
            <tr>
                <td>{label}</td>
                <td class="code">{name}</td>
                <td><code>{type_}</code></td>
                <td>{tags}</td>
            </tr>"#,
            label = field.label,
            name = field.name,
            type_ = type_str,
            tags = tags
        );
    }

    html.push_str(
        r"
        </tbody>
    </table>
</body>
</html>",
    );

    html
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::mock_describe::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};

    #[test]
    fn test_generate_html_dictionary() {
        let mut id_field = MockFieldDescribeBuilder::new("Id", FieldType::Id)
            .nillable(false)
            .build();
        id_field.custom = false;

        let mut name_field = MockFieldDescribeBuilder::new("Name", FieldType::String)
            .nillable(false)
            .build();
        name_field.custom = false;

        let mut custom_field = MockFieldDescribeBuilder::new("Custom__c", FieldType::String)
            .nillable(true)
            .build();
        custom_field.custom = true;

        let describe = MockSObjectDescribeBuilder::new("Account")
            .field(id_field)
            .field(name_field)
            .field(custom_field)
            .build();

        let html = generate_html_dictionary(&describe);

        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Schema Report: Account (Account)"));
        assert!(html.contains("<div class=\"stat-label\">Complexity Score</div>"));
        assert!(html.contains("<th>Label</th>"));
        assert!(html.contains("<td>Id</td>"));
        assert!(html.contains("<td>Name</td>"));
        assert!(html.contains("<td>Custom__c</td>"));
        assert!(html.contains("tag-custom"));
        assert!(html.contains("tag-required"));
    }
}
