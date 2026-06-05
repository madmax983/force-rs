//! Apex Test Data Factory Generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates an Apex Test Data Factory class from an SObject describe result.
#[cfg(feature = "schema")]
pub fn generate_apex_factory(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_apex_factory(&mut out, describe);
    out
}

/// Writes an Apex Test Data Factory class from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_apex_factory(out: &mut String, describe: &SObjectDescribe) {
    let class_name = format!("{}TestDataFactory", describe.name);

    let _ = writeln!(out, "@isTest");
    let _ = writeln!(out, "public class {} {{", class_name);

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    let _ = writeln!(
        out,
        "    public static {} build{}() {{",
        describe.name, describe.name
    );
    let _ = writeln!(
        out,
        "        {} obj = new {}();",
        describe.name, describe.name
    );

    for field in &fields {
        if field.createable && !field.nillable && !field.defaulted_on_create {
            let mock_val = match field.type_ {
                FieldType::String
                | FieldType::Textarea
                | FieldType::Email
                | FieldType::Phone
                | FieldType::Url
                | FieldType::Picklist
                | FieldType::Combobox => "'Test String'".to_string(),
                FieldType::Boolean => "true".to_string(),
                FieldType::Int | FieldType::Double | FieldType::Currency | FieldType::Percent => {
                    "42".to_string()
                }
                FieldType::Date => "Date.today()".to_string(),
                FieldType::Datetime => "Datetime.now()".to_string(),
                FieldType::Id | FieldType::Reference => "null // TODO: Assign valid Id".to_string(),
                _ => "null".to_string(),
            };
            let _ = writeln!(out, "        obj.{} = {};", field.name, mock_val);
        }
    }
    let _ = writeln!(out, "        return obj;");
    let _ = writeln!(out, "    }}");

    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "    public static {} create{}() {{",
        describe.name, describe.name
    );
    let _ = writeln!(
        out,
        "        {} obj = build{}();",
        describe.name, describe.name
    );
    let _ = writeln!(out, "        insert obj;");
    let _ = writeln!(out, "        return obj;");
    let _ = writeln!(out, "    }}");

    let _ = writeln!(out, "}}");
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::types::describe::{FieldDescribe, FieldType, SObjectDescribe};

    fn mock_field(
        name: &str,
        type_: FieldType,
        createable: bool,
        nillable: bool,
        defaulted: bool,
    ) -> FieldDescribe {
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
            createable,
            custom: false,
            default_value: None,
            default_value_formula: None,
            defaulted_on_create: defaulted,
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
            updateable: createable,
            write_requires_master_read: false,
        }
    }

    #[test]
    fn test_generate_apex_factory() {
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
                mock_field("Id", FieldType::Id, false, false, true),
                mock_field("Name", FieldType::String, true, false, false),
                mock_field("NumberOfEmployees", FieldType::Int, true, false, false),
                mock_field("Description", FieldType::Textarea, true, true, false),
            ],
        };

        let apex = generate_apex_factory(&describe);
        let expected = "@isTest\npublic class AccountTestDataFactory {\n    public static Account buildAccount() {\n        Account obj = new Account();\n        obj.Name = 'Test String';\n        obj.NumberOfEmployees = 42;\n        return obj;\n    }\n\n    public static Account createAccount() {\n        Account obj = buildAccount();\n        insert obj;\n        return obj;\n    }\n}\n";
        assert_eq!(apex, expected);
    }
}
