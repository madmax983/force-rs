//! Elasticsearch index mapping generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates an Elasticsearch index mapping from an SObject describe result.
#[cfg(feature = "schema")]
#[must_use]
pub fn generate_elastic_mapping(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 64);
    write_elastic_mapping(&mut out, describe);
    out
}

/// Writes an Elasticsearch index mapping from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_elastic_mapping(out: &mut String, describe: &SObjectDescribe) {
    let _ = writeln!(out, "{{");
    let _ = writeln!(out, "  \"mappings\": {{");
    let _ = writeln!(out, "    \"properties\": {{");

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    let mut first = true;
    for field in fields {
        if first {
            first = false;
        } else {
            let _ = writeln!(out, ",");
        }

        let elastic_type = map_type(&field.type_);
        let _ = write!(
            out,
            "      \"{}\": {{\n        \"type\": \"{}\"\n      }}",
            field.name, elastic_type
        );
    }

    let _ = writeln!(out);
    let _ = writeln!(out, "    }}");
    let _ = writeln!(out, "  }}");
    let _ = write!(out, "}}");
}

/// Maps a Salesforce `FieldType` to an Elasticsearch type.
#[cfg(feature = "schema")]
fn map_type(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Boolean => "boolean",
        FieldType::Int => "integer",
        FieldType::Double | FieldType::Currency | FieldType::Percent => "double",
        FieldType::Date | FieldType::Datetime => "date",
        FieldType::Id | FieldType::Reference | FieldType::Picklist | FieldType::Multipicklist => {
            "keyword"
        }
        _ => "text",
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::types::describe::FieldDescribe;

    fn mock_field(name: &str, type_: FieldType) -> FieldDescribe {
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
            nillable: true,
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
    fn test_elastic_mapping_generator() {
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
                mock_field("Id", FieldType::Id),
                mock_field("Name", FieldType::String),
                mock_field("NumberOfEmployees", FieldType::Int),
                mock_field("AnnualRevenue", FieldType::Currency),
                mock_field("IsActive", FieldType::Boolean),
                mock_field("CreatedDate", FieldType::Datetime),
            ],
        };

        let mapping = generate_elastic_mapping(&describe);

        let expected = r#"{
  "mappings": {
    "properties": {
      "Id": {
        "type": "keyword"
      },
      "AnnualRevenue": {
        "type": "double"
      },
      "CreatedDate": {
        "type": "date"
      },
      "IsActive": {
        "type": "boolean"
      },
      "Name": {
        "type": "text"
      },
      "NumberOfEmployees": {
        "type": "integer"
      }
    }
  }
}"#;
        assert_eq!(mapping, expected);
    }
}
