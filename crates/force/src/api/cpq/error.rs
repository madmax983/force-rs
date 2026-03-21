//! CPQ-specific error types.
//!
//! The Salesforce CPQ ServiceRouter can return errors in a non-standard
//! format. This module provides a dedicated error type that integrates
//! with the crate's [`ForceError`](crate::error::ForceError) hierarchy.

use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;

/// CPQ-specific error response from the ServiceRouter.
///
/// The ServiceRouter may return error payloads with a `message` and optional
/// `errorCode`. Unknown fields are captured in `extra`.
#[derive(Debug, Clone, Deserialize)]
pub struct CpqErrorResponse {
    /// Human-readable error message.
    pub message: String,

    /// Optional Salesforce error code.
    #[serde(rename = "errorCode", default)]
    pub error_code: Option<String>,

    /// Catch-all for additional error fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl fmt::Display for CpqErrorResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(code) = &self.error_code {
            write!(f, "[{code}] {}", self.message)
        } else {
            write!(f, "{}", self.message)
        }
    }
}

impl std::error::Error for CpqErrorResponse {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Must;

    #[test]
    fn test_cpq_error_deserialize_with_code() {
        let json = serde_json::json!({
            "message": "Quote not found",
            "errorCode": "ENTITY_NOT_FOUND"
        });

        let err: CpqErrorResponse = serde_json::from_value(json).must();
        assert_eq!(err.message, "Quote not found");
        assert_eq!(err.error_code.as_deref(), Some("ENTITY_NOT_FOUND"));
    }

    #[test]
    fn test_cpq_error_deserialize_without_code() {
        let json = serde_json::json!({
            "message": "Internal error occurred"
        });

        let err: CpqErrorResponse = serde_json::from_value(json).must();
        assert_eq!(err.message, "Internal error occurred");
        assert!(err.error_code.is_none());
    }

    #[test]
    fn test_cpq_error_captures_extra_fields() {
        let json = serde_json::json!({
            "message": "Validation failed",
            "errorCode": "VALIDATION_ERROR",
            "fields": ["SBQQ__Quantity__c"],
            "statusCode": 400
        });

        let err: CpqErrorResponse = serde_json::from_value(json).must();
        assert!(err.extra.contains_key("fields"));
        assert!(err.extra.contains_key("statusCode"));
    }

    #[test]
    fn test_cpq_error_display_with_code() {
        let err = CpqErrorResponse {
            message: "Not found".to_string(),
            error_code: Some("NOT_FOUND".to_string()),
            extra: HashMap::new(),
        };
        assert_eq!(err.to_string(), "[NOT_FOUND] Not found");
    }

    #[test]
    fn test_cpq_error_display_without_code() {
        let err = CpqErrorResponse {
            message: "Something went wrong".to_string(),
            error_code: None,
            extra: HashMap::new(),
        };
        assert_eq!(err.to_string(), "Something went wrong");
    }
}
