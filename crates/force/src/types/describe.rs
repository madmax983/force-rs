//! Salesforce Describe API types for object and field metadata.
//!
//! These types represent the schema metadata returned by the Salesforce Describe API,
//! including object definitions, field metadata, picklist values, and relationships.
//! They live in the `types` module so they can be shared across feature-gated API
//! handlers (e.g., REST and Tooling) without cross-feature dependencies.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Global describe result containing all available objects in the organization.
///
/// This is returned by the `describe_global()` method and provides a high-level
/// overview of all SObjects available in the org.
///
/// # Examples
///
/// ```ignore
/// let global = client.rest().describe_global().await?;
/// for sobject in &global.sobjects {
///     println!("{}: {}", sobject.name, sobject.label);
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalDescribe {
    /// Encoding used for the response.
    pub encoding: String,

    /// Maximum batch size for API requests.
    pub max_batch_size: i32,

    /// List of all available SObjects.
    pub sobjects: Vec<GlobalSObjectDescribe>,
}

/// Brief description of an SObject from global describe.
///
/// Contains basic metadata about an SObject without detailed field information.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalSObjectDescribe {
    /// Whether records of this object can be activated.
    pub activateable: bool,

    /// Whether this object can be merged with other records.
    pub mergeable: bool,

    /// Whether this object can be queried.
    pub queryable: bool,

    /// Whether this object supports replication.
    pub replicateable: bool,

    /// Whether this object supports retrieval.
    pub retrieveable: bool,

    /// Whether this object supports search.
    pub searchable: bool,

    /// Whether this object can be triggered.
    pub triggerable: bool,

    /// Whether this object can be undeletable.
    pub undeletable: bool,

    /// API name of the object (e.g., "Account", "Contact").
    pub name: String,

    /// User-friendly label for the object.
    pub label: String,

    /// Plural label for the object.
    pub label_plural: String,

    /// Key prefix for record IDs of this object type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_prefix: Option<String>,

    /// Whether this is a custom object.
    pub custom: bool,

    /// URL paths for accessing this object.
    pub urls: HashMap<String, String>,
}

/// Detailed description of a specific SObject.
///
/// Contains comprehensive metadata including all fields, relationships,
/// child relationships, and record type information.
///
/// # Examples
///
/// ```ignore
/// let describe = client.rest().describe("Account").await?;
/// println!("Object: {} ({})", describe.name, describe.label);
///
/// for field in &describe.fields {
///     println!("  Field: {} - {}", field.name, field.type_);
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SObjectDescribe {
    /// Whether records of this object can be activated.
    pub activateable: bool,

    /// Whether this object can be created.
    pub createable: bool,

    /// Whether this object is a custom object.
    pub custom: bool,

    /// Whether this object is a custom setting.
    pub custom_setting: bool,

    /// Whether this object can be deleted.
    pub deletable: bool,

    /// Whether this object is deprecated and scheduled for removal.
    pub deprecated_and_hidden: bool,

    /// Whether feed tracking is enabled for this object.
    pub feed_enabled: bool,

    /// Whether this object has subtypes.
    pub has_subtypes: bool,

    /// Whether this object is a subtype of another object.
    pub is_subtype: bool,

    /// Key prefix for record IDs of this object type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_prefix: Option<String>,

    /// User-friendly label for the object.
    pub label: String,

    /// Plural label for the object.
    pub label_plural: String,

    /// Whether the object layout is compactable.
    pub layoutable: bool,

    /// Whether this object can be merged with other records.
    pub mergeable: bool,

    /// Whether MRU (Most Recently Used) list is enabled.
    pub mru_enabled: bool,

    /// API name of the object.
    pub name: String,

    /// Whether this object can be queried.
    pub queryable: bool,

    /// Whether this object supports replication.
    pub replicateable: bool,

    /// Whether this object supports retrieval.
    pub retrieveable: bool,

    /// Whether this object supports search.
    pub searchable: bool,

    /// Whether this object can be triggered.
    pub triggerable: bool,

    /// Whether this object can be undeletable.
    pub undeletable: bool,

    /// Whether this object can be updated.
    pub updateable: bool,

    /// List of all fields on this object.
    pub fields: Vec<FieldDescribe>,

    /// Child relationships for this object.
    pub child_relationships: Vec<ChildRelationship>,

    /// Record type information.
    pub record_type_infos: Vec<RecordTypeInfo>,

    /// URL paths for accessing this object.
    pub urls: HashMap<String, String>,
}

