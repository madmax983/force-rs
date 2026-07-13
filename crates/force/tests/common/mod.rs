#![allow(dead_code)]
//! Shared harness for the tiered, env-gated live-contract test suite.
//!
//! This module mirrors the auth contract of `tests/live_salesforce.rs` so that
//! the sibling integration-test binaries (`live_core`, `live_special`,
//! `live_account_engagement`) share one credential loader and skip idiom. It is
//! deliberately included via `mod common;` from each test binary; because Rust
//! compiles it once per binary, unused helpers trip `dead_code` — hence the
//! blanket `allow` above.
//!
//! # Auth contract (identical env var names to `live_salesforce.rs`)
//!
//! The core loader tries sources in priority order:
//!
//! 1. **JWT Bearer** (feature `jwt`):
//!    `SF_JWT_CLIENT_ID`, `SF_JWT_USERNAME`, `SF_JWT_PRIVATE_KEY_PATH`,
//!    optional `SF_JWT_LOGIN_URL` (default `https://login.salesforce.com`).
//! 2. **Client Credentials**: `SF_CLIENT_ID`, `SF_CLIENT_SECRET`, `SF_TOKEN_URL`.
//! 3. **Username-Password** (feature `username_password`): `SF_UP_CLIENT_ID`,
//!    `SF_UP_CLIENT_SECRET`, `SF_UP_USERNAME`, `SF_UP_PASSWORD`,
//!    `SF_UP_SECURITY_TOKEN`, optional `SF_UP_TOKEN_URL`.
//! 4. **Bare access token**: `SF_ACCESS_TOKEN`, `SF_INSTANCE_URL`.
//! 5. **Salesforce CLI**: `SF_TARGET_ORG` (or the default org).
//!
//! Tier loaders (each returns `Option`, gated on its own env vars) enable the
//! special-surface tests: Data Cloud, Apex REST, Consent, Models, Agent API,
//! CPQ, and Account Engagement.

use async_trait::async_trait;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::{ForceClient, builder};
use force::config::ClientConfig;
use force::error::Result;

#[cfg(feature = "jwt")]
use force::auth::JwtBearerFlow;

/// Prefix for every ephemeral record these tests create. Cleanup targets only
/// records whose name/field carries this prefix so live runs never touch real
/// org data.
pub const LIVE_TEST_PREFIX: &str = "force-rs-live-test";

// ─── Unified live authenticator ────────────────────────────────────────────

/// Authenticator that dispatches to whichever flow supplied credentials.
#[derive(Debug, Clone)]
pub enum LiveAuth {
    #[cfg(feature = "jwt")]
    Jwt(JwtBearerFlow),
    ClientCredentials(force::auth::ClientCredentials),
    #[cfg(feature = "username_password")]
    UsernamePassword(force::auth::UsernamePassword),
    Token(EnvAuthenticator),
}

#[async_trait]
impl Authenticator for LiveAuth {
    async fn authenticate(&self) -> Result<AccessToken> {
        match self {
            #[cfg(feature = "jwt")]
            Self::Jwt(flow) => flow.authenticate().await,
            Self::ClientCredentials(flow) => flow.authenticate().await,
            #[cfg(feature = "username_password")]
            Self::UsernamePassword(flow) => flow.authenticate().await,
            Self::Token(env) => env.authenticate().await,
        }
    }

    async fn refresh(&self) -> Result<AccessToken> {
        match self {
            #[cfg(feature = "jwt")]
            Self::Jwt(flow) => flow.refresh().await,
            Self::ClientCredentials(flow) => flow.refresh().await,
            #[cfg(feature = "username_password")]
            Self::UsernamePassword(flow) => flow.refresh().await,
            Self::Token(env) => env.refresh().await,
        }
    }
}

impl std::fmt::Display for LiveAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(feature = "jwt")]
            Self::Jwt(_) => write!(f, "JWT Bearer"),
            Self::ClientCredentials(_) => write!(f, "Client Credentials"),
            #[cfg(feature = "username_password")]
            Self::UsernamePassword(_) => write!(f, "Username-Password"),
            Self::Token(_) => write!(f, "Access Token"),
        }
    }
}

/// Bare-token authenticator used by the `SF_ACCESS_TOKEN` and Salesforce CLI
/// fallbacks.
#[derive(Debug, Clone)]
pub struct EnvAuthenticator {
    access_token: String,
    instance_url: String,
}

