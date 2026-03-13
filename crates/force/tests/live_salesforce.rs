#![allow(missing_docs)]
#![cfg(all(feature = "rest", feature = "bulk"))]
//! Live Salesforce smoke tests for REST and Bulk APIs.
//!
//! These tests are ignored by default and require:
//! - `SF_ACCESS_TOKEN`
//! - `SF_INSTANCE_URL`
//! - optional `SF_API_VERSION` (defaults to `v60.0`)

use async_trait::async_trait;
use force::api::BulkPollPolicy;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::{ForceClient, builder};
use force::config::ClientConfig;
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