/// Detailed description of a field.
///
/// Contains comprehensive metadata about a single field including its type,
/// properties, validation rules, and relationships.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldDescribe {
    /// Whether this field is aggregatable in reports.
    pub aggregatable: bool,

    /// Whether this field supports auto-number generation.
    pub auto_number: bool,

    /// Number of bytes used for this field.
    pub byte_length: i32,

    /// Whether this field is calculated.
    pub calculated: bool,

    /// Formula for calculated fields.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calculated_formula: Option<String>,

    /// Whether this field can be cascaded in delete operations.
    pub cascade_delete: bool,

    /// Whether this field is case-sensitive.
    pub case_sensitive: bool,

    /// Compound field name (for address fields).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compound_field_name: Option<String>,

    /// Controller name for dependent picklists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controller_name: Option<String>,

    /// Whether this field can be created.
    pub createable: bool,

    /// Whether this is a custom field.
    pub custom: bool,

    /// Default value for this field (on create).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<serde_json::Value>,

    /// Default value formula.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value_formula: Option<String>,

    /// Whether this field has a default value on create.
    pub defaulted_on_create: bool,

    /// Whether this field is dependent on another picklist.
    pub dependent_picklist: bool,

    /// Whether this field is deprecated and hidden.
    pub deprecated_and_hidden: bool,

    /// Number of digits allowed (for number fields).
    pub digits: i32,

    /// Whether this field is displayed in the layout.
    pub display_location_in_decimal: bool,

    /// Whether this field is encrypted.
    pub encrypted: bool,

    /// Whether this field is an external ID.
    pub external_id: bool,

    /// Extra type information (for polymorphic fields).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_type_info: Option<String>,

    /// Whether this field can be filtered in queries.
    pub filterable: bool,

    /// Filtering lookup information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filtered_lookup_info: Option<FilteredLookupInfo>,

    /// Formula treat blanks as information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formula_treat_blanks_as: Option<String>,

    /// Whether this field can be grouped in reports.
    pub groupable: bool,

    /// Whether this field supports high scale numbers.
    pub high_scale_number: bool,

    /// Whether this field is an HTML formatted field.
    pub html_formatted: bool,

    /// Whether this field is an ID field.
    pub id_lookup: bool,

    /// Inline help text for this field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_help_text: Option<String>,

    /// User-friendly label for this field.
    pub label: String,

    /// Maximum length for text fields.
    pub length: i32,

    /// Mask for encrypted or formatted fields.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mask: Option<String>,

    /// Mask type for encrypted fields.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mask_type: Option<String>,

    /// API name of this field.
    pub name: String,

    /// Whether this field is a name field.
    pub name_field: bool,

    /// Whether this field is name-pointing (polymorphic).
    pub name_pointing: bool,

    /// Whether this field is nillable (can be null).
    pub nillable: bool,

    /// Whether this field is permissionable.
    pub permissionable: bool,

    /// Picklist values for picklist fields.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picklist_values: Option<Vec<PicklistValue>>,

    /// Whether this field uses a polymorphic foreign key.
    pub polymorphic_foreign_key: bool,

    /// Number of decimal places for number fields.
    pub precision: i32,

    /// Whether this field is query-by-distance enabled.
    pub query_by_distance: bool,

    /// Reference target field (for external objects).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_target_field: Option<String>,

    /// Object types this field can reference.
    pub reference_to: Vec<String>,

    /// Relationship name for lookup/master-detail fields.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship_name: Option<String>,

    /// Relationship order (for multiple relationships).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship_order: Option<i32>,

    /// Whether this field is restricted for delete operations.
    pub restricted_delete: bool,

    /// Whether this field is restricted for picklist values.
    pub restricted_picklist: bool,

    /// Number of decimal places displayed.
    pub scale: i32,

    /// Search prefixes for name fields.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_prefixes_supported: Option<bool>,

    /// SOAP type for this field.
    pub soap_type: String,

    /// Whether this field can be sorted in queries.
    pub sortable: bool,

    /// Data type of this field.
    #[serde(rename = "type")]
    pub type_: FieldType,

    /// Whether this field is unique.
    pub unique: bool,

    /// Whether this field can be updated.
    pub updateable: bool,

    /// Whether this field should be displayed in the UI.
    pub write_requires_master_read: bool,
}

