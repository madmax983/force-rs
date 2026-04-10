use crate::api::rest::describe::{FieldType, SObjectDescribe};

fn map_type(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Id | FieldType::Reference => "Id",
        FieldType::String
        | FieldType::Email
        | FieldType::Phone
        | FieldType::Url
        | FieldType::Picklist
        | FieldType::Multipicklist
        | FieldType::Combobox
        | FieldType::Textarea => "String",
        FieldType::Int => "Integer",
        FieldType::Double | FieldType::Currency | FieldType::Percent => "Decimal",
        FieldType::Boolean => "Boolean",
        FieldType::Date => "Date",
        FieldType::Datetime => "Datetime",
        FieldType::Time => "Time",
        FieldType::Base64 => "Blob",
        _ => "Object", // Fallback for unsupported/complex types
    }
}

/// Generates an Apex class wrapper for the given SObject describe metadata.
#[cfg(feature = "schema")]
pub fn generate_apex_class(describe: &SObjectDescribe) -> String {
    let mut out = String::new();
    write_apex_class(&mut out, describe);
    out
}

/// Writes an Apex class wrapper directly to the provided string buffer.
#[cfg(feature = "schema")]
pub fn write_apex_class(out: &mut String, describe: &SObjectDescribe) {
    use std::fmt::Write;

    let _ = writeln!(out, "public class {} {{", describe.name);

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in fields {
        let apex_type = map_type(&field.type_);
        let _ = writeln!(
            out,
            "    public {} {} {{ get; set; }}",
            apex_type, field.name
        );
    }

    let _ = writeln!(out, "}}");
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::api::rest::describe::{FieldDescribe, FieldType, SObjectDescribe};

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
                mock_field("Id", FieldType::Id),
                mock_field("Name", FieldType::String),
                mock_field("NumberOfEmployees", FieldType::Int),
                mock_field("IsActive", FieldType::Boolean),
                mock_field("CreatedDate", FieldType::Datetime),
            ],
        };

        let result = generate_apex_class(&describe);

        let expected = "public class Account {\n    public Id Id { get; set; }\n    public Datetime CreatedDate { get; set; }\n    public Boolean IsActive { get; set; }\n    public String Name { get; set; }\n    public Integer NumberOfEmployees { get; set; }\n}\n";

        assert_eq!(result, expected);
    }
}
