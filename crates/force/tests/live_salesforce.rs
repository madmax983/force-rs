#![allow(missing_docs)]
#![cfg(all(feature = "rest", feature = "bulk"))]
//! Live Salesforce smoke tests for REST and Bulk APIs.
//!
//! These tests are ignored by default and require:
//! - `SF_ACCESS_TOKEN`
//! - `SF_INSTANCE_URL`
//! - optional `SF_API_VERSION` (defaults to `v60.0`)

use async_trait::async_trait;
use force::api::bulk::{BulkPollPolicy, IngestJob, JobOperation};
use force::api::RestOperation;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::{ForceClient, builder};
use force::config::ClientConfig;
use force::error::ForceError;
use force::error::HttpError;
use force::error::Result;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone)]
struct EnvAuthenticator {
    access_token: String,
    instance_url: String,
}

#[async_trait]
impl Authenticator for EnvAuthenticator {
    async fn authenticate(&self) -> Result<AccessToken> {
        Ok(AccessToken::from_response(TokenResponse {
            access_token: self.access_token.clone(),
            instance_url: self.instance_url.clone(),
            token_type: "Bearer".to_string(),
            issued_at: chrono::Utc::now().timestamp_millis().to_string(),
            expires_in: Some(7_200),
            refresh_token: None,
            signature: "live-test".to_string(),
        }))
    }

    async fn refresh(&self) -> Result<AccessToken> {
        self.authenticate().await
    }
}

#[derive(Debug, Clone)]
struct LiveConfig {
    access_token: String,
    instance_url: String,
    api_version: String,
    runtime: LiveRuntimeConfig,
}

#[derive(Debug, Clone, Copy)]
struct LiveRuntimeConfig {
    test_timeout: Duration,
    bulk_poll_policy: BulkPollPolicy,
    bulk_query_row_limit: usize,
}

