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
            let code = first_error.error_code.as_deref().unwrap_or("UNKNOWN");

            // ⚡ Bolt: Pre-allocate a single buffer to avoid multiple heap allocations
            // from intermediate strings and `.join(", ")`.
            let mut cap = code.len() + first_error.message.len() + 4; // "[{}] "
            if !first_error.fields.is_empty() {
                cap += 11 + first_error.fields.iter().map(|f| f.len()).sum::<usize>(); // " (fields: )" + field lengths
                if first_error.fields.len() > 1 {
                    cap += (first_error.fields.len() - 1) * 2; // ", " separators
                }
            }

            let mut message = String::with_capacity(cap);
            message.push('[');
            message.push_str(code);
            message.push_str("] ");
            message.push_str(&first_error.message);

            if !first_error.fields.is_empty() {
                message.push_str(" (fields: ");
                let mut first = true;
                for field in &first_error.fields {
                    if !first {
                        message.push_str(", ");
                    }
                    first = false;
                    message.push_str(field);
                }
                message.push(')');
            }

            return HttpError::StatusError {
                status_code,
                message,
            };
        }
    }

    // Fallback to generic status error
    HttpError::StatusError {
        status_code,
        message: body.to_string(),
    }
}

/// Reads the body of an HTTP response as bytes up to a specified limit.
/// This prevents memory exhaustion (DoS) attacks from maliciously large error responses.
///
/// It strictly caps the internal allocation and reads chunk by chunk.
pub async fn read_capped_body_bytes(
    response: Response,
    limit_bytes: usize,
) -> Result<Vec<u8>, crate::error::ForceError> {
    let mut stream = response.bytes_stream();

    // ⚡ Bolt: Pre-allocate a reasonable capacity, up to max limit.
    // If limit is smaller than default, use limit. Default 4096.
    let init_cap = std::cmp::min(limit_bytes, 4096);
    let mut bytes = Vec::with_capacity(init_cap);

    while let Some(chunk) = stream.next().await {
        if let Ok(chunk_bytes) = chunk {
            // Check remaining capacity before extending
            let remaining = limit_bytes.saturating_sub(bytes.len());

            if remaining == 0 {
                return Err(crate::error::ForceError::InvalidInput(
                    "response exceeded size limit".to_string(),
                ));
            }

            if chunk_bytes.len() > remaining {
                return Err(crate::error::ForceError::InvalidInput(
                    "response exceeded size limit".to_string(),
                ));
            }
            bytes.extend_from_slice(&chunk_bytes);
        } else {
            break;
        }
    }

    Ok(bytes)
}

/// Converts an HTTP error response into a `ForceError` using Salesforce-aware parsing.
///
/// Reads the body of an HTTP response up to a specified byte limit.
/// This prevents memory exhaustion (DoS) attacks from maliciously large error responses.
///
/// It strictly caps the internal allocation and reads chunk by chunk.
pub async fn read_capped_body(
    response: Response,
    limit_bytes: usize,
) -> Result<String, crate::error::ForceError> {
    let bytes = read_capped_body_bytes(response, limit_bytes).await?;
    Ok(String::from_utf8(bytes)
        .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()))
}

pub async fn response_to_force_error(
    response: Response,
    fallback_message: &str,
) -> crate::error::ForceError {
    let status_code = response.status().as_u16();

    let body = match read_capped_body(response, 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => fallback_message.to_string(),
    };

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
            assert_eq!(
                message,
                "[INVALID_FIELD] Field does not exist (fields: Name)"
            );
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
            assert_eq!(message, "[UNKNOWN] Field does not exist (fields: Name)");
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

        let crate::error::ForceError::Http(HttpError::StatusError { message, .. }) = error else {
            panic!("Expected StatusError");
        };

        // It should fall back to the fallback message because reading exceeded the limit.
        assert_eq!(message, "fallback");
    }

    #[tokio::test]
    async fn test_read_capped_body_bytes_truncation() {
        let mock_server = MockServer::start().await;

        // Generate a payload larger than the limit
        let large_body = "A".repeat(5000);

        Mock::given(method("GET"))
            .and(path("/bytes"))
            .respond_with(ResponseTemplate::new(200).set_body_string(large_body))
            .mount(&mock_server)
            .await;

        let client = reqwest::Client::new();
        let url = format!("{}/bytes", mock_server.uri());
        let response = client.get(&url).send().await.must();

        // 4096 is our typical default init_cap in read_capped_body_bytes
        let result = read_capped_body_bytes(response, 4096).await;

        let Err(err) = result else {
            panic!("Expected Err");
        };

        assert!(err.to_string().contains("response exceeded size limit"));
    }
}
