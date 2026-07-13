//! Data types for the SOAP Partner API.
//!
//! The Partner WSDL is untyped: every field value crosses the wire as a string.
//! These types therefore model records generically (a type name plus an ordered
//! list of string fields) and expose lightly-typed result structs for the
//! well-known call responses (`SaveResult`, `QueryResult`, and friends).

/// A generic Salesforce record for the SOAP Partner API.
///
/// Because the Partner WSDL is untyped, an [`SObject`] carries an object type
/// name plus an ordered list of field name/value pairs. Response fields that
/// come back as `xsi:nil="true"` are stored with a value of `None`.
///
/// # Examples
///
/// ```
/// use force::api::soap::SObject;
///
/// let account = SObject::new("Account")
///     .with_field("Name", "Acme Corp")
///     .with_field("NumberOfEmployees", "100");
/// assert_eq!(account.get("Name"), Some("Acme Corp"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SObject {
    /// The Salesforce object API name (for example `Account` or `MyObject__c`).
    pub sobject_type: String,
    /// Ordered field name/value pairs. A value of `None` represents an
    /// explicit `xsi:nil` (null) field in a response.
    pub fields: Vec<(String, Option<String>)>,
    /// Field API names to explicitly null out on `update`/`upsert`.
    ///
    /// The Partner API ignores empty field elements, so nulling a field
    /// requires an explicit `fieldsToNull` entry.
    pub fields_to_null: Vec<String>,
}

impl SObject {
    /// Creates a new, empty record of the given object type.
    #[must_use]
    pub fn new(sobject_type: impl Into<String>) -> Self {
        Self {
            sobject_type: sobject_type.into(),
            fields: Vec::new(),
            fields_to_null: Vec::new(),
        }
    }

    /// Adds a field value, consuming and returning `self` for chaining.
    #[must_use]
    pub fn with_field(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.push((name.into(), Some(value.into())));
        self
    }

    /// Marks a field to be nulled out on `update`/`upsert`, returning `self`.
    #[must_use]
    pub fn with_null_field(mut self, name: impl Into<String>) -> Self {
        self.fields_to_null.push(name.into());
        self
    }

    /// Sets a field value in place.
    pub fn set_field(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.fields.push((name.into(), Some(value.into())));
    }

    /// Returns the value of the first field matching `name`, if present and non-null.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(field_name, _)| field_name == name)
            .and_then(|(_, value)| value.as_deref())
    }

    /// Returns the record `Id`, if present.
    #[must_use]
    pub fn id(&self) -> Option<&str> {
        self.get("Id")
    }
}

/// A per-record error returned inside a `SaveResult`, `UpsertResult`, or
/// `DeleteResult`.
///
/// These are **not** transport faults: they are returned in a successful
/// HTTP 200 response for records that individually failed to save.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SoapError {
    /// The Salesforce status code (for example `REQUIRED_FIELD_MISSING`).
    pub status_code: String,
    /// The human-readable error message.
    pub message: String,
    /// The field API names implicated in the error, if any.
    pub fields: Vec<String>,
}

/// The result of a `create` or `update` call for a single record.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SaveResult {
    /// The record `Id`. Present when `success` is `true`.
    pub id: Option<String>,
    /// Whether the save succeeded.
    pub success: bool,
    /// Per-record errors when `success` is `false`.
    pub errors: Vec<SoapError>,
}

/// The result of an `upsert` call for a single record.
///
/// Identical to [`SaveResult`] but additionally reports whether the record was
/// created (`true`) or updated (`false`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UpsertResult {
    /// Whether a new record was created (as opposed to an existing one updated).
    pub created: bool,
    /// The record `Id`. Present when `success` is `true`.
    pub id: Option<String>,
    /// Whether the upsert succeeded.
    pub success: bool,
    /// Per-record errors when `success` is `false`.
    pub errors: Vec<SoapError>,
}

/// The result of a `delete` call for a single record.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeleteResult {
    /// The record `Id` that was targeted.
    pub id: Option<String>,
    /// Whether the delete succeeded.
    pub success: bool,
    /// Per-record errors when `success` is `false`.
    pub errors: Vec<SoapError>,
}

/// The result of a `query`, `queryMore`, or `queryAll` call.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QueryResult {
    /// Whether this is the final page of results.
    pub done: bool,
    /// The locator used to fetch the next page via `query_more`. Absent when
    /// `done` is `true`.
    pub query_locator: Option<String>,
    /// The total number of records matched by the query.
    pub size: i64,
    /// The records on this page.
    pub records: Vec<SObject>,
}

