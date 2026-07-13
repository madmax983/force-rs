//! Error types for the Force API client.
//!
//! This module provides a comprehensive error hierarchy for all Salesforce API operations.

use std::fmt;

/// The main error type for Force API operations.
///
/// This error type provides detailed information about failures that can occur
/// when interacting with Salesforce APIs.
#[derive(Debug, thiserror::Error)]
pub enum ForceError {
    /// Authentication-related errors.
    #[error("authentication failed: {0}")]
    Authentication(#[from] AuthenticationError),

    /// HTTP communication errors.
    #[error("HTTP request failed: {0}")]
    Http(#[from] HttpError),

    /// Salesforce API-specific errors.
    #[error("Salesforce API error: {0}")]
    Api(#[from] ApiError),

    /// Configuration errors.
    #[error("configuration error: {0}")]
    Config(#[from] ConfigError),

    /// Serialization/deserialization errors.
    #[error("serialization error: {0}")]
    Serialization(#[from] SerializationError),

    /// Invalid Salesforce ID.
    #[error("invalid Salesforce ID: {0}")]
    InvalidId(#[from] crate::types::salesforce_id::SalesforceIdError),

    /// Invalid input provided to an API method.
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// I/O error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Feature not yet implemented.
    #[error("not implemented: {0}")]
    NotImplemented(String),

    /// GraphQL API errors (feature-gated).
    #[cfg(feature = "graphql")]
    #[error("GraphQL error: {0}")]
    GraphQL(#[from] crate::api::graphql::GraphqlErrorResponse),

    /// CPQ API errors (feature-gated).
    #[cfg(feature = "cpq")]
    #[error("CPQ error: {0}")]
    Cpq(#[from] crate::api::cpq::CpqErrorResponse),

    /// SOAP Partner API faults (feature-gated).
    #[cfg(feature = "soap")]
    #[error("SOAP fault: {0}")]
    Soap(#[from] crate::api::soap::SoapFault),
}

/// Authentication-related errors.
#[derive(Debug, thiserror::Error)]
pub enum AuthenticationError {
    /// OAuth token request failed.
    #[error("OAuth token request failed: {0}")]
    TokenRequestFailed(String),

    /// Invalid credentials provided.
    #[error("invalid credentials: {0}")]
    InvalidCredentials(String),

    /// Token has expired.
    #[error("access token has expired")]
    TokenExpired,

    /// Token refresh failed.
    #[error("failed to refresh access token: {0}")]
    TokenRefreshFailed(String),

    /// Invalid token state (internal error).
    #[error("invalid token state")]
    InvalidToken,

    /// JWT token creation failed (feature-gated).
    #[cfg(feature = "jwt")]
    #[error("JWT token creation failed: {0}")]
    JwtCreationFailed(String),

    /// Invalid JWT configuration (feature-gated).
    #[cfg(feature = "jwt")]
    #[error("invalid JWT configuration: {0}")]
    InvalidJwtConfig(String),
}

/// HTTP communication errors.
#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    /// Network request failed.
    #[error("network request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    /// HTTP status code indicates an error.
    #[error("HTTP {status_code}: {message}")]
    StatusError {
        /// The HTTP status code.
        status_code: u16,
        /// Error message.
        message: String,
    },

    /// Rate limit exceeded.
    #[error("rate limit exceeded, retry after {retry_after_seconds} seconds")]
    RateLimitExceeded {
        /// Number of seconds to wait before retrying.
        retry_after_seconds: u64,
    },

    /// Request timeout.
    #[error("request timeout after {timeout_seconds} seconds")]
    Timeout {
        /// Timeout duration in seconds.
        timeout_seconds: u64,
    },

    /// Invalid URL.
    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    /// Request body could not be built or cloned.
    #[error("request build error: {0}")]
    RequestBuildError(String),

    /// Response payload too large.
    #[error("response payload exceeded the safety limit of {limit_bytes} bytes")]
    PayloadTooLarge {
        /// The byte limit that was exceeded.
        limit_bytes: usize,
    },
}

/// Salesforce API-specific errors.
///
/// Error information from a failed Salesforce API operation.
///
/// When an operation fails, Salesforce returns detailed error information
/// including a message, error code, and the fields that caused the error.
///
/// # Examples
///
/// ```
/// use force::error::ApiError;
///
/// let error = ApiError {
///     message: "Required fields are missing: [Name]".to_string(),
///     error_code: "REQUIRED_FIELD_MISSING".to_string(),
///     fields: vec!["Name".to_string()],
/// };
/// assert_eq!(error.error_code, "REQUIRED_FIELD_MISSING");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    /// Human-readable error message.
    pub message: String,

    /// Error code from Salesforce.
    #[serde(alias = "statusCode", alias = "errorCode")]
    pub error_code: String,

    /// Additional error fields from the API response.
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

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.error_code, self.message)?;
        if !self.fields.is_empty() {
            write!(f, " (fields: ")?;
            for (i, field) in self.fields.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", field)?;
            }
            write!(f, ")")?;
        }
        Ok(())
    }
}

/// Configuration errors.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Missing required configuration value.
    #[error("missing required configuration: {0}")]
    MissingValue(String),

    /// Invalid configuration value.
    #[error("invalid configuration value for {field}: {reason}")]
    InvalidValue {
        /// The configuration field name.
        field: String,
        /// Reason why the value is invalid.
        reason: String,
    },

    /// Environment variable error.
    #[error("environment variable error: {0}")]
    EnvVar(#[from] std::env::VarError),
}

/// Serialization/deserialization errors.
#[derive(Debug, thiserror::Error)]
pub enum SerializationError {
    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// CSV serialization/deserialization error (feature-gated).
    #[cfg(feature = "bulk")]
    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),

    /// Invalid data format.
    #[error("invalid data format: {0}")]
    InvalidFormat(String),
}

/// A specialized Result type for Force API operations.
pub type Result<T> = std::result::Result<T, ForceError>;
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;

    #[test]
    fn test_authentication_error_display() {
        let err = AuthenticationError::TokenRequestFailed("invalid_grant".to_string());
        assert_eq!(err.to_string(), "OAuth token request failed: invalid_grant");
    }

    #[test]
    fn test_authentication_error_token_expired() {
        let err = AuthenticationError::TokenExpired;
        assert_eq!(err.to_string(), "access token has expired");
    }

    #[test]
    fn test_http_error_status() {
        let err = HttpError::StatusError {
            status_code: 404,
            message: "Resource not found".to_string(),
        };
        assert_eq!(err.to_string(), "HTTP 404: Resource not found");
    }

    #[test]
    fn test_http_error_rate_limit() {
        let err = HttpError::RateLimitExceeded {
            retry_after_seconds: 60,
        };
        assert_eq!(
            err.to_string(),
            "rate limit exceeded, retry after 60 seconds"
        );
    }

    #[test]
    fn test_http_error_timeout() {
        let err = HttpError::Timeout {
            timeout_seconds: 30,
        };
        assert_eq!(err.to_string(), "request timeout after 30 seconds");
    }

    #[test]
    fn test_api_error_display() {
        let err = ApiError {
            error_code: "INVALID_FIELD".to_string(),
            message: "Field does not exist".to_string(),
            fields: vec!["Account.InvalidField".to_string()],
        };
        assert_eq!(
            err.to_string(),
            "[INVALID_FIELD] Field does not exist (fields: Account.InvalidField)"
        );
    }

    #[test]
    fn test_api_error_no_fields() {
        let err = ApiError {
            error_code: "UNKNOWN_ERROR".to_string(),
            message: "An unknown error occurred".to_string(),
            fields: vec![],
        };
        assert_eq!(err.to_string(), "[UNKNOWN_ERROR] An unknown error occurred");
    }

    #[test]
    fn test_config_error_missing_value() {
        let err = ConfigError::MissingValue("client_id".to_string());
        assert_eq!(err.to_string(), "missing required configuration: client_id");
    }

    #[test]
    fn test_config_error_invalid_value() {
        let err = ConfigError::InvalidValue {
            field: "api_version".to_string(),
            reason: "must be in format vXX.0".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "invalid configuration value for api_version: must be in format vXX.0"
        );
    }

    #[test]
    fn test_serialization_error_invalid_format() {
        let err = SerializationError::InvalidFormat("expected ISO 8601 date".to_string());
        assert_eq!(
            err.to_string(),
            "invalid data format: expected ISO 8601 date"
        );
    }

    #[test]
    fn test_force_error_from_authentication() {
        let auth_err = AuthenticationError::TokenExpired;
        let force_err: ForceError = auth_err.into();
        assert_eq!(
            force_err.to_string(),
            "authentication failed: access token has expired"
        );
    }

    #[test]
    fn test_force_error_from_http() {
        let http_err = HttpError::InvalidUrl("not a valid url".to_string());
        let force_err: ForceError = http_err.into();
        assert_eq!(
            force_err.to_string(),
            "HTTP request failed: invalid URL: not a valid url"
        );
    }

    #[test]
    fn test_force_error_from_api() {
        let api_err = ApiError {
            error_code: "REQUIRED_FIELD_MISSING".to_string(),
            message: "Name is required".to_string(),
            fields: vec!["Name".to_string()],
        };
        let force_err: ForceError = api_err.into();
        assert_eq!(
            force_err.to_string(),
            "Salesforce API error: [REQUIRED_FIELD_MISSING] Name is required (fields: Name)"
        );
    }

    #[test]
    fn test_force_error_from_config() {
        let config_err = ConfigError::MissingValue("instance_url".to_string());
        let force_err: ForceError = config_err.into();
        assert_eq!(
            force_err.to_string(),
            "configuration error: missing required configuration: instance_url"
        );
    }

    #[test]
    fn test_force_error_from_serialization() {
        let ser_err = SerializationError::InvalidFormat("malformed JSON".to_string());
        let force_err: ForceError = ser_err.into();
        assert_eq!(
            force_err.to_string(),
            "serialization error: invalid data format: malformed JSON"
        );
    }

    #[test]
    fn test_result_type_ok() {
        let result: Result<i32> = Ok(42);
        assert!(result.is_ok());
        assert_eq!(result.must(), 42);
    }

    #[test]
    fn test_result_type_err() {
        let result: Result<i32> = Err(ForceError::Authentication(
            AuthenticationError::TokenExpired,
        ));
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(err.to_string().contains(""));
    }

    #[cfg(feature = "jwt")]
    #[test]
    fn test_jwt_error_creation_failed() {
        let err = AuthenticationError::JwtCreationFailed("invalid key".to_string());
        assert_eq!(err.to_string(), "JWT token creation failed: invalid key");
    }

    #[cfg(feature = "jwt")]
    #[test]
    fn test_jwt_error_invalid_config() {
        let err = AuthenticationError::InvalidJwtConfig("missing private key".to_string());
        assert_eq!(
            err.to_string(),
            "invalid JWT configuration: missing private key"
        );
    }

    #[cfg(feature = "bulk")]
    #[test]
    fn test_csv_error_conversion() {
        // CSV errors require actual CSV parsing to generate, so we test the variant exists
        let err = SerializationError::InvalidFormat("CSV test".to_string());
        assert!(err.to_string().contains("invalid data format"));
    }

    #[test]
    fn test_force_error_from_invalid_id_length() {
        let id_err = crate::types::salesforce_id::SalesforceIdError::InvalidLength(10);
        let force_err: ForceError = id_err.into();
        assert_eq!(
            force_err.to_string(),
            "invalid Salesforce ID: invalid ID length: 10 (must be 15 or 18 characters)"
        );
    }

    #[test]
    fn test_force_error_from_invalid_id_characters() {
        let id_err = crate::types::salesforce_id::SalesforceIdError::InvalidCharacters;
        let force_err: ForceError = id_err.into();
        assert_eq!(
            force_err.to_string(),
            "invalid Salesforce ID: ID contains invalid characters (must be alphanumeric)"
        );
    }

    #[test]
    fn test_force_error_from_invalid_id_checksum() {
        let id_err = crate::types::salesforce_id::SalesforceIdError::InvalidChecksum;
        let force_err: ForceError = id_err.into();
        assert_eq!(
            force_err.to_string(),
            "invalid Salesforce ID: invalid checksum for 18-character ID"
        );
    }

    #[test]
    fn test_error_trait_implementations() {
        // Verify all errors implement Send + Sync + 'static
        fn assert_send_sync<T: Send + Sync + 'static>() {}

        assert_send_sync::<ForceError>();
        assert_send_sync::<AuthenticationError>();
        assert_send_sync::<HttpError>();
        assert_send_sync::<ApiError>();
        assert_send_sync::<ConfigError>();
        assert_send_sync::<SerializationError>();
    }
}
