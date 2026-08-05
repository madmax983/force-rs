//! LLM Context Generator for Salesforce Schemas
//!
//! This module provides a utility to convert an `SObjectDescribe` into a highly condensed,
//! token-optimized textual representation specifically tailored for Large Language Models (LLMs).
//!
//! # The Spark
//! We have schema parsers and visualizers, but what if developers want to prompt an LLM
//! to write SOQL or Apex for their specific org? Passing a full JSON `describe` object consumes
//! thousands of tokens and includes irrelevant metadata. `generate_llm_context` strips away
//! the noise and creates a dense, readable string that gives an LLM exactly what it needs to
//! know about an SObject's structure, fields, and relationships.

use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Options for configuring the LLM context generation.
#[derive(Debug, Clone)]
pub struct LlmContextOptions {
    /// Whether to include field labels (can be omitted to save tokens).
    pub include_labels: bool,
    /// Whether to include relationship information.
    pub include_relationships: bool,
    /// Only include custom fields.
    pub custom_fields_only: bool,
}

impl Default for LlmContextOptions {
    fn default() -> Self {
        Self {
            include_labels: true,
            include_relationships: true,
            custom_fields_only: false,
        }
    }
}

/// Generates a token-optimized string representation of an SObject schema for LLM prompting.
///
/// # Arguments
///
/// * `describe` - The SObject description.
/// * `options` - Configuration options to control verbosity.
///
/// # Returns
///
/// A dense string formatted for LLM consumption.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn generate_llm_context(describe: &SObjectDescribe, options: &LlmContextOptions) -> String {
    let mut context = String::with_capacity(1024);

    let _ = writeln!(
        context,
        "SObject: {} (Custom: {}, Label: {})",
        describe.name, describe.custom, describe.label
    );
    let _ = writeln!(context, "Fields:");

    // ⚡ Bolt: Collecting references to fields into a Vec avoids the costly `describe.fields.clone()` heap allocation since we just need to sort them before iterating.
    let mut sorted_fields: Vec<&crate::types::describe::FieldDescribe> =
        describe.fields.iter().collect();
    sorted_fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in sorted_fields {
        if options.custom_fields_only && !field.custom {
            continue;
        }

        let type_str = match field.type_ {
            FieldType::String => "string",
            FieldType::Id => "id",
            FieldType::Reference => "reference",
            FieldType::Int => "int",
            FieldType::Double => "double",
            FieldType::Boolean => "boolean",
            FieldType::Date => "date",
            FieldType::Datetime => "datetime",
            FieldType::Picklist => "picklist",
            FieldType::Multipicklist => "multipicklist",
            FieldType::Currency => "currency",
            FieldType::Email => "email",
            FieldType::Phone => "phone",
            FieldType::Url => "url",
            FieldType::Textarea => "textarea",
            FieldType::Location => "location",
            FieldType::Address => "address",
            FieldType::Base64 => "base64",
            FieldType::Combobox => "combobox",
            FieldType::Encryptedstring => "encrypted",
            FieldType::Datacategorygroupreference => "datacategory",
            FieldType::Percent => "percent",
            FieldType::Time => "time",
            FieldType::AnyType => "any",
        };

        // ⚡ Bolt: Eliminate intermediate Vec allocation and format! heap allocation for modifiers
        let mut modifiers_str = String::new();
        let mut has_mod = false;

        let mut add_mod = |m: &str| {
            if has_mod {
                modifiers_str.push(',');
            } else {
                modifiers_str.push_str(" [");
                has_mod = true;
            }
            modifiers_str.push_str(m);
        };

        if !field.nillable {
            add_mod("req");
        }
        if field.unique {
            add_mod("uniq");
        }
        if field.external_id {
            add_mod("ext_id");
        }
        if field.calculated {
            add_mod("formula");
        }

        if has_mod {
            modifiers_str.push(']');
        }

        let label_str = if options.include_labels {
            format!(" // {}", field.label)
        } else {
            String::new()
        };

        if options.include_relationships && field.type_ == FieldType::Reference {
            let _ = write!(context, "  - {}: {} -> ", field.name, type_str);

            // ⚡ Bolt: Write reference targets directly to the `context` buffer, avoiding a temporary `.join(",")` String allocation.
            let mut first = true;
            for target in &field.reference_to {
                if !first {
                    context.push(',');
                }
                first = false;
                context.push_str(target);
            }

            let _ = writeln!(context, "{}{}", modifiers_str, label_str);
        } else {
            let _ = writeln!(
                context,
                "  - {}: {}{}{}",
                field.name, type_str, modifiers_str, label_str
            );
        }
    }

    if options.include_relationships && !describe.child_relationships.is_empty() {
        let _ = writeln!(context, "Child Relationships:");
        for rel in &describe.child_relationships {
            if let Some(rel_name) = &rel.relationship_name {
                let _ = writeln!(
                    context,
                    "  - {} (Child: {}.{})",
                    rel_name, rel.child_sobject, rel.field
                );
            }
        }
    }

    context
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;
    use serde_json::json;

    fn mock_describe() -> SObjectDescribe {
        use crate::test_utils::mock_describe::{
            MockFieldDescribeBuilder, MockSObjectDescribeBuilder,
        };
        use crate::types::describe::{ChildRelationship, FieldType};

        let mut describe = MockSObjectDescribeBuilder::new("CustomObj__c")
            .field(
                MockFieldDescribeBuilder::new("Id", FieldType::Id)
                    .label("Record ID")
                    .length(18)
                    .byte_length(18)
                    .nillable(false)
                    .createable(false)
                    .defaulted_on_create(true)
                    .updateable(false)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("Name", FieldType::String)
                    .label("Name")
                    .length(80)
                    .byte_length(80)
                    .nillable(false)
                    .createable(true)
                    .updateable(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("Account__c", FieldType::Reference)
                    .label("Account")
                    .length(18)
                    .byte_length(18)
                    .nillable(true)
                    .createable(true)
                    .updateable(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("Amount__c", FieldType::Currency)
                    .label("Amount")
                    .length(0)
                    .byte_length(0)
                    .precision(18)
                    .scale(2)
                    .digits(18)
                    .nillable(true)
                    .createable(true)
                    .updateable(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("ExtId__c", FieldType::String)
                    .label("External ID")
                    .length(255)
                    .byte_length(255)
                    .nillable(false)
                    .createable(true)
                    .updateable(true)
                    .build(),
            )
            .build();

        describe.custom = true;
        describe.label = "Custom Object".to_string();
        describe.fields[2].custom = true;
        describe.fields[2].reference_to = vec!["Account".to_string()];
        describe.fields[3].custom = true;
        describe.fields[4].custom = true;
        describe.fields[4].external_id = true;
        describe.fields[4].unique = true;

        describe.child_relationships = vec![ChildRelationship {
            cascade_delete: false,
            child_sobject: "ChildObj__c".to_string(),
            deprecated_and_hidden: false,
            field: "ParentId__c".to_string(),
            junction_id_list_names: vec![],
            junction_reference_to: vec![],
            relationship_name: Some("Children__r".to_string()),
            restricted_delete: false,
        }];

        describe
    }

    #[test]
    fn test_generate_llm_context_default() {
        let describe = mock_describe();
        let options = LlmContextOptions::default();
        let context = generate_llm_context(&describe, &options);

        // Header
        assert!(context.contains("SObject: CustomObj__c (Custom: true, Label: Custom Object)"));

        // Fields with labels and modifiers
        assert!(context.contains("- Id: id [req] // Record ID"));
        assert!(context.contains("- Name: string [req] // Name"));
        assert!(context.contains("- Account__c: reference -> Account // Account"));
        assert!(context.contains("- Amount__c: currency // Amount"));
        assert!(context.contains("- ExtId__c: string [req,uniq,ext_id] // External ID"));

        // Relationships
        assert!(context.contains("Child Relationships:"));
        assert!(context.contains("- Children__r (Child: ChildObj__c.ParentId__c)"));
    }

    #[test]
    fn test_generate_llm_context_custom_only_no_labels() {
        let describe = mock_describe();
        let options = LlmContextOptions {
            include_labels: false,
            include_relationships: false,
            custom_fields_only: true,
        };
        let context = generate_llm_context(&describe, &options);

        // Should not have Id or Name
        assert!(!context.contains("- Id:"));
        assert!(!context.contains("- Name:"));

        // Custom fields present without labels
        assert!(context.contains("- Account__c: reference\n"));
        assert!(context.contains("- Amount__c: currency\n"));
        assert!(context.contains("- ExtId__c: string [req,uniq,ext_id]\n"));

        // No labels
        assert!(!context.contains("// Amount"));

        // No relationships
        assert!(!context.contains("Child Relationships:"));
        assert!(!context.contains("-> Account"));
    }
}
