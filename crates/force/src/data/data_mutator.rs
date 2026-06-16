//! Data Mutator for Chaos Testing.
//!
//! This module provides the `DataMutator` utility to intentionally corrupt or mutate
//! `DynamicSObject` records. This is invaluable for testing robust error handling
//! against partial Bulk API failures or strict validation rules.
//!
//! # The Spark
//! We have `DataFaker` to generate perfect data and `DataValidator` to check data.
//! But what if developers want to test their error handling? How do they ensure their
//! pipeline correctly handles a Bulk API partial failure? `DataMutator` solves this
//! by intentionally injecting schema-violating errors into records!

use crate::types::DynamicSObject;
use crate::types::describe::{FieldType, SObjectDescribe};

/// Strategies for mutating record data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationStrategy {
    /// Removes a randomly selected required field to simulate missing data.
    RemoveRequiredField,
    /// Truncates a string field beyond its maximum allowed length.
    ExceedMaxLength,
    /// Changes a field to an incompatible type (e.g., placing a string in a number field).
    InjectTypeMismatch,
}

/// Utility for intentionally mutating SObject records for testing.
#[derive(Debug)]
pub struct DataMutator<'a> {
    describe: &'a SObjectDescribe,
}

impl<'a> DataMutator<'a> {
    /// Creates a new `DataMutator` for the given SObject schema.
    #[must_use]
    pub fn new(describe: &'a SObjectDescribe) -> Self {
        Self { describe }
    }

    /// Mutates the given record using the specified strategy.
    ///
    /// # Returns
    /// `true` if a mutation was successfully applied, `false` if no suitable
    /// field could be found for the given strategy.
    pub fn mutate_record(&self, record: &mut DynamicSObject, strategy: MutationStrategy) -> bool {
        match strategy {
            MutationStrategy::RemoveRequiredField => self.remove_required_field(record),
            MutationStrategy::ExceedMaxLength => self.exceed_max_length(record),
            MutationStrategy::InjectTypeMismatch => self.inject_type_mismatch(record),
        }
    }

    fn remove_required_field(&self, record: &mut DynamicSObject) -> bool {
        for field in &self.describe.fields {
            if !field.nillable && !field.defaulted_on_create && field.name != "Id" {
                if record.fields.contains_key(&field.name) {
                    record.fields.remove(&field.name);
                    return true;
                }
            }
        }
        false
    }

    #[allow(clippy::cast_sign_loss)]
    fn exceed_max_length(&self, record: &mut DynamicSObject) -> bool {
        for field in &self.describe.fields {
            if field.type_ == FieldType::String || field.type_ == FieldType::Textarea {
                if let Some(val) = record.fields.get(&field.name) {
                    if let Some(s) = val.as_str() {
                        if field.length > 0 {
                            let mut long_str = s.to_string();
                            while long_str.len() <= field.length as usize {
                                long_str.push_str(" AAAAA");
                            }
                            record.set_field(&field.name, long_str);
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    fn inject_type_mismatch(&self, record: &mut DynamicSObject) -> bool {
        for field in &self.describe.fields {
            if field.type_ == FieldType::Int
                || field.type_ == FieldType::Double
                || field.type_ == FieldType::Currency
            {
                if record.fields.contains_key(&field.name) {
                    record.set_field(&field.name, "NOT_A_NUMBER");
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Attributes;
    use crate::types::SalesforceId;
    use crate::types::describe::FieldDescribe;

    fn mock_field(name: &str, type_: FieldType, length: i32, nillable: bool) -> FieldDescribe {
        FieldDescribe {
            aggregatable: true,
            auto_number: false,
            byte_length: length,
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
            updateable: true,
            write_requires_master_read: false,
        }
    }

    fn create_mock_describe() -> SObjectDescribe {
        SObjectDescribe {
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
                mock_field("Name", FieldType::String, 10, false), // Required, short max length
                mock_field("AnnualRevenue", FieldType::Currency, 0, true), // Number
            ],
        }
    }

    #[allow(clippy::unwrap_used)]
    fn create_record() -> DynamicSObject {
        let id = SalesforceId::new("001000000000001AAA").unwrap();
        let attrs = Attributes::new("Account", &id, "v60.0");
        let mut record = DynamicSObject::new(attrs);
        record.set_field("Name", "Acme Corp");
        record.set_field("AnnualRevenue", 500_000);
        record
    }

    #[test]
    fn test_remove_required_field() {
        let describe = create_mock_describe();
        let mut record = create_record();
        let mutator = DataMutator::new(&describe);

        assert!(record.fields.contains_key("Name"));
        let result = mutator.mutate_record(&mut record, MutationStrategy::RemoveRequiredField);

        assert!(result);
        assert!(!record.fields.contains_key("Name"));
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_exceed_max_length() {
        let describe = create_mock_describe();
        let mut record = create_record();
        let mutator = DataMutator::new(&describe);

        let result = mutator.mutate_record(&mut record, MutationStrategy::ExceedMaxLength);

        assert!(result);
        let name = record.get_field_as::<String>("Name").unwrap().unwrap();
        assert!(name.len() > 10);
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_inject_type_mismatch() {
        let describe = create_mock_describe();
        let mut record = create_record();
        let mutator = DataMutator::new(&describe);

        let result = mutator.mutate_record(&mut record, MutationStrategy::InjectTypeMismatch);

        assert!(result);
        // AnnualRevenue was a number, now it should be a string
        let revenue = record.get_field("AnnualRevenue").unwrap();
        assert!(revenue.is_string());
        assert_eq!(revenue.as_str().unwrap(), "NOT_A_NUMBER");
    }
}
