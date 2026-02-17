//! Salesforce SObject types and traits.
//!
//! This module provides types for working with Salesforce SObjects (Standard Objects),
//! including dynamic field access and typed SObject representations.

use crate::error::Result as ForceResult;
use crate::types::{SalesforceId, validator};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Standard attributes present on all SObjects.
///
/// These fields are automatically included by Salesforce in query results
/// and provide metadata about the record.
///
/// # Examples
///
/// ```
/// use force::types::Attributes;
///
/// let attrs = Attributes {
///     type_: "Account".to_string(),
///     url: "/services/data/v60.0/sobjects/Account/001000000000001AAA".to_string(),
/// };
/// assert_eq!(attrs.type_, "Account");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attributes {
    /// The SObject type name (e.g., "Account", "Contact").
    #[serde(rename = "type")]
    pub type_: String,

    /// The resource URL for this record.
    pub url: String,
}

impl Attributes {
    /// Creates new attributes for the given SObject type and ID.
    ///
    /// The URL format follows Salesforce's REST API convention.
    ///
    /// # Panics
    ///
    /// Panics if the SObject type or API version are invalid.
    #[must_use]
    #[allow(clippy::expect_used)]
    pub fn new(type_name: impl Into<String>, id: &SalesforceId, api_version: &str) -> Self {
        Self::try_new(type_name, id, api_version).expect("Invalid Attributes parameters")
    }

    /// Creates new attributes for the given SObject type and ID, validating inputs.
    ///
    /// # Errors
    ///
    /// Returns an error if the SObject type or API version are invalid.
    pub fn try_new(
        type_name: impl Into<String>,
        id: &SalesforceId,
        api_version: &str,
    ) -> ForceResult<Self> {
        let type_ = type_name.into();
        validator::validate_sobject_name(&type_)?;
        validator::validate_api_version(api_version)?;

        let url = format!(
            "/services/data/{}/sobjects/{}/{}",
            api_version,
            type_,
            id.as_str()
        );
        Ok(Self { type_, url })
    }

    /// Returns the `SObject` type name.
    #[must_use]
    pub fn object_type(&self) -> &str {
        &self.type_
    }
}

/// A dynamic SObject that can hold any Salesforce record.
///
/// This type uses a JSON map internally to store fields dynamically,
/// allowing you to work with any SObject type without compile-time knowledge
/// of its structure.
///
/// # Examples
///
/// ```
/// use force::types::{DynamicSObject, SalesforceId, Attributes};
/// use serde_json::json;
///
/// let id = SalesforceId::new("001000000000001AAA").unwrap();
/// let attrs = Attributes::new("Account", &id, "v60.0");
///
/// let mut account = DynamicSObject::new(attrs);
/// account.set_field("Name", "Acme Corp");
/// account.set_field("Industry", "Technology");
///
/// assert_eq!(account.get_field("Name").and_then(|v| v.as_str()), Some("Acme Corp"));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DynamicSObject {
    /// Standard SObject attributes.
    pub attributes: Attributes,

    /// Dynamic fields stored as a JSON map.
    #[serde(flatten)]
    pub fields: Map<String, Value>,
}

impl DynamicSObject {
    /// Creates a new dynamic SObject with the given attributes.
    #[must_use]
    pub fn new(attributes: Attributes) -> Self {
        Self {
            attributes,
            fields: Map::new(),
        }
    }

    /// Creates a new dynamic SObject from a JSON value.
    ///
    /// # Errors
    ///
    /// Returns an error if the value is not a valid SObject (missing attributes).
    pub fn from_value(value: Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(value)
    }

    /// Gets a field value by name.
    #[must_use]
    pub fn get_field(&self, name: &str) -> Option<&Value> {
        self.fields.get(name)
    }