fn env_u32(key: &str, default: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

fn env_flag(key: &str) -> bool {
    std::env::var(key)
        .map(|value| {
            let value = value.to_ascii_lowercase();
            matches!(value.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false)
}

fn assert_status_error_with_code(err: &ForceError, expected_status: u16, expected_codes: &[&str]) {
    match err {
        ForceError::Http(HttpError::StatusError {
            status_code,
            message,
        }) => {
            assert_eq!(*status_code, expected_status);
            assert!(
                expected_codes
                    .iter()
                    .any(|code| message.contains(code) || message.contains(&format!("[{code}]"))),
                "expected one of {expected_codes:?}, got message: {message}",
            );
        }
        _ => panic!("expected Http::StatusError, got: {err:?}"),
    }
}

fn load_runtime_config() -> LiveRuntimeConfig {
    let timeout_secs = env_u64("SF_LIVE_TEST_TIMEOUT_SECS", 120);
    let poll_attempts = env_u32("SF_LIVE_BULK_POLL_MAX_ATTEMPTS", 10);
    let poll_initial_ms = env_u64("SF_LIVE_BULK_POLL_INITIAL_BACKOFF_MS", 1_000);
    let poll_max_ms = env_u64("SF_LIVE_BULK_POLL_MAX_BACKOFF_MS", 30_000);
    let bulk_query_row_limit =
        usize::try_from(env_u64("SF_LIVE_BULK_QUERY_ROW_LIMIT", 5)).unwrap_or(5);

    LiveRuntimeConfig {
        test_timeout: Duration::from_secs(timeout_secs),
        bulk_poll_policy: BulkPollPolicy::new(
            poll_attempts,
            Duration::from_millis(poll_initial_ms),
            Duration::from_millis(poll_max_ms),
        ),
        bulk_query_row_limit,
    }
}

fn load_live_config() -> Option<LiveConfig> {
    let access_token = std::env::var("SF_ACCESS_TOKEN").ok()?;
    let instance_url = std::env::var("SF_INSTANCE_URL").ok()?;
    let api_version = std::env::var("SF_API_VERSION").unwrap_or_else(|_| "v60.0".to_string());
    Some(LiveConfig {
        access_token,
        instance_url,
        api_version,
        runtime: load_runtime_config(),
    })
}

async fn create_live_client(config: &LiveConfig) -> Result<ForceClient<EnvAuthenticator>> {
    let auth = EnvAuthenticator {
        access_token: config.access_token.clone(),
        instance_url: config.instance_url.clone(),
    };

    let client_config = ClientConfig {
        api_version: config.api_version.clone(),
        ..Default::default()
    };

    builder()
        .config(client_config)
        .authenticate(auth)
        .build()
        .await
}

#[tokio::test]
#[ignore = "requires a live Salesforce org and SF_ACCESS_TOKEN/SF_INSTANCE_URL env vars"]
async fn live_rest_query_smoke() -> Result<()> {
    let Some(config) = load_live_config() else {
        eprintln!("skipping live_rest_query_smoke: missing SF_ACCESS_TOKEN or SF_INSTANCE_URL");
        return Ok(());
    };

    let result = tokio::time::timeout(config.runtime.test_timeout, async {
        let client = create_live_client(&config).await?;
        client
            .rest()
            .query::<force::types::DynamicSObject>("SELECT Id FROM Account LIMIT 1")
            .await
    })
    .await
    .map_err(|_| HttpError::Timeout {
        timeout_seconds: config.runtime.test_timeout.as_secs(),
    })??;

    assert!(result.total_size <= 1);
    assert!(result.records.len() <= 1);
    Ok(())
}

#[derive(Debug, Deserialize)]
struct LiveAccountRow {
    #[serde(rename = "Id")]
    id: String,
}

#[tokio::test]
#[ignore = "requires a live Salesforce org and SF_ACCESS_TOKEN/SF_INSTANCE_URL env vars"]
async fn live_bulk_query_stream_smoke() -> Result<()> {
    let Some(config) = load_live_config() else {
        eprintln!(
            "skipping live_bulk_query_stream_smoke: missing SF_ACCESS_TOKEN or SF_INSTANCE_URL"
        );
        return Ok(());
    };

    let seen = tokio::time::timeout(config.runtime.test_timeout, async {
        let client = create_live_client(&config).await?;
        let soql = format!(
            "SELECT Id FROM Account LIMIT {}",
            config.runtime.bulk_query_row_limit
        );
        let mut stream = client
            .bulk()
            .bulk_query_with_policy::<LiveAccountRow>(&soql, config.runtime.bulk_poll_policy)
            .await?;

        let mut seen = 0usize;
        while let Some(row) = stream.next().await? {
            assert!(!row.id.is_empty());
            seen += 1;
            if seen >= config.runtime.bulk_query_row_limit {
                break;
            }
        }
        Ok::<usize, force::error::ForceError>(seen)
    })
    .await
    .map_err(|_| HttpError::Timeout {
        timeout_seconds: config.runtime.test_timeout.as_secs(),
    })??;

    assert!(seen <= config.runtime.bulk_query_row_limit);
    Ok(())
}

#[tokio::test]
#[ignore = "requires a live Salesforce org and SF_ACCESS_TOKEN/SF_INSTANCE_URL env vars"]
async fn live_rest_query_malformed_soql_error_payload() -> Result<()> {
    let Some(config) = load_live_config() else {
        eprintln!(
            "skipping live_rest_query_malformed_soql_error_payload: missing SF_ACCESS_TOKEN or SF_INSTANCE_URL"
        );
        return Ok(());
    };

    let result = tokio::time::timeout(config.runtime.test_timeout, async {
        let client = create_live_client(&config).await?;
        client
            .rest()
            .query::<force::types::DynamicSObject>("SELECT FROM Account")
            .await
    })
    .await
    .map_err(|_| HttpError::Timeout {
        timeout_seconds: config.runtime.test_timeout.as_secs(),
    })?;

    let Err(error) = result else {
        panic!("expected malformed query to fail");
    };
    assert_status_error_with_code(&error, 400, &["MALFORMED_QUERY", "INVALID_FIELD"]);
    Ok(())
}

#[tokio::test]
#[ignore = "requires a live Salesforce org and SF_ACCESS_TOKEN/SF_INSTANCE_URL env vars"]
async fn live_rest_query_invalid_locator_error_payload() -> Result<()> {
    let Some(config) = load_live_config() else {
        eprintln!(
            "skipping live_rest_query_invalid_locator_error_payload: missing SF_ACCESS_TOKEN or SF_INSTANCE_URL"
        );
        return Ok(());
    };

    let invalid_locator_path = format!(
        "/services/data/{}/query/this-is-not-a-valid-locator",
        config.api_version
    );

    let result = tokio::time::timeout(config.runtime.test_timeout, async {
        let client = create_live_client(&config).await?;
        client
            .rest()
            .query_more::<force::types::DynamicSObject>(&invalid_locator_path)
            .await
    })
    .await
    .map_err(|_| HttpError::Timeout {
        timeout_seconds: config.runtime.test_timeout.as_secs(),
    })?;

    let Err(error) = result else {
        panic!("expected invalid locator to fail");
    };
    assert_status_error_with_code(
        &error,
        404,
        &["INVALID_QUERY_LOCATOR", "NOT_FOUND", "MALFORMED_QUERY"],
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires a live Salesforce org and SF_ACCESS_TOKEN/SF_INSTANCE_URL env vars"]
async fn live_bulk_ingest_partial_failure_results() -> Result<()> {
    if !env_flag("SF_LIVE_RUN_PARTIAL_FAILURE") {
        eprintln!(
            "skipping live_bulk_ingest_partial_failure_results: set SF_LIVE_RUN_PARTIAL_FAILURE=1 to enable"
        );
        return Ok(());
    }

    let Some(config) = load_live_config() else {
        eprintln!(
            "skipping live_bulk_ingest_partial_failure_results: missing SF_ACCESS_TOKEN or SF_INSTANCE_URL"
        );
        return Ok(());
    };

    let result = tokio::time::timeout(config.runtime.test_timeout, async {
        let client = create_live_client(&config).await?;
        let handler = client.bulk();
        let job = IngestJob::create(&handler, "Account", JobOperation::Insert, None).await?;

        // First row should be valid in most orgs; second row intentionally exceeds
        // standard Account.Name length to trigger a row-level validation failure.
        let long_name = "X".repeat(400);
        let csv_data = format!("Name\nLive Smoke Partial Row\n{long_name}\n");

        let job = job.upload(csv_data).await?;
        let job = job.close().await?;
        let job = job
            .poll_until_complete_with_policy(config.runtime.bulk_poll_policy)
            .await?;
        let successful = String::from_utf8(job.successful_results().await?)
            .map_err(|error| HttpError::InvalidUrl(error.to_string()))?;
        let failed = String::from_utf8(job.failed_results().await?)
            .map_err(|error| HttpError::InvalidUrl(error.to_string()))?;
        Ok::<(String, String), force::error::ForceError>((successful, failed))
    })
    .await
    .map_err(|_| HttpError::Timeout {
        timeout_seconds: config.runtime.test_timeout.as_secs(),
    })??;

    let (successful, failed) = result;
    let successful_rows = successful
        .lines()
        .skip(1)
        .filter(|line| !line.is_empty())
        .count();
    let failed_rows = failed
        .lines()
        .skip(1)
        .filter(|line| !line.is_empty())
        .count();
    assert!(
        successful_rows >= 1,
        "expected at least one successful row, got:\n{successful}",
    );
    assert!(
        failed_rows >= 1,
        "expected at least one failed row, got:\n{failed}",
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires a live Salesforce org and SF_ACCESS_TOKEN/SF_INSTANCE_URL env vars"]
async fn live_rest_throttling_error_payload() -> Result<()> {
    if !env_flag("SF_LIVE_RUN_THROTTLE") {
        eprintln!(
            "skipping live_rest_throttling_error_payload: set SF_LIVE_RUN_THROTTLE=1 to enable"
        );
        return Ok(());
    }

    let Some(config) = load_live_config() else {
        eprintln!(
            "skipping live_rest_throttling_error_payload: missing SF_ACCESS_TOKEN or SF_INSTANCE_URL"
        );
        return Ok(());
    };

    let max_requests = env_u64("SF_LIVE_THROTTLE_MAX_REQUESTS", 5_000);
    let client = create_live_client(&config).await?;

    let mut attempts = 0_u64;
    while attempts < max_requests {
        attempts += 1;
        let result = client
            .rest()
            .query::<force::types::DynamicSObject>("SELECT Id FROM Account LIMIT 1")
            .await;
        if let Err(error) = result {
            match &error {
                ForceError::Http(HttpError::RateLimitExceeded { .. }) => return Ok(()),
                ForceError::Http(HttpError::StatusError {
                    status_code: 429,
                    message,
                }) if message.contains("REQUEST_LIMIT_EXCEEDED") => return Ok(()),
                _ => return Err(error),
            }
        }
    }

    panic!(
        "did not hit throttling within {max_requests} requests; increase SF_LIVE_THROTTLE_MAX_REQUESTS"
    );
}
