use crate::types::describe::{FieldDescribe, FieldType, SObjectDescribe};
use std::collections::HashMap;

/// Builder for creating Mock `FieldDescribe` objects in tests
pub struct MockFieldDescribeBuilder {
    field: FieldDescribe,
}

impl MockFieldDescribeBuilder {
    pub fn new(name: &str, type_: FieldType) -> Self {
        Self {
            field: FieldDescribe {
                aggregatable: true,
                auto_number: false,
                byte_length: 255,
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
                length: 255,
                mask: None,
                mask_type: None,
                name: name.to_string(),
                name_field: name == "Name",
                name_pointing: false,
                nillable: true,
                permissionable: true,
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
                soap_type: match type_ {
                    FieldType::Id => "tns:ID",
                    FieldType::Int => "xsd:int",
                    FieldType::Double | FieldType::Currency | FieldType::Percent => "xsd:double",
                    FieldType::Boolean => "xsd:boolean",
                    _ => "xsd:string",
                }
                .to_string(),
                sortable: true,
                type_,
                unique: false,
                updateable: true,
                write_requires_master_read: false,
            },
        }
    }

    #[must_use]
    pub fn length(mut self, length: i32) -> Self {
        self.field.length = length;
        self
    }

    #[must_use]
    pub fn byte_length(mut self, byte_length: i32) -> Self {
        self.field.byte_length = byte_length;
        self
    }

    #[must_use]
    pub fn nillable(mut self, nillable: bool) -> Self {
        self.field.nillable = nillable;
        self
    }

    #[must_use]
    pub fn createable(mut self, createable: bool) -> Self {
        self.field.createable = createable;
        self
    }

    #[must_use]
    pub fn updateable(mut self, updateable: bool) -> Self {
        self.field.updateable = updateable;
        self
    }

    #[must_use]
    pub fn permissionable(mut self, permissionable: bool) -> Self {
        self.field.permissionable = permissionable;
        self
    }

    #[must_use]
    pub fn defaulted_on_create(mut self, defaulted_on_create: bool) -> Self {
        self.field.defaulted_on_create = defaulted_on_create;
        self
    }

    #[must_use]
    pub fn picklist_values(mut self, values: Vec<crate::types::describe::PicklistValue>) -> Self {
        self.field.picklist_values = Some(values);
        self
    }

    #[must_use]
    pub fn precision(mut self, precision: i32) -> Self {
        self.field.precision = precision;
        self
    }

    #[must_use]
    pub fn scale(mut self, scale: i32) -> Self {
        self.field.scale = scale;
        self
    }

    pub fn digits(mut self, digits: i32) -> Self {
        self.field.digits = digits;
        self
    }

    #[must_use]
    pub fn label(mut self, label: &str) -> Self {
        self.field.label = label.to_string();
        self
    }

    #[must_use]
    pub fn soap_type(mut self, soap_type: &str) -> Self {
        self.field.soap_type = soap_type.to_string();
        self
    }

    pub fn build(self) -> FieldDescribe {
        self.field
    }
}

/// Builder for creating Mock `SObjectDescribe` objects in tests
pub struct MockSObjectDescribeBuilder {
    describe: SObjectDescribe,
}

impl MockSObjectDescribeBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            describe: SObjectDescribe {
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
                label: name.to_string(),
                label_plural: format!("{}s", name),
                layoutable: true,
                mergeable: true,
                mru_enabled: true,
                name: name.to_string(),
                queryable: true,
                replicateable: true,
                retrieveable: true,
                searchable: true,
                triggerable: true,
                undeletable: true,
                updateable: true,
                urls: HashMap::new(),
                child_relationships: vec![],
                record_type_infos: vec![],
                fields: vec![],
            },
        }
    }

    #[must_use]
    pub fn field(mut self, field: FieldDescribe) -> Self {
        self.describe.fields.push(field);
        self
    }

    #[must_use]
    pub fn feed_enabled(mut self, feed_enabled: bool) -> Self {
        self.describe.feed_enabled = feed_enabled;
        self
    }

    pub fn build(self) -> SObjectDescribe {
        self.describe
    }
}