    /// Gets a field value as a specific type.
    ///
    /// # Errors
    ///
    /// Returns an error if the field cannot be deserialized to type T.
    pub fn get_field_as<T: for<'de> Deserialize<'de>>(
        &self,
        name: &str,
    ) -> Result<Option<T>, serde_json::Error> {
        match self.fields.get(name) {
            Some(value) => serde_json::from_value(value.clone()).map(Some),
            None => Ok(None),
        }
    }

    /// Sets a field value.
    pub fn set_field(&mut self, name: impl Into<String>, value: impl Serialize) {
        if let Ok(json_value) = serde_json::to_value(value) {
            self.fields.insert(name.into(), json_value);
        }
    }

    /// Removes a field by name.
    ///
    /// Returns the removed value, if it existed.
    pub fn remove_field(&mut self, name: &str) -> Option<Value> {
        self.fields.remove(name)
    }

    /// Returns true if the SObject has a field with the given name.
    #[must_use]
    pub fn has_field(&self, name: &str) -> bool {
        self.fields.contains_key(name)
    }

    /// Returns the `SObject` type name.
    #[must_use]
    pub fn object_type(&self) -> &str {
        &self.attributes.type_
    }

    /// Returns an iterator over field names.
    pub fn field_names(&self) -> impl Iterator<Item = &String> {
        self.fields.keys()
    }

    /// Returns the number of fields (excluding attributes).
    #[must_use]
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    /// Converts the `SObject` to a JSON value.
    #[must_use]
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

/// Builder for constructing `DynamicSObject` instances.
///
/// # Examples
///
/// ```
/// use force::types::{DynamicSObjectBuilder, SalesforceId};
///
/// let id = SalesforceId::new("001000000000001AAA").unwrap();
/// let account = DynamicSObjectBuilder::new("Account", &id, "v60.0")
///     .field("Name", "Acme Corp")
///     .field("Industry", "Technology")
///     .field("AnnualRevenue", 1000000)
///     .build();
///
/// assert_eq!(account.object_type(), "Account");
/// assert_eq!(account.get_field("Name").and_then(|v| v.as_str()), Some("Acme Corp"));
/// ```
#[derive(Debug)]
pub struct DynamicSObjectBuilder {
    sobject: DynamicSObject,
}

impl DynamicSObjectBuilder {
    /// Creates a new builder for the given `SObject` type and ID.
    #[must_use]
    pub fn new(type_name: impl Into<String>, id: &SalesforceId, api_version: &str) -> Self {
        let attributes = Attributes::new(type_name, id, api_version);
        Self {
            sobject: DynamicSObject::new(attributes),
        }
    }

    /// Adds a field to the `SObject`.
    #[must_use]
    pub fn field(mut self, name: impl Into<String>, value: impl Serialize) -> Self {
        self.sobject.set_field(name, value);
        self
    }

    /// Builds the `DynamicSObject`.
    #[must_use]
    pub fn build(self) -> DynamicSObject {
        self.sobject
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Must;
    use serde_json::json;

    // RED PHASE - Write failing tests first

    #[test]
    fn test_attributes_new() {
        let id = SalesforceId::new("001000000000001AAA").must();
        let attrs = Attributes::new("Account", &id, "v60.0");

        assert_eq!(attrs.type_, "Account");
        assert_eq!(
            attrs.url,
            "/services/data/v60.0/sobjects/Account/001000000000001AAA"
        );
    }

    #[test]
    fn test_attributes_object_type() {
        let id = SalesforceId::new("001000000000001AAA").must();
        let attrs = Attributes::new("Contact", &id, "v60.0");

        assert_eq!(attrs.object_type(), "Contact");
    }

    #[test]
    fn test_attributes_serialize() {
        let id = SalesforceId::new("001000000000001AAA").must();
        let attrs = Attributes::new("Account", &id, "v60.0");

        let json = serde_json::to_string(&attrs).must();
        assert!(json.contains("\"type\":\"Account\""));
        assert!(json.contains("\"url\":"));
    }

    #[test]
    fn test_attributes_deserialize() {
        let json = r#"{
            "type": "Account",
            "url": "/services/data/v60.0/sobjects/Account/001000000000001AAA"
        }"#;

        let attrs: Attributes = serde_json::from_str(json).must();
        assert_eq!(attrs.type_, "Account");
    }