#[async_trait]
impl Authenticator for EnvAuthenticator {
    async fn authenticate(&self) -> Result<AccessToken> {
        Ok(AccessToken::from_response(TokenResponse {
            access_token: secrecy::SecretString::new(self.access_token.clone().into()),
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

// ─── Env helpers ───────────────────────────────────────────────────────────

/// Reads a trimmed, non-empty env var, or `None`.
pub fn env_string(key: &str) -> Option<String> {
    std::env::var(key).ok().and_then(|v| {
        let v = v.trim().to_string();
        if v.is_empty() { None } else { Some(v) }
    })
}

/// Prints a standardized skip line to stderr for the given test.
pub fn skip(test_name: &str, reason: &str) {
    eprintln!("SKIP {test_name}: {reason}");
}

/// Normalizes a Salesforce OAuth token URL: accepts a bare host, base URL, or a
/// full `/services/oauth2/token` endpoint and always returns the token endpoint.
fn normalize_token_url(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    let base = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    if base.ends_with("/services/oauth2/token") {
        base
    } else {
        format!("{base}/services/oauth2/token")
    }
}

/// Resolves the JWT `(audience, token_url)` pair from a login URL.
#[cfg(feature = "jwt")]
fn jwt_endpoints(login_url: &str) -> (String, String) {
    let trimmed = login_url.trim().trim_end_matches('/');
    let base = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    let audience = base
        .trim_end_matches("/services/oauth2/token")
        .trim_end_matches('/')
        .to_string();
    let token_url = format!("{audience}/services/oauth2/token");
    (audience, token_url)
}

/// Resolves a private key path, trying `CARGO_MANIFEST_DIR` and the workspace
/// root as fallbacks for a relative path.
#[cfg(feature = "jwt")]
fn resolve_key_path(raw: &str) -> std::path::PathBuf {
    let path = std::path::PathBuf::from(raw);
    if path.exists() || !path.is_relative() {
        return path;
    }
    let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") else {
        return path;
    };
    let from_manifest = std::path::PathBuf::from(&manifest_dir).join(&path);
    if from_manifest.exists() {
        return from_manifest;
    }
    if let Some(root) = std::path::PathBuf::from(&manifest_dir)
        .parent()
        .and_then(|p| p.parent())
    {
        let from_root = root.join(&path);
        if from_root.exists() {
            return from_root;
        }
    }
    path
}

// ─── Core-tier config ──────────────────────────────────────────────────────

/// Resolved core-tier configuration: an authenticator plus the API version.
pub struct CoreConfig {
    pub auth: LiveAuth,
    pub api_version: String,
}

#[cfg(feature = "jwt")]
fn try_jwt_auth() -> Option<LiveAuth> {
    let client_id = env_string("SF_JWT_CLIENT_ID")?;
    let username = env_string("SF_JWT_USERNAME")?;
    let private_key_path = env_string("SF_JWT_PRIVATE_KEY_PATH")?;

    let resolved = resolve_key_path(&private_key_path);
    let private_key_pem = std::fs::read_to_string(&resolved).ok()?;
    let login_url = env_string("SF_JWT_LOGIN_URL")
        .unwrap_or_else(|| "https://login.salesforce.com".to_string());
    let (audience, token_url) = jwt_endpoints(&login_url);

    let flow = JwtBearerFlow::new(
        &client_id,
        &username,
        &private_key_pem,
        audience.as_str(),
        token_url.as_str(),
    )
    .ok()?;
    Some(LiveAuth::Jwt(flow))
}

fn try_client_credentials_auth() -> Option<LiveAuth> {
    let client_id = env_string("SF_CLIENT_ID")?;
    let client_secret = env_string("SF_CLIENT_SECRET")?;
    let token_url = normalize_token_url(&env_string("SF_TOKEN_URL")?);
    Some(LiveAuth::ClientCredentials(
        force::auth::ClientCredentials::new(client_id, client_secret, token_url),
    ))
}

#[cfg(feature = "username_password")]
fn try_username_password_auth() -> Option<LiveAuth> {
    let client_id = env_string("SF_UP_CLIENT_ID")?;
    let client_secret = env_string("SF_UP_CLIENT_SECRET")?;
    let username = env_string("SF_UP_USERNAME")?;
    let password = env_string("SF_UP_PASSWORD")?;
    let security_token = env_string("SF_UP_SECURITY_TOKEN").unwrap_or_default();
    let token_url = normalize_token_url(
        &env_string("SF_UP_TOKEN_URL")
            .unwrap_or_else(|| "https://login.salesforce.com".to_string()),
    );
    Some(LiveAuth::UsernamePassword(
        force::auth::UsernamePassword::new(
            client_id,
            client_secret,
            username,
            password,
            security_token,
            token_url,
        ),
    ))
}

fn try_token_auth() -> Option<LiveAuth> {
    let access_token = env_string("SF_ACCESS_TOKEN")?;
    let instance_url = env_string("SF_INSTANCE_URL")?;
    Some(LiveAuth::Token(EnvAuthenticator {
        access_token,
        instance_url,
    }))
}

#[derive(Debug, serde::Deserialize)]
struct SfCliOrgDisplayEnvelope {
    result: SfCliOrgDisplayResult,
}

#[derive(Debug, serde::Deserialize)]
struct SfCliOrgDisplayResult {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    #[serde(rename = "instanceUrl")]
    instance_url: Option<String>,
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

fn try_sf_cli_auth() -> Option<LiveAuth> {
    let target_org = env_string("SF_TARGET_ORG");

    for command_name in sf_cli_command_candidates() {
        let mut command = std::process::Command::new(command_name);
        command.args(["org", "display", "--verbose", "--json"]);
        if let Some(ref org) = target_org {
            command.args(["--target-org", org]);
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
        let Ok(envelope) = serde_json::from_str::<SfCliOrgDisplayEnvelope>(&stdout) else {
            continue;
        };
        if let (Some(access_token), Some(instance_url)) =
            (envelope.result.access_token, envelope.result.instance_url)
        {
            return Some(LiveAuth::Token(EnvAuthenticator {
                access_token,
                instance_url,
            }));
        }
    }
    None
}

fn load_live_auth() -> Option<LiveAuth> {
    #[cfg(feature = "jwt")]
    if let Some(auth) = try_jwt_auth() {
        return Some(auth);
    }
    if let Some(auth) = try_client_credentials_auth() {
        return Some(auth);
    }
    #[cfg(feature = "username_password")]
    if let Some(auth) = try_username_password_auth() {
        return Some(auth);
    }
    if let Some(auth) = try_token_auth() {
        return Some(auth);
    }
    try_sf_cli_auth()
}

/// Loads core-tier credentials, trying every auth source in priority order.
///
/// Returns `None` when no credentials are configured, which callers turn into a
/// clean skip.
pub fn load_core_config() -> Option<CoreConfig> {
    let auth = load_live_auth()?;
    let api_version = env_string("SF_API_VERSION").unwrap_or_else(|| "v62.0".to_string());
    Some(CoreConfig { auth, api_version })
}

/// Builds a `ForceClient` from a resolved [`CoreConfig`].
///
/// # Errors
///
/// Returns an error if the client builder fails (e.g. initial authentication).
pub async fn create_client(config: &CoreConfig) -> Result<ForceClient<LiveAuth>> {
    let client_config = ClientConfig {
        api_version: config.api_version.clone(),
        ..Default::default()
    };
    builder()
        .config(client_config)
        .authenticate(config.auth.clone())
        .build()
        .await
}

// ─── Special-surface tier loaders ──────────────────────────────────────────

/// Data Cloud tier: gated on the `SF_DATA_CLOUD` boolean flag.
///
/// Returns a [`DataCloudConfig`], honoring an optional
/// `SF_DATA_CLOUD_TOKEN_URL` override for the token-exchange endpoint.
#[cfg(feature = "data_cloud")]
pub fn load_data_cloud() -> Option<force::auth::DataCloudConfig> {
    let enabled = std::env::var("SF_DATA_CLOUD").is_ok_and(|v| {
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    });
    if !enabled {
        return None;
    }
    Some(force::auth::DataCloudConfig {
        token_exchange_url: env_string("SF_DATA_CLOUD_TOKEN_URL"),
        api_version: env_string("SF_DATA_CLOUD_API_VERSION"),
    })
}

/// Apex REST tier: the custom `/services/apexrest/{path}` suffix to GET.
pub fn apex_rest_path() -> Option<String> {
    env_string("SF_APEX_REST_PATH")
}

/// Consent tier: the `(action, ids)` pair from `SF_CONSENT_ACTION` +
/// comma-separated `SF_CONSENT_IDS`.
pub fn consent_params() -> Option<(String, Vec<String>)> {
    let action = env_string("SF_CONSENT_ACTION")?;
    let ids: Vec<String> = env_string("SF_CONSENT_IDS")?
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if ids.is_empty() {
        return None;
    }
    Some((action, ids))
}

/// Models (Agentforce) tier: the model name from `SF_MODELS_MODEL`.
pub fn models_model() -> Option<String> {
    env_string("SF_MODELS_MODEL")
}

/// Agent API tier: the agent id from `SF_AGENT_ID`.
pub fn agent_id() -> Option<String> {
    env_string("SF_AGENT_ID")
}

/// Account Engagement (Pardot) tier: the business unit id from
/// `SF_AE_BUSINESS_UNIT_ID`.
pub fn account_engagement_bu() -> Option<String> {
    env_string("SF_AE_BUSINESS_UNIT_ID")
}

/// CPQ tier: an existing quote id from `SF_CPQ_QUOTE_ID`.
pub fn cpq_quote_id() -> Option<String> {
    env_string("SF_CPQ_QUOTE_ID")
}