/// The result of a `search` (SOSL) call.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SearchResult {
    /// The records matched by the search, flattened from `searchRecords`.
    pub records: Vec<SObject>,
}

/// Information about the current user and org, returned by `getUserInfo`.
///
/// Fields that Salesforce does not always return are modelled as `Option`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UserInfo {
    /// The 18-character user Id.
    pub user_id: String,
    /// The user's full name.
    pub user_full_name: String,
    /// The user's email address.
    pub user_email: String,
    /// The user's login name.
    pub user_name: String,
    /// The 18-character organization Id.
    pub organization_id: String,
    /// The organization name.
    pub organization_name: String,
    /// The user's profile Id.
    pub profile_id: Option<String>,
    /// The user's role Id, if assigned.
    pub role_id: Option<String>,
    /// The remaining validity of the session, in seconds.
    pub session_seconds_valid: Option<i64>,
    /// The user's default currency ISO code (multi-currency orgs).
    pub user_default_currency_iso_code: Option<String>,
    /// The user's language.
    pub user_language: Option<String>,
    /// The user's locale.
    pub user_locale: Option<String>,
    /// The user's time zone.
    pub user_time_zone: Option<String>,
    /// The user's type (for example `Standard`).
    pub user_type: Option<String>,
    /// The user's currency symbol.
    pub currency_symbol: Option<String>,
    /// The org's default currency ISO code.
    pub org_default_currency_iso_code: Option<String>,
    /// Whether the org disallows HTML attachments.
    pub org_disallow_html_attachments: Option<bool>,
    /// Whether the org has person accounts enabled.
    pub org_has_person_accounts: Option<bool>,
    /// Whether accessibility mode is enabled for the user.
    pub accessibility_mode: Option<bool>,
}

/// A picklist entry within a [`FieldDescribe`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PicklistEntry {
    /// The picklist value's API value.
    pub value: String,
    /// The picklist value's display label.
    pub label: Option<String>,
    /// Whether this value is active.
    pub active: Option<bool>,
    /// Whether this value is the default.
    pub default_value: Option<bool>,
}

/// A field's metadata within a [`DescribeSObjectResult`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FieldDescribe {
    /// The field API name.
    pub name: String,
    /// The field display label.
    pub label: Option<String>,
    /// The field's SOAP type (for example `string`, `double`, `reference`).
    pub field_type: Option<String>,
    /// The field length, for string-like types.
    pub length: Option<i64>,
    /// Whether the field can be null.
    pub nillable: Option<bool>,
    /// Whether the field is custom.
    pub custom: Option<bool>,
    /// The picklist entries, for picklist fields.
    pub picklist_values: Vec<PicklistEntry>,
}

/// The metadata for a single object, returned by `describeSObject`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DescribeSObjectResult {
    /// The object API name.
    pub name: String,
    /// The object display label.
    pub label: Option<String>,
    /// The object plural label.
    pub label_plural: Option<String>,
    /// The object key prefix (the first three characters of its record Ids).
    pub key_prefix: Option<String>,
    /// Whether the object is custom.
    pub custom: bool,
    /// Whether records can be created.
    pub createable: bool,
    /// Whether records can be updated.
    pub updateable: bool,
    /// Whether records can be deleted.
    pub deletable: bool,
    /// Whether records can be queried.
    pub queryable: bool,
    /// The object's fields.
    pub fields: Vec<FieldDescribe>,
}

/// A single object entry within a [`DescribeGlobalResult`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DescribeGlobalSObject {
    /// The object API name.
    pub name: String,
    /// The object display label.
    pub label: Option<String>,
    /// The object key prefix.
    pub key_prefix: Option<String>,
    /// Whether the object is custom.
    pub custom: bool,
    /// Whether records can be created.
    pub createable: bool,
    /// Whether records can be queried.
    pub queryable: bool,
    /// Whether records can be updated.
    pub updateable: bool,
    /// Whether records can be deleted.
    pub deletable: bool,
}

/// The result of a `describeGlobal` call.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DescribeGlobalResult {
    /// The org's character encoding.
    pub encoding: Option<String>,
    /// The maximum batch size for calls.
    pub max_batch_size: Option<i64>,
    /// The objects available in the org.
    pub sobjects: Vec<DescribeGlobalSObject>,
}