/// Field data type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    /// String field.
    String,
    /// Picklist field.
    Picklist,
    /// Multi-select picklist.
    #[serde(rename = "multipicklist")]
    Multipicklist,
    /// Combobox field.
    Combobox,
    /// Reference (lookup) field.
    Reference,
    /// Base64-encoded data.
    Base64,
    /// Boolean field.
    Boolean,
    /// Currency field.
    Currency,
    /// Text area field.
    Textarea,
    /// Integer field.
    Int,
    /// Double (decimal) field.
    Double,
    /// Percent field.
    Percent,
    /// Phone number field.
    Phone,
    /// ID field.
    Id,
    /// Date field.
    Date,
    /// DateTime field.
    Datetime,
    /// Time field.
    Time,
    /// URL field.
    Url,
    /// Email field.
    Email,
    /// Encrypted string field.
    #[serde(rename = "encryptedstring")]
    Encryptedstring,
    /// Datacategory group reference.
    #[serde(rename = "datacategorygroupreference")]
    Datacategorygroupreference,
    /// Location (geolocation) field.
    Location,
    /// Address field.
    Address,
    /// Any type (for generic fields).
    #[serde(rename = "anyType")]
    AnyType,
}

/// Picklist value metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PicklistValue {
    /// Whether this value is active.
    pub active: bool,

    /// Whether this is the default value.
    pub default_value: bool,

    /// Display label for this value.
    pub label: String,

    /// Base64-encoded valid-for bitmap for dependent picklists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_for: Option<String>,

    /// API value.
    pub value: String,
}

/// Child relationship metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildRelationship {
    /// Whether this relationship is cascaded on delete.
    pub cascade_delete: bool,

    /// Name of the child object.
    #[serde(rename = "childSObject")]
    pub child_sobject: String,

    /// Whether this relationship is deprecated and hidden.
    pub deprecated_and_hidden: bool,

    /// Name of the field on the child object.
    pub field: String,

    /// Relationship name (for SOQL queries).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship_name: Option<String>,

    /// Whether this relationship is restricted for delete.
    pub restricted_delete: bool,
}

/// Record type metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordTypeInfo {
    /// Whether this record type is active.
    pub active: bool,

    /// Whether this is the default record type.
    pub default_record_type_mapping: bool,

    /// Developer name of the record type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub developer_name: Option<String>,

    /// Whether this record type is master.
    pub master: bool,

    /// Display name of the record type.
    pub name: String,

    /// Record type ID.
    pub record_type_id: String,

    /// URLs for this record type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urls: Option<HashMap<String, String>>,
}

/// Filtered lookup information.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilteredLookupInfo {
    /// Name of the controlling field.
    pub controlling_fields: Vec<String>,

    /// Whether the filter is dependent on the controlling field.
    pub dependent: bool,

    /// Whether the filter is optional.
    pub optional_filter: bool,
}

#[cfg(test)]
mod tests {
    use crate::test_support::Must;
    use super::*;
    use serde_json::json;

    #[test]
    fn picklist_value_accepts_salesforce_base64_valid_for_bitmap() {
        let value: PicklistValue = serde_json::from_value(json!({
            "active": true,
            "defaultValue": false,
            "label": "Prospect",
            "validFor": "AAAAAgAA",
            "value": "Prospect"
        }))
        .must();

        assert_eq!(value.valid_for.as_deref(), Some("AAAAAgAA"));
    }
}