    #[test]
    fn test_dynamic_sobject_new() {
        let id = SalesforceId::new("001000000000001AAA").must();
        let attrs = Attributes::new("Account", &id, "v60.0");
        let sobject = DynamicSObject::new(attrs);

        assert_eq!(sobject.object_type(), "Account");
        assert_eq!(sobject.field_count(), 0);
    }

    #[test]
    fn test_dynamic_sobject_set_and_get_field() {
        let id = SalesforceId::new("001000000000001AAA").must();
        let attrs = Attributes::new("Account", &id, "v60.0");
        let mut sobject = DynamicSObject::new(attrs);

        sobject.set_field("Name", "Acme Corp");
        sobject.set_field("Industry", "Technology");

        assert_eq!(
            sobject.get_field("Name").and_then(|v| v.as_str()),
            Some("Acme Corp")
        );
        assert_eq!(
            sobject.get_field("Industry").and_then(|v| v.as_str()),
            Some("Technology")
        );
    }

    #[test]
    fn test_dynamic_sobject_get_field_as() {
        let id = SalesforceId::new("001000000000001AAA").must();
        let attrs = Attributes::new("Account", &id, "v60.0");
        let mut sobject = DynamicSObject::new(attrs);

        sobject.set_field("AnnualRevenue", 1_000_000);

        let revenue: Option<i64> = sobject.get_field_as("AnnualRevenue").must();
        assert_eq!(revenue, Some(1_000_000));
    }

    #[test]
    fn test_dynamic_sobject_get_field_as_type_mismatch() {
        let id = SalesforceId::new("001000000000001AAA").must();
        let attrs = Attributes::new("Account", &id, "v60.0");
        let mut sobject = DynamicSObject::new(attrs);

        sobject.set_field("Name", "Acme Corp");

        // Try to get string field as integer
        let result: Result<Option<i64>, _> = sobject.get_field_as("Name");
        assert!(result.is_err());
    }

    #[test]
    fn test_dynamic_sobject_has_field() {
        let id = SalesforceId::new("001000000000001AAA").must();
        let attrs = Attributes::new("Account", &id, "v60.0");
        let mut sobject = DynamicSObject::new(attrs);

        sobject.set_field("Name", "Acme Corp");

        assert!(sobject.has_field("Name"));
        assert!(!sobject.has_field("Industry"));
    }

    #[test]
    fn test_dynamic_sobject_remove_field() {
        let id = SalesforceId::new("001000000000001AAA").must();
        let attrs = Attributes::new("Account", &id, "v60.0");
        let mut sobject = DynamicSObject::new(attrs);

        sobject.set_field("Name", "Acme Corp");
        assert!(sobject.has_field("Name"));

        let removed = sobject.remove_field("Name");
        assert!(removed.is_some());
        assert!(!sobject.has_field("Name"));
    }

