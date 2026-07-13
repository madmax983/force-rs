//! Error types for the Marketing Cloud Engagement client.

use serde_json::Value;

/// Convenience result alias used throughout this crate.
pub type Result<T> = std::result::Result<T, MarketingCloudError>;

/// Errors returned by the Marketing Cloud Engagement client.
#[derive(Debug, thiserror::Error)]
pub enum MarketingCloudError {
    /// The underlying HTTP transport failed (connection, timeout, TLS, ...).
    #[error("http request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// A request or response body could not be (de)serialized.
    #[error("json (de)serialization failed: {0}")]
    Json(#[from] serde_json::Error),

    /// Authentication against the `v2/token` endpoint failed.
    #[error("authentication failed: {0}")]
    Auth(String),

    /// The client was misconfigured (missing tenant subdomain, credentials, ...).
    #[error("invalid configuration: {0}")]
    Configuration(String),

    /// The access token could not be turned into a valid `Authorization` header.
    #[error("could not build a valid authorization header from the access token")]
    InvalidAuthHeader,

    /// A REST request URL could not be constructed.
    #[error("invalid request url: {0}")]
    InvalidUrl(String),

    /// Marketing Cloud returned a non-success status with an error envelope.
    ///
    /// Tolerates both the flat `{message, errorcode, documentation}` shape and the
    /// richer `{requestId, errors: [{errorCode, message, ...}]}` shape.
    #[error("marketing cloud api error (http {status}): {message}")]
    Api {
        /// The HTTP status code returned by Marketing Cloud.
        status: u16,
        /// A human-readable description of the failure.
        message: String,
        /// The Marketing Cloud specific error code, if present.
        error_code: Option<String>,
        /// A documentation URL, if the envelope provided one.
        documentation: Option<String>,
        /// The request id echoed back by newer style endpoints, if present.
        request_id: Option<String>,
    },
}

impl MarketingCloudError {
    /// Builds an [`MarketingCloudError::Api`] by tolerantly parsing an error body.
    ///
    /// Accepts both documented envelope shapes and falls back to the raw body text
    /// (truncated) when neither matches.
    #[must_use]
    pub fn from_error_body(status: u16, body: &str) -> Self {
        match serde_json::from_str::<Value>(body) {
            Ok(value) => Self::from_error_value(status, &value, body),
            Err(_) => Self::Api {
                status,
                message: fallback_message(body),
                error_code: None,
                documentation: None,
                request_id: None,
            },
        }
    }

    fn from_error_value(status: u16, value: &Value, raw: &str) -> Self {
        // Rich shape: { requestId, errors: [ { errorCode, message, ... } ] }
        if let Some(errors) = value.get("errors").and_then(Value::as_array) {
            if let Some(first) = errors.first() {
                let message = first
                    .get("message")
                    .and_then(Value::as_str)
                    .or_else(|| value.get("message").and_then(Value::as_str))
                    .map_or_else(|| fallback_message(raw), ToString::to_string);
                return Self::Api {
                    status,
                    message,
                    error_code: string_or_number(first.get("errorCode"))
                        .or_else(|| string_or_number(value.get("errorcode"))),
                    documentation: first
                        .get("documentation")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                    request_id: value
                        .get("requestId")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                };
            }
        }

        // Flat shape: { message, errorcode, documentation }
        if let Some(message) = value.get("message").and_then(Value::as_str) {
            return Self::Api {
                status,
                message: message.to_string(),
                error_code: string_or_number(value.get("errorcode")),
                documentation: value
                    .get("documentation")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map(ToString::to_string),
                request_id: value
                    .get("requestId")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
            };
        }

        Self::Api {
            status,
            message: fallback_message(raw),
            error_code: None,
            documentation: None,
            request_id: None,
        }
    }
}

/// Extracts an error code that may be encoded as either a string or a number.
fn string_or_number(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

/// Produces a bounded fallback message from an otherwise unparseable body.
fn fallback_message(body: &str) -> String {
    const MAX: usize = 512;
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "no error body returned".to_string();
    }
    if trimmed.len() > MAX {
        let mut end = MAX;
        while !trimmed.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &trimmed[..end])
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn parses_flat_error_envelope() {
        let body = r#"{"message":"Bad key","errorcode":118001,"documentation":"https://x"}"#;
        let err = MarketingCloudError::from_error_body(400, body);
        match err {
            MarketingCloudError::Api {
                status,
                message,
                error_code,
                documentation,
                ..
            } => {
                assert_eq!(status, 400);
                assert_eq!(message, "Bad key");
                assert_eq!(error_code.as_deref(), Some("118001"));
                assert_eq!(documentation.as_deref(), Some("https://x"));
            }
            other => panic!("expected Api error, got {other:?}"),
        }
    }

    #[test]
    fn parses_rich_error_envelope() {
        let body = r#"{"requestId":"abc-123","errorcode":30000,"message":"top","errors":[{"responseCode":400,"errorCode":"validation.email.subject_empty","message":"subject empty","details":[]}]}"#;
        let err = MarketingCloudError::from_error_body(400, body);
        match err {
            MarketingCloudError::Api {
                message,
                error_code,
                request_id,
                ..
            } => {
                assert_eq!(message, "subject empty");
                assert_eq!(
                    error_code.as_deref(),
                    Some("validation.email.subject_empty")
                );
                assert_eq!(request_id.as_deref(), Some("abc-123"));
            }
            other => panic!("expected Api error, got {other:?}"),
        }
    }

    #[test]
    fn falls_back_to_raw_body_for_non_json() {
        let err = MarketingCloudError::from_error_body(503, "Service Unavailable");
        match err {
            MarketingCloudError::Api {
                status, message, ..
            } => {
                assert_eq!(status, 503);
                assert_eq!(message, "Service Unavailable");
            }
            other => panic!("expected Api error, got {other:?}"),
        }
    }

    #[test]
    fn empty_body_yields_placeholder_message() {
        let err = MarketingCloudError::from_error_body(500, "   ");
        assert!(err.to_string().contains("no error body returned"));
    }

    #[test]
    fn display_includes_status_and_message() {
        let err = MarketingCloudError::from_error_body(404, r#"{"message":"nope"}"#);
        assert_eq!(
            err.to_string(),
            "marketing cloud api error (http 404): nope"
        );
    }

    #[test]
    fn configuration_error_display() {
        let err = MarketingCloudError::Configuration("missing tenant_subdomain".to_string());
        assert_eq!(
            err.to_string(),
            "invalid configuration: missing tenant_subdomain"
        );
    }

    #[test]
    fn auth_error_display() {
        let err = MarketingCloudError::Auth("bad credentials".to_string());
        assert_eq!(err.to_string(), "authentication failed: bad credentials");
    }
}
