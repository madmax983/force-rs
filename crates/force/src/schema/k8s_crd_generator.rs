//! Kubernetes Custom Resource Definition (CRD) generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates a Kubernetes Custom Resource Definition (CRD) from an SObject describe result.
#[cfg(feature = "schema")]
pub fn generate_k8s_crd(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_k8s_crd(&mut out, describe);
    out
}

/// Writes a Kubernetes CRD from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_k8s_crd(out: &mut String, describe: &SObjectDescribe) {
    let plural = describe.label_plural.to_lowercase().replace(' ', "");
    let singular = describe.label.to_lowercase().replace(' ', "");

    out.push_str("apiVersion: apiextensions.k8s.io/v1\n");
    out.push_str("kind: CustomResourceDefinition\n");
    out.push_str("metadata:\n");
    let _ = writeln!(out, "  name: {}.salesforce.com", plural);
    out.push_str("spec:\n");
    out.push_str("  group: salesforce.com\n");
    out.push_str("  versions:\n");
    out.push_str("    - name: v1\n");
    out.push_str("      served: true\n");
    out.push_str("      storage: true\n");
    out.push_str("      schema:\n");
    out.push_str("        openAPIV3Schema:\n");
    out.push_str("          type: object\n");
    out.push_str("          properties:\n");
    out.push_str("            spec:\n");
    out.push_str("              type: object\n");
    out.push_str("              properties:\n");

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in fields {
        let _ = writeln!(out, "                {}:", field.name);
        let _ = writeln!(out, "                  type: {}", map_type(&field.type_));
        let _ = writeln!(out, "                  description: \"{}\"", field.label);
    }

    out.push_str("  scope: Namespaced\n");
    out.push_str("  names:\n");
    let _ = writeln!(out, "    plural: {}", plural);
    let _ = writeln!(out, "    singular: {}", singular);
    let _ = writeln!(out, "    kind: {}", describe.name);
}

/// Maps a Salesforce `FieldType` to an OpenAPI v3 type used in K8s CRDs.
#[cfg(feature = "schema")]
fn map_type(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Boolean => "boolean",
        FieldType::Int => "integer",
        FieldType::Double | FieldType::Currency | FieldType::Percent => "number",
        _ => "string",
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
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
    fn test_k8s_crd_generator() {
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
                mock_field("IsActive", FieldType::Boolean, true, true),
            ],
        };

        let crd = generate_k8s_crd(&describe);
        assert!(crd.contains("name: accounts.salesforce.com"));
        assert!(crd.contains("kind: Account"));
        assert!(crd.contains("type: boolean"));
    }
}