    #[test]
    fn test_dynamic_sobject_field_names() {
        let id = SalesforceId::new("001000000000001AAA").must();
        let attrs = Attributes::new("Account", &id, "v60.0");
        let mut sobject = DynamicSObject::new(attrs);

        sobject.set_field("Name", "Acme Corp");
        sobject.set_field("Industry", "Technology");

        let names: Vec<&String> = sobject.field_names().collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&&"Name".to_string()));
        assert!(names.contains(&&"Industry".to_string()));
    }

    #[test]
    fn test_dynamic_sobject_field_count() {
        let id = SalesforceId::new("001000000000001AAA").must();
        let attrs = Attributes::new("Account", &id, "v60.0");
        let mut sobject = DynamicSObject::new(attrs);

        assert_eq!(sobject.field_count(), 0);

        sobject.set_field("Name", "Acme Corp");
        assert_eq!(sobject.field_count(), 1);

        sobject.set_field("Industry", "Technology");
        assert_eq!(sobject.field_count(), 2);
    }

    #[test]
    fn test_dynamic_sobject_serialize() {
        let id = SalesforceId::new("001000000000001AAA").must();
        let attrs = Attributes::new("Account", &id, "v60.0");
        let mut sobject = DynamicSObject::new(attrs);
        sobject.set_field("Name", "Acme Corp");

        let json = serde_json::to_string(&sobject).must();
        assert!(json.contains("\"attributes\""));
        assert!(json.contains("\"Name\":\"Acme Corp\""));
    }

    #[test]
    fn test_dynamic_sobject_deserialize() {
        let json = json!({
            "attributes": {
                "type": "Account",
                "url": "/services/data/v60.0/sobjects/Account/001000000000001AAA"
            },
            "Name": "Acme Corp",
            "Industry": "Technology"
        });

        let sobject: DynamicSObject = serde_json::from_value(json).must();
        assert_eq!(sobject.object_type(), "Account");
        assert_eq!(
            sobject.get_field("Name").and_then(|v| v.as_str()),
            Some("Acme Corp")
        );
    }

    #[test]
    fn test_dynamic_sobject_from_value() {
        let json = json!({
            "attributes": {
                "type": "Contact",
                "url": "/services/data/v60.0/sobjects/Contact/003000000000001AAA"
            },
            "FirstName": "John",
            "LastName": "Doe"
        });

        let sobject = DynamicSObject::from_value(json).must();
        assert_eq!(sobject.object_type(), "Contact");
        assert_eq!(sobject.field_count(), 2);
    }

    #[test]
    fn test_dynamic_sobject_to_value() {
        let id = SalesforceId::new("001000000000001AAA").must();
        let attrs = Attributes::new("Account", &id, "v60.0");
        let mut sobject = DynamicSObject::new(attrs);
        sobject.set_field("Name", "Acme Corp");

        let value = sobject.to_value();
        assert!(value.is_object());
        assert!(value.get("attributes").is_some());
        assert!(value.get("Name").is_some());
    }

    #[test]
    fn test_builder_basic() {
        let id = SalesforceId::new("001000000000001AAA").must();
        let account = DynamicSObjectBuilder::new("Account", &id, "v60.0")
            .field("Name", "Acme Corp")
            .build();

        assert_eq!(account.object_type(), "Account");
        assert_eq!(
            account.get_field("Name").and_then(|v| v.as_str()),
            Some("Acme Corp")
        );
    }

    #[test]
    fn test_builder_multiple_fields() {
        let id = SalesforceId::new("001000000000001AAA").must();
        let account = DynamicSObjectBuilder::new("Account", &id, "v60.0")
            .field("Name", "Acme Corp")
            .field("Industry", "Technology")
            .field("AnnualRevenue", 1_000_000)
            .build();

        assert_eq!(account.field_count(), 3);
    }

    #[test]
    fn test_builder_empty() {
        let id = SalesforceId::new("001000000000001AAA").must();
        let account = DynamicSObjectBuilder::new("Account", &id, "v60.0").build();

        assert_eq!(account.field_count(), 0);
        assert_eq!(account.object_type(), "Account");
    }

    #[test]
    fn test_roundtrip_serialization() {
        let id = SalesforceId::new("001000000000001AAA").must();
        let original = DynamicSObjectBuilder::new("Account", &id, "v60.0")
            .field("Name", "Acme Corp")
            .field("Industry", "Technology")
            .build();

        let json = serde_json::to_string(&original).must();
        let deserialized: DynamicSObject = serde_json::from_str(&json).must();

        assert_eq!(original, deserialized);
    }
}
