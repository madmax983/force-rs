//! Zod schema generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::api::rest::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates a TypeScript Zod schema definition from an SObject describe result.
#[cfg(feature = "schema")]
pub fn generate_zod_schema(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);

    out.push_str("import { z } from \"zod\";\n\n");
    out.push_str("/**\n");
    let _ = writeln!(out, " * {}", describe.label);
    out.push_str(" */\n");
    let _ = writeln!(out, "export const {}Schema = z.object({{", describe.name);

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

    for field in fields {
        out.push_str("  /**\n");
        let _ = writeln!(out, "   * {}", field.label);
        if let Some(help) = &field.inline_help_text {
            let _ = writeln!(out, "   * {}", help);
        }
        if !field.updateable {
            out.push_str("   * @readonly\n");
        }
        out.push_str("   */\n");

        let base_zod_type = match field.type_ {
            FieldType::Boolean => "z.boolean()",
            FieldType::Int | FieldType::Double | FieldType::Currency | FieldType::Percent => {
                "z.number()"
            }
            _ => "z.string()",
        };

        let mut zod_expr = String::from(base_zod_type);

        if base_zod_type == "z.string()" && field.length > 0 {
            let _ = write!(zod_expr, ".max({})", field.length);
        }

        if field.nillable {
            zod_expr.push_str(".nullable()");
        }

        let _ = writeln!(out, "  {}: {},", field.name, zod_expr);
    }

    out.push_str("});\n\n");
    let _ = writeln!(
        out,
        "export type {} = z.infer<typeof {}Schema>;",
        describe.name, describe.name
    );

    out
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::api::rest::describe::FieldDescribe;

    fn mock_field(
        name: &str,
        type_: FieldType,
        length: i32,
        nillable: bool,
        updateable: bool,
    ) -> FieldDescribe {
        FieldDescribe {
            aggregatable: true,
            auto_number: false,
            byte_length: length * 3, // Just a rough estimate for byte_length
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
            updateable,
            write_requires_master_read: false,
        }
    }

    #[test]
    fn test_zod_generator() {
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
                mock_field("Id", FieldType::Id, 18, false, false),
                mock_field("Name", FieldType::String, 255, false, true),
                mock_field("NumberOfEmployees", FieldType::Int, 0, true, true),
                mock_field("IsActive", FieldType::Boolean, 0, true, true),
            ],
        };

        let zod_code = generate_zod_schema(&describe);

        let expected = r#"import { z } from "zod";

/**
 * Account
 */
export const AccountSchema = z.object({
  /**
   * Id
   * @readonly
   */
  Id: z.string().max(18),
  /**
   * IsActive
   */
  IsActive: z.boolean().nullable(),
  /**
   * Name
   */
  Name: z.string().max(255),
  /**
   * NumberOfEmployees
   */
  NumberOfEmployees: z.number().nullable(),
});

export type Account = z.infer<typeof AccountSchema>;
"#;
        assert_eq!(zod_code, expected);
    }
}
