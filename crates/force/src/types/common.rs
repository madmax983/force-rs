//! Common Salesforce API response types.
//!
//! This module contains response types used across different Salesforce APIs,
//! including CRUD operation responses and error structures.

use crate::types::SalesforceId;
use serde::{Deserialize, Serialize};

/// Response from a create operation.
///
/// When creating a record in Salesforce, the API returns information about
/// whether the operation succeeded, the ID of the created record (if successful),
/// and any errors that occurred.
///
/// # Examples
///
/// ```
/// use force::types::{ApiError, CreateResponse, SalesforceId};
///
/// // Successful create
/// let response = CreateResponse {
///     id: Some(SalesforceId::new("001000000000001AAA").unwrap()),
///     success: true,
///     errors: vec![],
/// };
/// assert!(response.is_success());
///
/// // Failed create
/// let response = CreateResponse {
///     id: None,
///     success: false,
///     errors: vec![
///         ApiError {
///             message: "Required fields missing".to_string(),
///             error_code: "REQUIRED_FIELD_MISSING".to_string(),
///             fields: vec!["Name".to_string()],
///         }
///     ],
/// };
/// assert!(!response.is_success());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateResponse {
    /// The ID of the created record (present only if success is true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<SalesforceId>,

    /// Whether the create operation succeeded.
    pub success: bool,

    /// List of errors (empty if success is true).
    #[serde(default)]
    pub errors: Vec<ApiError>,
}

impl CreateResponse {
    /// Creates a successful response with the given ID.
    #[must_use]
    pub fn success(id: SalesforceId) -> Self {
        Self {
            id: Some(id),
            success: true,
            errors: Vec::new(),
        }
    }

    /// Creates a failed response with the given errors.
    #[must_use]
    pub fn failure(errors: Vec<ApiError>) -> Self {
        Self {
            id: None,
            success: false,
            errors,
        }
    }

    /// Returns true if the operation succeeded.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.success
    }

    /// Returns true if the operation failed.
    #[must_use]
    pub const fn is_failure(&self) -> bool {
        !self.success
    }
}

/// Response from an update operation.
///
/// Update operations return only a success flag and any errors.
/// The ID is not returned since it's already known (it was in the request).
///
/// # Examples
///
/// ```
/// use force::types::UpdateResponse;
///
/// let response = UpdateResponse {
///     success: true,
///     errors: vec![],
/// };
/// assert!(response.is_success());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResponse {
    /// Whether the update operation succeeded.
    pub success: bool,

    /// List of errors (empty if success is true).
    #[serde(default)]
    pub errors: Vec<ApiError>,
}

impl UpdateResponse {
    /// Creates a successful response.
    #[must_use]
    pub const fn success() -> Self {
        Self {
            success: true,
            errors: Vec::new(),
        }
    }

    /// Creates a failed response with the given errors.
    #[must_use]
    pub fn failure(errors: Vec<ApiError>) -> Self {
        Self {
            success: false,
            errors,
        }
    }

    /// Returns true if the operation succeeded.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.success
    }

    /// Returns true if the operation failed.
    #[must_use]
    pub const fn is_failure(&self) -> bool {
        !self.success
    }
}

/// Response from a delete operation.
///
/// Delete operations return only a success flag and any errors.
///
/// # Examples
///
/// ```
/// use force::types::DeleteResponse;
///
/// let response = DeleteResponse {
///     success: true,
///     errors: vec![],
/// };
/// assert!(response.is_success());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResponse {
    /// Whether the delete operation succeeded.
    pub success: bool,

    /// List of errors (empty if success is true).
    #[serde(default)]
    pub errors: Vec<ApiError>,
}

impl DeleteResponse {
    /// Creates a successful response.
    #[must_use]
    pub const fn success() -> Self {
        Self {
            success: true,
            errors: Vec::new(),
        }
    }

    /// Creates a failed response with the given errors.
    #[must_use]
    pub fn failure(errors: Vec<ApiError>) -> Self {
        Self {
            success: false,
            errors,
        }
    }

    /// Returns true if the operation succeeded.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.success
    }

    /// Returns true if the operation failed.
    #[must_use]
    pub const fn is_failure(&self) -> bool {
        !self.success
    }
}

