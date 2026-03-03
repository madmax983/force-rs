//! Integration tests for HTTP middleware using wiremock.

#[cfg(test)]
mod integration_tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::unwrap_used)]
    use crate::auth::{AccessToken, TokenResponse};
    use crate::error::ForceError;
    use crate::http::{
        HttpExecutor, RequestCompletion, RequestRetryClass, RetryEvent, RetryPolicy, TelemetryHooks,
    };

    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU32, Ordering};
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[derive(serde::Deserialize, Debug)]
    struct TestResponse {
        id: String,
        name: String,
    }

    #[derive(serde::Deserialize)]
    struct IdOnlyResponse {
        id: String,
    }

    fn create_test_token() -> AccessToken {
        let response = TokenResponse {
            access_token: "test_token_123".to_string(),
            instance_url: "https://test.salesforce.com".to_string(),
            token_type: "Bearer".to_string(),
            issued_at: "1640000000000".to_string(), // 2021-12-20
            signature: "test_signature".to_string(),
            expires_in: Some(7200), // 2 hours
            refresh_token: None,
        };
        AccessToken::from_response(response)
    }
    #[tokio::test]
    async fn test_successful_request_with_bearer_token() {
        // Arrange
        let mock_server = MockServer::start().await;
        let executor = HttpExecutor::new();
        let token = create_test_token();

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/sobjects"))
            .and(header("Authorization", "Bearer test_token_123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "sobjects": []
            })))
            .mount(&mock_server)
            .await;

        // Act
        let url = format!("{}/services/data/v60.0/sobjects", mock_server.uri());
        let request = reqwest::Client::new().get(&url).build().unwrap();

        let result = executor
            .execute(request, &token, || async {
                panic!("Should not call refresh on success")
            })
            .await;

        // Assert
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.status(), 200);
    }

    #[tokio::test]
    async fn test_401_triggers_token_refresh_and_retry() {
        // Arrange
        let mock_server = MockServer::start().await;
        let executor = HttpExecutor::new();
        let token = create_test_token();
        let refresh_count = Arc::new(AtomicU32::new(0));
        let refresh_count_clone = Arc::clone(&refresh_count);

        // First request returns 401
        Mock::given(method("GET"))
            .and(path("/test"))
            .and(header("Authorization", "Bearer test_token_123"))
            .respond_with(ResponseTemplate::new(401))
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;

        // Second request (after refresh) returns 200
        Mock::given(method("GET"))
            .and(path("/test"))
            .and(header("Authorization", "Bearer new_token_456"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true
            })))
            .mount(&mock_server)
            .await;

        // Act
        let url = format!("{}/test", mock_server.uri());
        let request = reqwest::Client::new().get(&url).build().unwrap();

        let result = executor
            .execute(request, &token, || {
                let count = Arc::clone(&refresh_count_clone);
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    let response = TokenResponse {
                        access_token: "new_token_456".to_string(),
                        instance_url: "https://test.salesforce.com".to_string(),
                        token_type: "Bearer".to_string(),
                        issued_at: "1640000000000".to_string(),
                        signature: "test_signature".to_string(),
                        expires_in: Some(7200),
                        refresh_token: None,
                    };
                    Ok(AccessToken::from_response(response))
                }
            })
            .await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(refresh_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_429_returns_rate_limit_error_with_retry_after() {
        // Arrange
        let mock_server = MockServer::start().await;
        let executor = HttpExecutor::new();
        let token = create_test_token();

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(
                ResponseTemplate::new(429)
                    .append_header("Retry-After", "120")
                    .set_body_string("Rate limit exceeded"),
            )
            .mount(&mock_server)
            .await;

        // Act
        let url = format!("{}/test", mock_server.uri());
        let request = reqwest::Client::new().get(&url).build().unwrap();

        let result = executor
            .execute(request, &token, || async {
                panic!("Should not refresh on 429")
            })
            .await;

        // Assert
        assert!(result.is_err());
        if let Err(ForceError::Http(crate::error::HttpError::RateLimitExceeded {
            retry_after_seconds,
        })) = result
        {
            assert_eq!(retry_after_seconds, 120);
        } else {
            panic!("Expected RateLimitExceeded error, got: {:?}", result);
        }
    }

    #[tokio::test]
    async fn test_429_defaults_to_60s_when_header_missing() {
        // Arrange
        let mock_server = MockServer::start().await;
        let executor = HttpExecutor::new();
        let token = create_test_token();

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(429).set_body_string("Rate limit exceeded"))
            .mount(&mock_server)
            .await;

        // Act
        let url = format!("{}/test", mock_server.uri());
        let request = reqwest::Client::new().get(&url).build().unwrap();

        let result = executor
            .execute(request, &token, || async {
                panic!("Should not refresh on 429")
            })
            .await;

        // Assert
        assert!(result.is_err());
        if let Err(ForceError::Http(crate::error::HttpError::RateLimitExceeded {
            retry_after_seconds,
        })) = result
        {
            assert_eq!(retry_after_seconds, 60);
        } else {
            panic!(
                "Expected RateLimitExceeded error with default 60s, got: {:?}",
                result
            );
        }
    }

    #[tokio::test]
    async fn test_429_defaults_to_60s_when_header_invalid() {
        // Arrange
        let mock_server = MockServer::start().await;
        let executor = HttpExecutor::new();
        let token = create_test_token();

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(
                ResponseTemplate::new(429)
                    .append_header("Retry-After", "soon")
                    .set_body_string("Rate limit exceeded"),
            )
            .mount(&mock_server)
            .await;

        // Act
        let url = format!("{}/test", mock_server.uri());
        let request = reqwest::Client::new().get(&url).build().unwrap();

        let result = executor
            .execute(request, &token, || async {
                panic!("Should not refresh on 429")
            })
            .await;

        // Assert
        assert!(result.is_err());
        if let Err(ForceError::Http(crate::error::HttpError::RateLimitExceeded {
            retry_after_seconds,
        })) = result
        {
            assert_eq!(retry_after_seconds, 60);
        } else {
            panic!(
                "Expected RateLimitExceeded error with default 60s, got: {:?}",
                result
            );
        }
    }

    #[tokio::test]
    async fn test_503_retries_with_exponential_backoff() {
        // Arrange
        let mock_server = MockServer::start().await;
        // Use shorter backoff for testing to speed up execution
        let executor = HttpExecutor::with_config(2, std::time::Duration::from_secs(30))
            .with_base_backoff(std::time::Duration::from_millis(10));
        let token = create_test_token();

        // First two requests return 503
        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(2)
            .mount(&mock_server)
            .await;

        // Third request succeeds
        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "recovered": true
            })))
            .mount(&mock_server)
            .await;

        // Act
        let url = format!("{}/test", mock_server.uri());
        let request = reqwest::Client::new().get(&url).build().unwrap();

        let start = std::time::Instant::now();
        let result = executor
            .execute(request, &token, || async {
                panic!("Should not refresh on 503 retry")
            })
            .await;
        let elapsed = start.elapsed();

        // Assert
        assert!(result.is_ok());
        // Should have waited ~10ms + ~20ms = ~30ms for backoff
        assert!(elapsed.as_millis() >= 25);
    }

    #[tokio::test]
    async fn test_503_does_not_retry_mutation_by_default() {
        let mock_server = MockServer::start().await;
        let executor = HttpExecutor::with_config(3, std::time::Duration::from_secs(30));
        let token = create_test_token();

        Mock::given(method("POST"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(503).set_body_string("temporary outage"))
            .expect(1)
            .mount(&mock_server)
            .await;

        let url = format!("{}/test", mock_server.uri());
        let request = reqwest::Client::new().post(&url).build().unwrap();

        let result = executor
            .execute(request, &token, || async {
                panic!("Should not refresh on 503")
            })
            .await;

        assert!(result.is_err());
        if let Err(ForceError::Http(crate::error::HttpError::StatusError { status_code, .. })) =
            result
        {
            assert_eq!(status_code, 503);
        } else {
            panic!("Expected 503 status error for non-retried mutation");
        }
    }

    #[tokio::test]
    async fn test_503_can_retry_mutation_with_explicit_policy() {
        let mock_server = MockServer::start().await;
        let executor = HttpExecutor::with_retry_policy(
            RetryPolicy::new(3, 2),
            std::time::Duration::from_secs(30),
        );
        let token = create_test_token();

        Mock::given(method("POST"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(2)
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "recovered": true
            })))
            .mount(&mock_server)
            .await;

        let url = format!("{}/test", mock_server.uri());
        let request = reqwest::Client::new().post(&url).build().unwrap();

        let result = executor
            .execute(request, &token, || async {
                panic!("Should not refresh on 503")
            })
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_telemetry_hooks_capture_retry_and_completion() {
        let mock_server = MockServer::start().await;
        let token = create_test_token();
        let retries = Arc::new(AtomicU32::new(0));
        let completions: Arc<Mutex<Vec<RequestCompletion>>> = Arc::new(Mutex::new(Vec::new()));
        let retries_clone = Arc::clone(&retries);
        let completions_clone = Arc::clone(&completions);

        let hooks = TelemetryHooks::new()
            .on_retry(move |event: &RetryEvent| {
                assert_eq!(event.method, "GET");
                assert_eq!(event.path, "/test");
                // Verify request class is correctly identified
                assert_eq!(event.request_class, "read");
                retries_clone.fetch_add(1, Ordering::SeqCst);
            })
            .on_complete(move |completion| {
                if let Ok(mut guard) = completions_clone.lock() {
                    guard.push(completion.clone());
                } else {
                    panic!("completion lock poisoned");
                }
            });

        let executor = HttpExecutor::with_config(2, std::time::Duration::from_secs(30))
            .with_telemetry_hooks(hooks);

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let url = format!("{}/test?secret=redacted", mock_server.uri());
        let request = reqwest::Client::new().get(&url).build().unwrap();

        let result = executor
            .execute(request, &token, || async {
                panic!("Should not refresh on 503")
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(retries.load(Ordering::SeqCst), 1);
        let Ok(completions) = completions.lock() else {
            panic!("completion lock poisoned");
        };
        assert_eq!(completions.len(), 1);
        assert_eq!(completions[0].path, "/test");
        assert_eq!(completions[0].request_class, "read");
        assert_eq!(completions[0].status_code, Some(200));
        assert_eq!(completions[0].retries, 1);
    }

    #[tokio::test]
    async fn test_api_error_parsing_salesforce_format() {
        // Arrange
        let mock_server = MockServer::start().await;
        let executor = HttpExecutor::new();
        let token = create_test_token();

        let error_body = serde_json::json!([
            {
                "errorCode": "INVALID_FIELD",
                "message": "Field 'InvalidField' does not exist",
                "fields": ["InvalidField"]
            }
        ]);

        Mock::given(method("POST"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(400).set_body_json(error_body))
            .mount(&mock_server)
            .await;

        // Act
        let url = format!("{}/test", mock_server.uri());
        let request = reqwest::Client::new().post(&url).build().unwrap();

        let result = executor
            .execute(request, &token, || async {
                panic!("Should not refresh on 400")
            })
            .await;

        // Assert
        assert!(result.is_err());
        if let Err(ForceError::Http(crate::error::HttpError::StatusError {
            status_code,
            message,
        })) = result
        {
            assert_eq!(status_code, 400);
            assert!(message.contains("INVALID_FIELD"));
            assert!(message.contains("Field 'InvalidField' does not exist"));
        } else {
            panic!("Expected StatusError, got: {:?}", result);
        }
    }

    #[tokio::test]
    async fn test_execute_json_deserializes_response() {
        // Arrange
        let mock_server = MockServer::start().await;
        let executor = HttpExecutor::new();
        let token = create_test_token();

        let response_data = serde_json::json!({
            "id": "001xx000003DGbm",
            "name": "Test Account"
        });

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_data))
            .mount(&mock_server)
            .await;

        // Act
        let url = format!("{}/test", mock_server.uri());
        let request = reqwest::Client::new().get(&url).build().unwrap();

        let result: Result<TestResponse, ForceError> = executor
            .execute_json(request, &token, || async { panic!("Should not refresh") })
            .await;

        // Assert
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.id, "001xx000003DGbm");
        assert_eq!(response.name, "Test Account");
    }

    #[tokio::test]
    async fn test_malformed_json_returns_serialization_error() {
        // Arrange
        let mock_server = MockServer::start().await;
        let executor = HttpExecutor::new();
        let token = create_test_token();

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{invalid json"))
            .mount(&mock_server)
            .await;

        // Act
        let url = format!("{}/test", mock_server.uri());
        let request = reqwest::Client::new().get(&url).build().unwrap();

        let result: Result<IdOnlyResponse, ForceError> = executor
            .execute_json(request, &token, || async { panic!("Should not refresh") })
            .await;

        // Assert
        assert!(result.is_err());
        // Should get an HTTP error when parsing JSON fails
        assert!(matches!(result, Err(ForceError::Http(_))));
    }

    #[tokio::test]
    async fn test_double_401_fails_after_refresh() {
        // Arrange
        let mock_server = MockServer::start().await;
        let executor = HttpExecutor::new();
        let token = create_test_token();

        // Both requests return 401 (refresh doesn't help)
        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&mock_server)
            .await;

        // Act
        let url = format!("{}/test", mock_server.uri());
        let request = reqwest::Client::new().get(&url).build().unwrap();

        let result = executor
            .execute(request, &token, || async {
                let response = TokenResponse {
                    access_token: "new_token".to_string(),
                    instance_url: "https://test.salesforce.com".to_string(),
                    token_type: "Bearer".to_string(),
                    issued_at: "1640000000000".to_string(),
                    signature: "test_signature".to_string(),
                    expires_in: Some(7200),
                    refresh_token: None,
                };
                Ok(AccessToken::from_response(response))
            })
            .await;

        // Assert
        assert!(result.is_err());
        if let Err(ForceError::Http(crate::error::HttpError::StatusError {
            status_code,
            message,
        })) = result
        {
            assert_eq!(status_code, 401);
            assert!(message.contains("after token refresh"));
        } else {
            panic!("Expected StatusError with 401, got: {:?}", result);
        }
    }

    #[tokio::test]
    async fn test_network_timeout_retries() {
        // Arrange
        let mock_server = MockServer::start().await;
        // Use short backoff and timeout
        let executor = HttpExecutor::with_config(2, std::time::Duration::from_millis(50))
            .with_base_backoff(std::time::Duration::from_millis(10));
        let token = create_test_token();

        // First request times out (delay > 50ms)
        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(
                ResponseTemplate::new(200).set_delay(std::time::Duration::from_millis(100)),
            )
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;

        // Second request succeeds
        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "recovered": true
            })))
            .mount(&mock_server)
            .await;

        // Act
        let url = format!("{}/test", mock_server.uri());
        let request = reqwest::Client::new().get(&url).build().unwrap();

        let start = std::time::Instant::now();
        let result = executor
            .execute(request, &token, || async {
                panic!("Should not refresh on timeout retry")
            })
            .await;
        let elapsed = start.elapsed();

        // Assert
        assert!(result.is_ok());
        // Should have waited at least 50ms (timeout) + 10ms (backoff)
        assert!(elapsed.as_millis() >= 60);
    }

    #[tokio::test]
    async fn test_idempotent_mutation_retries_on_503() {
        let mock_server = MockServer::start().await;
        // Policy: Read=3, Mutation=0. IdempotentMutation=3 (implied from Read).
        let executor = HttpExecutor::with_retry_policy(
            RetryPolicy::new(3, 0),
            std::time::Duration::from_secs(30),
        );
        let token = create_test_token();

        // Fail 2 times with 503
        Mock::given(method("POST"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(2)
            .mount(&mock_server)
            .await;

        // Succeed on 3rd attempt
        Mock::given(method("POST"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "recovered": true
            })))
            .mount(&mock_server)
            .await;

        let url = format!("{}/test", mock_server.uri());
        let request = reqwest::Client::new().post(&url).build().unwrap();

        // Explicitly use IdempotentMutation
        let result = executor
            .execute_response_with_retry_class(
                request,
                &token,
                || async { panic!("Should not refresh on 503") },
                RequestRetryClass::IdempotentMutation,
            )
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.status(), 200);
    }
}
