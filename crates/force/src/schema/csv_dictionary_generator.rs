//! CSV Data Dictionary generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
#[cfg(feature = "schema")]
use csv::WriterBuilder;

/// Generates a CSV formatted data dictionary from an SObject describe result.
///
/// The CSV includes columns for Name, Label, Type, Length, Nillable, and Createable.
#[cfg(feature = "schema")]
pub fn generate_csv_dictionary(describe: &SObjectDescribe) -> Result<String, csv::Error> {
    let mut writer = WriterBuilder::new().from_writer(Vec::new());

    // Write header
    writer.write_record(["Name", "Label", "Type", "Length", "Nillable", "Createable"])?;

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in fields {
        let type_str = match field.type_ {
            FieldType::Id => "Id".to_string(),
            FieldType::String => "String".to_string(),
            FieldType::Int => "Int".to_string(),
            FieldType::Double => "Double".to_string(),
            FieldType::Boolean => "Boolean".to_string(),
            FieldType::Reference => "Reference".to_string(),
            FieldType::Date => "Date".to_string(),
            FieldType::Datetime => "Datetime".to_string(),
            FieldType::Picklist => "Picklist".to_string(),
            FieldType::Textarea => "Textarea".to_string(),
            FieldType::Email => "Email".to_string(),
            FieldType::Phone => "Phone".to_string(),
            FieldType::Url => "Url".to_string(),
            FieldType::Currency => "Currency".to_string(),
            FieldType::Percent => "Percent".to_string(),
            _ => "Other".to_string(),
        };

        let length_str = field.length.to_string();
        let nillable_str = if field.nillable { "true" } else { "false" };
        let createable_str = if field.createable { "true" } else { "false" };

        writer.write_record([
            field.name.as_str(),
            field.label.as_str(),
            type_str.as_str(),
            length_str.as_str(),
            nillable_str,
            createable_str,
        ])?;
    }

    let bytes = writer.into_inner().map_err(|e| e.into_error())?;
    Ok(String::from_utf8(bytes).unwrap_or_default())
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;
    use crate::types::describe::FieldDescribe;

    fn mock_field(name: &str, type_: FieldType, nillable: bool, updateable: bool) -> FieldDescribe {
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
            updateable,
            write_requires_master_read: false,
        }
    }

    #[test]
    fn test_csv_dictionary_generator() {
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
                mock_field("Name", FieldType::String, false, true),
                mock_field("NumberOfEmployees", FieldType::Int, true, true),
            ],
        };

        let csv_str = generate_csv_dictionary(&describe).must();

        assert!(csv_str.contains("Name,Label,Type,Length,Nillable,Createable"));
        assert!(csv_str.contains("Id,Id,Id,18,false,true"));
        assert!(csv_str.contains("Name,Name,String,18,false,true"));
        assert!(csv_str.contains("NumberOfEmployees,NumberOfEmployees,Int,18,true,true"));
    }
}
