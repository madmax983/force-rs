//! HTTP error parsing and conversion.

use crate::error::HttpError;
use futures::StreamExt;
use reqwest::Response;

/// Parses Salesforce API error from response body.
///
/// Salesforce error responses typically have this format:
/// ```json
/// [
///   {
///     "errorCode": "INVALID_FIELD",
///     "message": "Field does not exist",
///     "fields": ["InvalidField"]
///   }
/// ]
/// ```
#[derive(serde::Deserialize)]
struct SalesforceError {
    #[serde(rename = "errorCode")]
    error_code: Option<String>,
    message: String,
    #[serde(default)]
    fields: Vec<String>,
}

pub fn parse_api_error(status_code: u16, body: &str) -> HttpError {
    // Try to parse as Salesforce error array
    if let Ok(errors) = serde_json::from_str::<Vec<SalesforceError>>(body) {
        if let Some(first_error) = errors.first() {
            return HttpError::StatusError {
                status_code,
                message: format!(
                    "[{}] {}",
                    first_error.error_code.as_deref().unwrap_or("UNKNOWN"),
                    first_error.message
                ),
            };
        }
    }

    // Fallback to generic status error
    HttpError::StatusError {
        status_code,
        message: body.to_string(),
    }
}

/// Converts an HTTP error response into a `ForceError` using Salesforce-aware parsing.
///
/// If the response body is empty or unreadable, falls back to `fallback_message`.
pub async fn response_to_force_error(
    response: Response,
    fallback_message: &str,
) -> crate::error::ForceError {
    let status_code = response.status().as_u16();

    // Read up to 1MB to prevent memory exhaustion DoS
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        if let Ok(chunk_bytes) = chunk {
            bytes.extend_from_slice(&chunk_bytes);
            if bytes.len() > 1024 * 1024 {
                bytes.truncate(1024 * 1024);
                break;
            }
        } else {
            break;
        }
    }
    let body = String::from_utf8_lossy(&bytes).into_owned();

    let payload = if body.trim().is_empty() {
        fallback_message.to_string()
    } else {
        body
    };
    parse_api_error(status_code, &payload).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_api_error_with_salesforce_format() {
        let body =
            r#"[{"errorCode":"INVALID_FIELD","message":"Field does not exist","fields":["Name"]}]"#;
        let error = parse_api_error(400, body);

        if let HttpError::StatusError {
            status_code,
            message,
        } = error
        {
            assert_eq!(status_code, 400);
            assert_eq!(message, "[INVALID_FIELD] Field does not exist");
        } else {
            panic!("Expected StatusError");
        }
    }

    #[test]
    fn test_parse_api_error_fallback() {
        let body = "Some error text";
        let error = parse_api_error(500, body);

        if let HttpError::StatusError {
            status_code,
            message,
        } = error
        {
            assert_eq!(status_code, 500);
            assert_eq!(message, "Some error text");
        } else {
            panic!("Expected StatusError");
        }
    }

    #[test]
    fn test_parse_api_error_object() {
        // Sometimes Salesforce returns a single object instead of an array
        // Though not standard, we should verify it gracefully falls back
        let body = r#"{"errorCode":"INVALID_FIELD","message":"Field does not exist"}"#;
        let error = parse_api_error(400, body);

        if let HttpError::StatusError {
            status_code,
            message,
        } = error
        {
            assert_eq!(status_code, 400);
            // It should fall back to the raw JSON string
            assert_eq!(message, body);
        } else {
            panic!("Expected StatusError");
        }
    }

    #[test]
    fn test_parse_api_error_malformed() {
        let body = "{malformed_json}";
        let error = parse_api_error(500, body);

        if let HttpError::StatusError {
            status_code,
            message,
        } = error
        {
            assert_eq!(status_code, 500);
            assert_eq!(message, body);
        } else {
            panic!("Expected StatusError");
        }
    }

    #[test]
    fn test_parse_api_error_empty() {
        let body = "";
        let error = parse_api_error(500, body);

        if let HttpError::StatusError {
            status_code,
            message,
        } = error
        {
            assert_eq!(status_code, 500);
            assert_eq!(message, "");
        } else {
            panic!("Expected StatusError");
        }
    }

    #[test]
    fn test_parse_api_error_without_error_code() {
        let body = r#"[{"message":"Field does not exist","fields":["Name"]}]"#;
        let error = parse_api_error(400, body);

        if let HttpError::StatusError {
            status_code,
            message,
        } = error
        {
            assert_eq!(status_code, 400);
            assert_eq!(message, "[UNKNOWN] Field does not exist");
        } else {
            panic!("Expected StatusError");
        }
    }
}

#[cfg(all(test, feature = "mock"))]
mod integration_tests {
    use super::*;
    use crate::test_support::Must;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_response_to_force_error_truncation() {
        let mock_server = MockServer::start().await;

        // Generate a 2MB string.
        let large_body = "A".repeat(2 * 1024 * 1024);

        Mock::given(method("GET"))
            .and(path("/error"))
            .respond_with(ResponseTemplate::new(400).set_body_string(large_body))
            .mount(&mock_server)
            .await;

        let client = reqwest::Client::new();
        let url = format!("{}/error", mock_server.uri());
        let response = client.get(&url).send().await.must();

        let error = response_to_force_error(response, "fallback").await;

        let message_len =
            if let crate::error::ForceError::Http(HttpError::StatusError { message, .. }) = error {
                message.len()
            } else {
                panic!("Expected StatusError");
            };

        // It should be truncated exactly at 1MB (1048576 bytes)
        assert_eq!(message_len, 1024 * 1024);
    }
}
