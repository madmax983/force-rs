use crate::types::describe::{FieldType, SObjectDescribe};
use serde::{Deserialize, Serialize};

/// The masking strategy to apply to a field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaskingStrategy {
    /// Keep the data as is.
    Preserve,
    /// Replace with randomized/fictitious data.
    Scramble,
    /// Replace with null/empty.
    Nullify,
    /// Replace with a one-way hash.
    Hash,
}

/// The masking policy for a specific field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldMaskingPolicy {
    /// The API name of the field.
    pub name: String,
    /// The masking strategy to apply.
    pub strategy: MaskingStrategy,
}

/// A generated data masking policy for an SObject.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataMaskingPolicy {
    /// The name of the SObject.
    pub object_name: String,
    /// The masking policies for its fields.
    pub fields: Vec<FieldMaskingPolicy>,
}

/// Generates a heuristic-based data masking policy for an SObject based on its Describe metadata.
#[cfg(feature = "schema")]
pub fn generate_masking_policy(describe: &SObjectDescribe) -> DataMaskingPolicy {
    let mut fields = Vec::with_capacity(describe.fields.len());

    for field in &describe.fields {
        let strategy = if field.type_ == FieldType::Id || field.type_ == FieldType::Reference {
            MaskingStrategy::Preserve
        } else if field.type_ == FieldType::Email || field.type_ == FieldType::Phone {
            MaskingStrategy::Scramble
        } else {
            let lower_name = field.name.to_lowercase();
            if lower_name.contains("secret")
                || lower_name.contains("password")
                || lower_name.contains("ssn")
                || lower_name.contains("token")
            {
                MaskingStrategy::Hash
            } else if lower_name.contains("name") {
                if field.type_ == FieldType::String {
                    MaskingStrategy::Scramble
                } else {
                    MaskingStrategy::Preserve
                }
            } else if lower_name.contains("address")
                || lower_name.contains("city")
                || lower_name.contains("street")
                || lower_name.contains("zip")
                || lower_name.contains("postal")
                || lower_name.contains("dob")
                || lower_name.contains("birth")
            {
                MaskingStrategy::Scramble
            } else {
                MaskingStrategy::Preserve
            }
        };

        fields.push(FieldMaskingPolicy {
            name: field.name.clone(),
            strategy,
        });
    }

    DataMaskingPolicy {
        object_name: describe.name.clone(),
        fields,
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;
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
            id_lookup: false,
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
    fn test_generate_masking_policy() {
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
            label: "Contact".to_string(),
            label_plural: "Contacts".to_string(),
            layoutable: true,
            mergeable: true,
            mru_enabled: true,
            name: "Contact".to_string(),
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
                mock_field("Email", FieldType::Email),
                mock_field("Phone", FieldType::Phone),
                mock_field("Secret_Key__c", FieldType::String),
                mock_field("Description", FieldType::Textarea),
                mock_field("Age__c", FieldType::Int),
            ],
        };

        let policy = generate_masking_policy(&describe);
        assert_eq!(policy.object_name, "Contact");
        assert_eq!(policy.fields.len(), 6);

        let get_strategy = |name: &str| {
            policy
                .fields
                .iter()
                .find(|f| f.name == name)
                .map(|f| &f.strategy)
                .must()
        };

        assert_eq!(get_strategy("Id"), &MaskingStrategy::Preserve);
        assert_eq!(get_strategy("Email"), &MaskingStrategy::Scramble);
        assert_eq!(get_strategy("Phone"), &MaskingStrategy::Scramble);
        assert_eq!(get_strategy("Secret_Key__c"), &MaskingStrategy::Hash);
        assert_eq!(get_strategy("Description"), &MaskingStrategy::Preserve);
        assert_eq!(get_strategy("Age__c"), &MaskingStrategy::Preserve);
    }
}
