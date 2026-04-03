#![allow(missing_docs)]
#![cfg(all(feature = "rest", feature = "bulk"))]
//! Live Salesforce smoke tests for REST and Bulk APIs.
//!
//! These tests are ignored by default and require:
//! - `SF_ACCESS_TOKEN`
//! - `SF_INSTANCE_URL`
//! - optional `SF_API_VERSION` (defaults to `v60.0`)
//! - or a locally authenticated Salesforce CLI org, optionally selected via
//!   `SF_TARGET_ORG`

use async_trait::async_trait;
use force::api::bulk::{BulkPollPolicy, IngestJob, JobOperation};
use force::api::rest_operation::RestOperation;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::{ForceClient, builder};
use force::config::ClientConfig;
use force::error::ForceError;
use force::error::HttpError;
use force::error::Result;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone)]
struct LiveCredentials {
    access_token: String,
    instance_url: String,
}

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

fn assert_status_error_with_any_status(
    err: &ForceError,
    expected_statuses: &[u16],
    expected_codes: &[&str],
) {
    match err {
        ForceError::Http(HttpError::StatusError {
            status_code,
            message,
        }) => {
            assert!(
                expected_statuses.contains(status_code),
                "expected one of {expected_statuses:?}, got status {status_code}",
            );
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

fn env_string(env_lookup: &impl Fn(&str) -> Option<String>, key: &str) -> Option<String> {
    env_lookup(key).and_then(|value| {
        let value = value.trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    })
}

#[derive(Debug, Deserialize)]
struct SfCliOrgDisplayEnvelope {
    result: SfCliOrgDisplayResult,
}

#[derive(Debug, Deserialize)]
struct SfCliOrgDisplayResult {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    #[serde(rename = "instanceUrl")]
    instance_url: Option<String>,
}

fn parse_sf_cli_org_display(payload: &str) -> Option<LiveCredentials> {
    let parsed: SfCliOrgDisplayEnvelope = serde_json::from_str(payload).ok()?;
    let access_token = parsed.result.access_token?;
    let instance_url = parsed.result.instance_url?;
    Some(LiveCredentials {
        access_token,
        instance_url,
    })
}

const fn sf_cli_command_candidates() -> &'static [&'static str] {
    #[cfg(windows)]
    {
        &["sf.cmd", "sf"]
    }

    #[cfg(not(windows))]
    {
        &["sf"]
    }
}

fn load_live_credentials_from_sf_cli(target_org: Option<&str>) -> Option<LiveCredentials> {
    for command_name in sf_cli_command_candidates() {
        let mut command = std::process::Command::new(command_name);
        command.args(["org", "display", "--verbose", "--json"]);
        if let Some(target_org) = target_org {
            command.args(["--target-org", target_org]);
        }

        let Ok(output) = command.output() else {
            continue;
        };
        if !output.status.success() {
            continue;
        }

        let Ok(stdout) = String::from_utf8(output.stdout) else {
            continue;
        };
        if let Some(credentials) = parse_sf_cli_org_display(&stdout) {
            return Some(credentials);
        }
    }

    None
}

fn load_live_credentials_with(
    env_lookup: &impl Fn(&str) -> Option<String>,
    sf_cli_loader: impl FnOnce(Option<&str>) -> Option<LiveCredentials>,
) -> Option<LiveCredentials> {
    let access_token = env_string(env_lookup, "SF_ACCESS_TOKEN");
    let instance_url = env_string(env_lookup, "SF_INSTANCE_URL");
    if let (Some(access_token), Some(instance_url)) = (access_token, instance_url) {
        return Some(LiveCredentials {
            access_token,
            instance_url,
        });
    }

    let target_org = env_string(env_lookup, "SF_TARGET_ORG");
    sf_cli_loader(target_org.as_deref())
}

fn load_live_config() -> Option<LiveConfig> {
    let credentials = load_live_credentials_with(
        &|key| std::env::var(key).ok(),
        load_live_credentials_from_sf_cli,
    )?;
    let api_version = std::env::var("SF_API_VERSION").unwrap_or_else(|_| "v60.0".to_string());
    Some(LiveConfig {
        access_token: credentials.access_token,
        instance_url: credentials.instance_url,
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
    assert_status_error_with_any_status(
        &error,
        &[400, 404],
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

#[cfg(test)]
mod config_resolution_tests {
    use super::*;
    use serde_json::json;
    use std::cell::Cell;
    use std::collections::HashMap;

    fn env_map(entries: &[(&str, &str)]) -> HashMap<String, String> {
        entries
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    #[test]
    fn live_credentials_prefer_explicit_env_over_sf_cli() {
        let env = env_map(&[
            ("SF_ACCESS_TOKEN", "env-token"),
            ("SF_INSTANCE_URL", "https://env.example.com"),
        ]);
        let cli_calls = Cell::new(0_u32);

        let credentials = load_live_credentials_with(&|key| env.get(key).cloned(), |target_org| {
            cli_calls.set(cli_calls.get() + 1);
            assert_eq!(target_org, None);
            Some(LiveCredentials {
                access_token: "cli-token".to_string(),
                instance_url: "https://cli.example.com".to_string(),
            })
        });

        let Some(credentials) = credentials else {
            panic!("expected env credentials to win");
        };
        assert_eq!(credentials.access_token, "env-token");
        assert_eq!(credentials.instance_url, "https://env.example.com");
        assert_eq!(cli_calls.get(), 0);
    }

    #[test]
    fn live_credentials_fall_back_to_sf_cli_target_org() {
        let env = env_map(&[("SF_TARGET_ORG", "dev-hydra")]);

        let Some(credentials) =
            load_live_credentials_with(&|key| env.get(key).cloned(), |target_org| {
                assert_eq!(target_org, Some("dev-hydra"));
                Some(LiveCredentials {
                    access_token: "cli-token".to_string(),
                    instance_url: "https://cli.example.com".to_string(),
                })
            })
        else {
            panic!("expected sf cli credentials");
        };

        assert_eq!(credentials.access_token, "cli-token");
        assert_eq!(credentials.instance_url, "https://cli.example.com");
    }

    #[test]
    fn parse_sf_cli_org_display_extracts_credentials() {
        let payload = json!({
            "status": 0,
            "result": {
                "accessToken": "00Dxx!token",
                "instanceUrl": "https://dev-org.my.salesforce.com"
            }
        });

        let Some(credentials) = parse_sf_cli_org_display(&payload.to_string()) else {
            panic!("expected verbose sf payload");
        };

        assert_eq!(credentials.access_token, "00Dxx!token");
        assert_eq!(
            credentials.instance_url,
            "https://dev-org.my.salesforce.com"
        );
    }

    #[test]
    fn sf_cli_command_candidates_match_platform() {
        let candidates = sf_cli_command_candidates();

        #[cfg(windows)]
        assert_eq!(candidates.first().copied(), Some("sf.cmd"));

        #[cfg(not(windows))]
        assert_eq!(candidates.first().copied(), Some("sf"));
    }
}
