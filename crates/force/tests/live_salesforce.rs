#![cfg(all(feature = "rest", feature = "bulk"))]
//! Live Salesforce smoke tests for REST and Bulk APIs.
//!
//! These tests are ignored by default and require:
//! - `SF_ACCESS_TOKEN`
//! - `SF_INSTANCE_URL`
//! - optional `SF_API_VERSION` (defaults to `v60.0`)

use async_trait::async_trait;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::{builder, ForceClient};
use force::config::ClientConfigBuilder;
use force::error::Result;
use serde::Deserialize;

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
}

fn load_live_config() -> Option<LiveConfig> {
    let access_token = std::env::var("SF_ACCESS_TOKEN").ok()?;
    let instance_url = std::env::var("SF_INSTANCE_URL").ok()?;
    let api_version = std::env::var("SF_API_VERSION").unwrap_or_else(|_| "v60.0".to_string());
    Some(LiveConfig {
        access_token,
        instance_url,
        api_version,
    })
}

async fn create_live_client(config: &LiveConfig) -> Result<ForceClient<EnvAuthenticator>> {
    let auth = EnvAuthenticator {
        access_token: config.access_token.clone(),
        instance_url: config.instance_url.clone(),
    };

    let client_config = ClientConfigBuilder::new()
        .api_version(config.api_version.clone())
        .build();

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

    let client = create_live_client(&config).await?;
    let result = client
        .query::<force::types::DynamicSObject>("SELECT Id FROM Account LIMIT 1")
        .await?;

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
        eprintln!("skipping live_bulk_query_stream_smoke: missing SF_ACCESS_TOKEN or SF_INSTANCE_URL");
        return Ok(());
    };

    let client = create_live_client(&config).await?;
    let mut stream = client
        .bulk()
        .bulk_query::<LiveAccountRow>("SELECT Id FROM Account LIMIT 5")
        .await?;

    let mut seen = 0usize;
    while let Some(row) = stream.next().await? {
        assert!(!row.id.is_empty());
        seen += 1;
        if seen >= 5 {
            break;
        }
    }

    assert!(seen <= 5);
    Ok(())
}
