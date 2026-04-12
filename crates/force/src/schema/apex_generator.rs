//! Apex class generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::api::rest::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates a Salesforce Apex class wrapper definition from an SObject describe result.
#[cfg(feature = "schema")]
pub fn generate_apex_class(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_apex_class(&mut out, describe);
    out
}

/// Writes a Salesforce Apex class wrapper definition from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_apex_class(out: &mut String, describe: &SObjectDescribe) {
    out.push_str("/**\n");
    let _ = writeln!(out, " * @description Apex Wrapper for {}", describe.label);
    out.push_str(" */\n");
    let _ = writeln!(out, "public class {}Wrapper {{", describe.name);

    // Sort fields alphabetically, Id first
    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in fields {
        let apex_type = map_type(&field.type_);
        out.push_str("    /**\n");
        let _ = writeln!(out, "     * @description {}", field.label);
        out.push_str("     */\n");
        let _ = writeln!(
            out,
            "    @AuraEnabled\n    public {} {} {{ get; set; }}\n",
            apex_type, field.name
        );
    }

    out.push_str("}\n");
}

/// Maps a Salesforce `FieldType` to an Apex primitive type.
#[cfg(feature = "schema")]
fn map_type(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Boolean => "Boolean",
        FieldType::Int => "Integer",
        FieldType::Double => "Double",
        FieldType::Currency | FieldType::Percent => "Decimal",
        FieldType::Date => "Date",
        FieldType::Datetime => "Datetime",
        FieldType::Time => "Time",
        FieldType::Id => "Id",
        _ => "String",
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::api::rest::describe::FieldDescribe;

    fn mock_field(name: &str, type_: FieldType, label: &str) -> FieldDescribe {
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
            label: label.to_string(),
            length: 18,
            mask: None,
            mask_type: None,
            name: name.to_string(),
            name_field: name == "Name",
            name_pointing: false,
            nillable: false,
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
    fn test_apex_generator() {
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
                mock_field("Id", FieldType::Id, "Id"),
                mock_field("Name", FieldType::String, "Account Name"),
                mock_field("NumberOfEmployees", FieldType::Int, "Employees"),
                mock_field("IsActive", FieldType::Boolean, "Active"),
            ],
        };

        let apex = generate_apex_class(&describe);

        let expected = "/**\n * @description Apex Wrapper for Account\n */\npublic class AccountWrapper {\n    /**\n     * @description Id\n     */\n    @AuraEnabled\n    public Id Id { get; set; }\n\n    /**\n     * @description Active\n     */\n    @AuraEnabled\n    public Boolean IsActive { get; set; }\n\n    /**\n     * @description Account Name\n     */\n    @AuraEnabled\n    public String Name { get; set; }\n\n    /**\n     * @description Employees\n     */\n    @AuraEnabled\n    public Integer NumberOfEmployees { get; set; }\n\n}\n";
        assert_eq!(apex, expected);
    }
}
