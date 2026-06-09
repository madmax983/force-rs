//! Test-only helper utilities for ergonomic assertions without `unwrap`/`expect`.

use core::fmt::Debug;
use std::collections::HashMap;

use async_trait::async_trait;

use crate::auth::{AccessToken, Authenticator, TokenResponse};
use crate::error::Result as ForceResult;
use crate::types::describe::{FieldDescribe, FieldType, SObjectDescribe};

/// Extension trait for unwrapping `Result`/`Option` in tests without `unwrap()`.
pub trait Must<T> {
    /// Extracts the inner value or panics with a default diagnostic message.
    fn must(self) -> T;
}

impl<T, E: Debug> Must<T> for std::result::Result<T, E> {
    fn must(self) -> T {
        match self {
            Ok(value) => value,
            Err(error) => panic!("unexpected Err: {error:?}"),
        }
    }
}

impl<T> Must<T> for Option<T> {
    fn must(self) -> T {
        match self {
            Some(value) => value,
            None => panic!("unexpected None"),
        }
    }
}

/// Extension trait for unwrapping with custom panic messages.
pub trait MustMsg<T> {
    /// Extracts the inner value or panics with `message`.
    fn must_msg(self, message: &str) -> T;
}

impl<T, E: Debug> MustMsg<T> for std::result::Result<T, E> {
    fn must_msg(self, message: &str) -> T {
        match self {
            Ok(value) => value,
            Err(error) => panic!("{message}: {error:?}"),
        }
    }
}

impl<T> MustMsg<T> for Option<T> {
    fn must_msg(self, message: &str) -> T {
        match self {
            Some(value) => value,
            None => panic!("{message}"),
        }
    }
}

/// Mock authenticator for testing.
#[derive(Debug, Clone)]
pub struct MockAuthenticator {
    token: String,
    instance_url: String,
}

impl MockAuthenticator {
    /// Creates a new mock authenticator.
    pub fn new(token: &str, instance_url: &str) -> Self {
        Self {
            token: token.to_string(),
            instance_url: instance_url.to_string(),
        }
    }
}

#[async_trait]
impl Authenticator for MockAuthenticator {
    async fn authenticate(&self) -> ForceResult<AccessToken> {
        Ok(AccessToken::from_response(TokenResponse {
            access_token: self.token.clone(),
            instance_url: self.instance_url.clone(),
            token_type: "Bearer".to_string(),
            issued_at: "1704067200000".to_string(),
            signature: "test_sig".to_string(),
            expires_in: Some(7200),
            refresh_token: None,
        }))
    }

    async fn refresh(&self) -> ForceResult<AccessToken> {
        self.authenticate().await
    }
}

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

    pub fn length(mut self, length: i32) -> Self {
        self.field.length = length;
        self
    }

    pub fn byte_length(mut self, byte_length: i32) -> Self {
        self.field.byte_length = byte_length;
        self
    }

    pub fn nillable(mut self, nillable: bool) -> Self {
        self.field.nillable = nillable;
        self
    }

    pub fn createable(mut self, createable: bool) -> Self {
        self.field.createable = createable;
        self
    }

    pub fn unique(mut self, value: bool) -> Self {
        self.field.unique = value;
        self
    }

    pub fn updateable(mut self, updateable: bool) -> Self {
        self.field.updateable = updateable;
        self
    }

    pub fn permissionable(mut self, permissionable: bool) -> Self {
        self.field.permissionable = permissionable;
        self
    }

    pub fn defaulted_on_create(mut self, defaulted_on_create: bool) -> Self {
        self.field.defaulted_on_create = defaulted_on_create;
        self
    }

    pub fn picklist_values(mut self, values: Vec<crate::types::describe::PicklistValue>) -> Self {
        self.field.picklist_values = Some(values);
        self
    }

    pub fn precision(mut self, precision: i32) -> Self {
        self.field.precision = precision;
        self
    }

    pub fn digits(mut self, digits: i32) -> Self {
        self.field.digits = digits;
        self
    }

    pub fn label(mut self, label: &str) -> Self {
        self.field.label = label.to_string();
        self
    }

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

    pub fn field(mut self, field: FieldDescribe) -> Self {
        self.describe.fields.push(field);
        self
    }

    pub fn feed_enabled(mut self, feed_enabled: bool) -> Self {
        self.describe.feed_enabled = feed_enabled;
        self
    }

    pub fn build(self) -> SObjectDescribe {
        self.describe
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_must_result_ok() {
        let result: Result<i32, &str> = Ok(42);
        assert_eq!(result.must(), 42);
    }

    #[test]
    #[should_panic(expected = "unexpected Err: \"error message\"")]
    fn test_must_result_err() {
        let result: Result<i32, &str> = Err("error message");
        let _ = result.must();
    }

    #[test]
    fn test_must_option_some() {
        let option: Option<i32> = Some(42);
        assert_eq!(option.must(), 42);
    }

    #[test]
    #[should_panic(expected = "unexpected None")]
    fn test_must_option_none() {
        let option: Option<i32> = None;
        let _ = option.must();
    }

    #[test]
    fn test_must_msg_result_ok() {
        let result: Result<i32, &str> = Ok(42);
        assert_eq!(result.must_msg("Custom panic message"), 42);
    }

    #[test]
    #[should_panic(expected = "Custom panic message: \"error message\"")]
    fn test_must_msg_result_err() {
        let result: Result<i32, &str> = Err("error message");
        let _ = result.must_msg("Custom panic message");
    }

    #[test]
    fn test_must_msg_option_some() {
        let option: Option<i32> = Some(42);
        assert_eq!(option.must_msg("Custom panic message"), 42);
    }

    #[test]
    #[should_panic(expected = "Custom panic message")]
    fn test_must_msg_option_none() {
        let option: Option<i32> = None;
        let _ = option.must_msg("Custom panic message");
    }

    #[tokio::test]
    async fn test_mock_authenticator() {
        let auth = MockAuthenticator::new("my_token", "https://mock.salesforce.com");
        let token = auth.authenticate().await.must();
        assert_eq!(token.as_str(), "my_token");
        assert_eq!(token.instance_url(), "https://mock.salesforce.com");

        let refresh_token = auth.refresh().await.must();
        assert_eq!(refresh_token.as_str(), "my_token");
        assert_eq!(refresh_token.instance_url(), "https://mock.salesforce.com");
    }
}
