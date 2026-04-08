//! Protocol Buffers schema generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::api::rest::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates a Protocol Buffers schema definition from an SObject describe result.
#[cfg(feature = "schema")]
#[must_use]
pub fn generate_protobuf_schema(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_protobuf_schema(&mut out, describe);
    out
}

/// Writes a Protocol Buffers schema definition from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_protobuf_schema(out: &mut String, describe: &SObjectDescribe) {
    let _ = writeln!(out, "syntax = \"proto3\";");
    let _ = writeln!(out, "package com.salesforce.schema;");
    let _ = writeln!(out, "");
    let _ = writeln!(out, "message {} {{", describe.name);

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| {
        if a.name == "Id" {
            std::cmp::Ordering::Less
        } else if b.name == "Id" {
            std::cmp::Ordering::Greater
        } else {
            a.name.cmp(&b.name)
        }
    });

    let mut field_number = 1;
    for field in fields {
        let proto_type = map_type(&field.type_);

        let optional_modifier = if field.nillable { "optional " } else { "" };

        let _ = writeln!(
            out,
            "  {}{} {} = {};",
            optional_modifier, proto_type, field.name, field_number
        );
        field_number += 1;
    }

    let _ = writeln!(out, "}}");
}

/// Maps a Salesforce `FieldType` to a Protocol Buffers scalar type.
#[cfg(feature = "schema")]
fn map_type(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Boolean => "bool",
        FieldType::Int => "int32",
        FieldType::Double | FieldType::Currency | FieldType::Percent => "double",
        // Most Salesforce types serialize as strings
        _ => "string",
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::api::rest::describe::FieldDescribe;

    fn mock_field(name: &str, type_: FieldType, nillable: bool) -> FieldDescribe {
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
            createable: true,
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
            label: name.to_string(),
            length: 18,
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
            updateable: true,
            write_requires_master_read: false,
        }
    }

    #[test]
    fn test_protobuf_generator() {
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
                mock_field("Id", FieldType::Id, false),
                mock_field("Name", FieldType::String, false),
                mock_field("NumberOfEmployees", FieldType::Int, true),
                mock_field("AnnualRevenue", FieldType::Currency, true),
                mock_field("IsActive", FieldType::Boolean, true),
            ],
        };

        let proto_code = generate_protobuf_schema(&describe);

        let expected = r#"syntax = "proto3";
package com.salesforce.schema;

message Account {
  string Id = 1;
  optional double AnnualRevenue = 2;
  optional bool IsActive = 3;
  string Name = 4;
  optional int32 NumberOfEmployees = 5;
}
"#;
        assert_eq!(proto_code, expected);
    }
}