/// Response from an upsert operation.
///
/// Upsert operations can either create a new record or update an existing one.
/// The response indicates which action was taken.
///
/// # Examples
///
/// ```
/// use force::types::{UpsertResponse, SalesforceId};
///
/// // Created a new record
/// let response = UpsertResponse {
///     id: SalesforceId::new("001000000000001AAA").unwrap(),
///     success: true,
///     created: true,
///     errors: vec![],
/// };
/// assert!(response.is_created());
///
/// // Updated existing record
/// let response = UpsertResponse {
///     id: SalesforceId::new("001000000000001AAA").unwrap(),
///     success: true,
///     created: false,
///     errors: vec![],
/// };
/// assert!(response.is_updated());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertResponse {
    /// The ID of the record (created or updated).
    pub id: SalesforceId,

    /// Whether the upsert operation succeeded.
    pub success: bool,

    /// True if a new record was created, false if existing was updated.
    pub created: bool,

    /// List of errors (empty if success is true).
    #[serde(default)]
    pub errors: Vec<ApiError>,
}

impl UpsertResponse {
    /// Creates a successful response for a newly created record.
    #[must_use]
    pub const fn created(id: SalesforceId) -> Self {
        Self {
            id,
            success: true,
            created: true,
            errors: Vec::new(),
        }
    }

    /// Creates a successful response for an updated record.
    #[must_use]
    pub const fn updated(id: SalesforceId) -> Self {
        Self {
            id,
            success: true,
            created: false,
            errors: Vec::new(),
        }
    }

    /// Creates a failed response with the given ID and errors.
    #[must_use]
    pub fn failure(id: SalesforceId, errors: Vec<ApiError>) -> Self {
        Self {
            id,
            success: false,
            created: false,
            errors,
        }
    }

    /// Returns true if the operation succeeded.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.success
    }

    /// Returns true if the operation failed.
    #[must_use]
    pub const fn is_failure(&self) -> bool {
        !self.success
    }

    /// Returns true if a new record was created.
    #[must_use]
    pub const fn is_created(&self) -> bool {
        self.created
    }

    /// Returns true if an existing record was updated.
    #[must_use]
    pub const fn is_updated(&self) -> bool {
        !self.created
    }
}

/// Error information from a failed Salesforce API operation.
///
/// When an operation fails, Salesforce returns detailed error information
/// including a message, error code, and the fields that caused the error.
///
/// # Examples
///
/// ```
/// use force::types::ApiError;
///
/// let error = ApiError {
///     message: "Required fields are missing: [Name]".to_string(),
///     error_code: "REQUIRED_FIELD_MISSING".to_string(),
///     fields: vec!["Name".to_string()],
/// };
/// assert_eq!(error.error_code, "REQUIRED_FIELD_MISSING");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    /// Human-readable error message.
    pub message: String,

    /// Salesforce error code (e.g., "REQUIRED_FIELD_MISSING").
    #[serde(rename = "statusCode")]
    pub error_code: String,

    /// Fields that caused the error (if applicable).
    #[serde(default)]
    pub fields: Vec<String>,
}

