//! Prisma schema generator for Salesforce SObject Describe metadata.
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates a Prisma schema model from an SObject describe result.
#[cfg(feature = "schema")]
pub fn generate_prisma_model(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_prisma_model(&mut out, describe);
    out
}

/// Writes a Prisma schema model from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_prisma_model(out: &mut String, describe: &SObjectDescribe) {
    let _ = writeln!(out, "/// {}", describe.label);
    let _ = writeln!(out, "model {} {{", describe.name);

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in fields {
        let prisma_type = map_type(&field.type_);
        let optional = if field.nillable { "?" } else { "" };

        let mut attributes = String::new();
        if field.name == "Id" {
            attributes.push_str(" @id");
        }
        if field.unique {
            attributes.push_str(" @unique");
        }

        let _ = writeln!(
            out,
            "  {} {}{}{}",
            field.name, prisma_type, optional, attributes
        );
    }

    out.push_str("}\n");
}

/// Maps a Salesforce `FieldType` to a Prisma type.
fn map_type(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Boolean => "Boolean",
        FieldType::Int => "Int",
        FieldType::Double | FieldType::Currency | FieldType::Percent => "Float",
        FieldType::Date | FieldType::Datetime => "DateTime",
        // Most Salesforce types translate cleanly to String in Prisma
        _ => "String",
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::types::describe::FieldDescribe;

    fn mock_field(name: &str, type_: FieldType, nillable: bool, unique: bool) -> FieldDescribe {
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
            unique,
            updateable: true,
            write_requires_master_read: false,
        }
    }

    #[test]
    fn test_prisma_generator() {
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
                mock_field("Id", FieldType::Id, false, false),
                mock_field("Name", FieldType::String, false, false),
                mock_field("NumberOfEmployees", FieldType::Int, true, false),
                mock_field("AnnualRevenue", FieldType::Currency, true, false),
                mock_field("ExternalId__c", FieldType::String, true, true),
                mock_field("CreatedDate", FieldType::Datetime, false, false),
            ],
        };

        let prisma = generate_prisma_model(&describe);

        let expected = "/// Account\nmodel Account {\n  Id String @id\n  AnnualRevenue Float?\n  CreatedDate DateTime\n  ExternalId__c String? @unique\n  Name String\n  NumberOfEmployees Int?\n}\n";
        assert_eq!(prisma, expected);
    }
}