impl ApiError {
    /// Creates a new API error.
    #[must_use]
    pub fn new(message: impl Into<String>, error_code: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error_code: error_code.into(),
            fields: Vec::new(),
        }
    }

    /// Creates a new API error with associated fields.
    #[must_use]
    pub fn with_fields(
        message: impl Into<String>,
        error_code: impl Into<String>,
        fields: Vec<String>,
    ) -> Self {
        Self {
            message: message.into(),
            error_code: error_code.into(),
            fields,
        }
    }
}
#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::unwrap_used)]
    use super::*;

    // RED PHASE - Write failing tests first

    #[test]
    fn test_create_response_success() {
        let id = SalesforceId::new("001000000000001AAA").unwrap();
        let response = CreateResponse::success(id.clone());

        assert!(response.is_success());
        assert!(!response.is_failure());
        assert_eq!(response.id, Some(id));
        assert!(response.errors.is_empty());
    }

    #[test]
    fn test_create_response_failure() {
        let errors = vec![ApiError::new("Test error", "TEST_ERROR")];
        let response = CreateResponse::failure(errors.clone());

        assert!(!response.is_success());
        assert!(response.is_failure());
        assert_eq!(response.id, None);
        assert_eq!(response.errors, errors);
    }

    #[test]
    fn test_create_response_serialize() {
        let id = SalesforceId::new("001000000000001AAA").unwrap();
        let response = CreateResponse::success(id);

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("001000000000001AAA"));
    }

    #[test]
    fn test_create_response_deserialize() {
        let json = r#"{
            "id": "001000000000001AAA",
            "success": true,
            "errors": []
        }"#;

        let response: CreateResponse = serde_json::from_str(json).unwrap();
        assert!(response.is_success());
        assert!(response.id.is_some());
    }

    #[test]
    fn test_update_response_success() {
        let response = UpdateResponse::success();

        assert!(response.is_success());
        assert!(!response.is_failure());
        assert!(response.errors.is_empty());
    }

    #[test]
    fn test_update_response_failure() {
        let errors = vec![ApiError::new("Update failed", "UPDATE_ERROR")];
        let response = UpdateResponse::failure(errors.clone());

        assert!(!response.is_success());
        assert!(response.is_failure());
        assert_eq!(response.errors, errors);
    }

    #[test]
    fn test_delete_response_success() {
        let response = DeleteResponse::success();

        assert!(response.is_success());
        assert!(!response.is_failure());
        assert!(response.errors.is_empty());
    }

    #[test]
    fn test_delete_response_failure() {
        let errors = vec![ApiError::new("Delete failed", "DELETE_ERROR")];
        let response = DeleteResponse::failure(errors.clone());

        assert!(!response.is_success());
        assert!(response.is_failure());
        assert_eq!(response.errors, errors);
    }

    #[test]
    fn test_upsert_response_created() {
        let id = SalesforceId::new("001000000000001AAA").unwrap();
        let response = UpsertResponse::created(id.clone());

        assert!(response.is_success());
        assert!(response.is_created());
        assert!(!response.is_updated());
        assert_eq!(response.id, id);
        assert!(response.errors.is_empty());
    }

    #[test]
    fn test_upsert_response_updated() {
        let id = SalesforceId::new("001000000000001AAA").unwrap();
        let response = UpsertResponse::updated(id.clone());

        assert!(response.is_success());
        assert!(!response.is_created());
        assert!(response.is_updated());
        assert_eq!(response.id, id);
        assert!(response.errors.is_empty());
    }

    #[test]
    fn test_upsert_response_failure() {
        let id = SalesforceId::new("001000000000001AAA").unwrap();
        let errors = vec![ApiError::new("Upsert failed", "UPSERT_ERROR")];
        let response = UpsertResponse::failure(id.clone(), errors.clone());

        assert!(!response.is_success());
        assert!(response.is_failure());
        assert_eq!(response.id, id);
        assert_eq!(response.errors, errors);
    }

    #[test]
    fn test_api_error_new() {
        let error = ApiError::new("Test message", "TEST_CODE");

        assert_eq!(error.message, "Test message");
        assert_eq!(error.error_code, "TEST_CODE");
        assert!(error.fields.is_empty());
    }

    #[test]
    fn test_api_error_with_fields() {
        let fields = vec!["Name".to_string(), "Email".to_string()];
        let error =
            ApiError::with_fields("Missing fields", "REQUIRED_FIELD_MISSING", fields.clone());

        assert_eq!(error.message, "Missing fields");
        assert_eq!(error.error_code, "REQUIRED_FIELD_MISSING");
        assert_eq!(error.fields, fields);
    }

    #[test]
    fn test_api_error_serialize() {
        let error = ApiError::with_fields(
            "Required fields missing",
            "REQUIRED_FIELD_MISSING",
            vec!["Name".to_string()],
        );

        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("\"statusCode\":\"REQUIRED_FIELD_MISSING\""));
        assert!(json.contains("\"message\":\"Required fields missing\""));
        assert!(json.contains("\"fields\""));
    }

    #[test]
    fn test_api_error_deserialize() {
        let json = r#"{
            "message": "Required fields are missing: [Name]",
            "statusCode": "REQUIRED_FIELD_MISSING",
            "fields": ["Name"]
        }"#;

        let error: ApiError = serde_json::from_str(json).unwrap();
        assert_eq!(error.error_code, "REQUIRED_FIELD_MISSING");
        assert_eq!(error.fields, vec!["Name"]);
    }

    #[test]
    fn test_response_roundtrip_serialization() {
        let id = SalesforceId::new("001000000000001AAA").unwrap();
        let original = CreateResponse::success(id);

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: CreateResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_empty_errors_serialization() {
        let response = UpdateResponse::success();
        let json = serde_json::to_string(&response).unwrap();

        // Empty errors array should still be serialized
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("errors").is_some());
    }
}
