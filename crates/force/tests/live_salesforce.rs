#![allow(missing_docs)]
#![cfg(all(feature = "rest", feature = "bulk"))]
//! Live Salesforce contract tests for all API surfaces and auth flows.
//!
//! These tests are ignored by default and require at least one set of
//! authentication credentials. The loader tries sources in order:
//!
//! 1. **JWT Bearer** (feature `jwt`) — renewable, preferred for CI:
//!    - `SF_JWT_CLIENT_ID`, `SF_JWT_USERNAME`, `SF_JWT_PRIVATE_KEY_PATH`
//!    - optional `SF_JWT_LOGIN_URL` (defaults to `https://login.salesforce.com`)
//!
//! 2. **Client Credentials** — renewable, preferred for server-to-server:
//!    - `SF_CLIENT_ID`, `SF_CLIENT_SECRET`
//!    - optional `SF_TOKEN_URL` (defaults to production)
//!
//! 3. **Username-Password** (feature `username_password`) — deprecated by Salesforce:
//!    - `SF_UP_CLIENT_ID`, `SF_UP_CLIENT_SECRET`, `SF_UP_USERNAME`,
//!      `SF_UP_PASSWORD`, `SF_UP_SECURITY_TOKEN`
//!    - optional `SF_UP_TOKEN_URL` (defaults to production)
//!
//! 4. **Bare access token** — temporary, fallback only:
//!    - `SF_ACCESS_TOKEN`, `SF_INSTANCE_URL`
//!
//! 5. **Salesforce CLI** — local dev convenience:
//!    - `SF_TARGET_ORG` or default org from `sf org display`

use async_trait::async_trait;
use force::api::RestOperation;
use force::api::bulk::{BulkPollPolicy, IngestJob, JobOperation};
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::{ForceClient, builder};
use force::config::ClientConfig;
use force::error::ForceError;
use force::error::HttpError;
use force::error::Result;
use serde::Deserialize;
use std::time::Duration;

#[cfg(feature = "jwt")]
use force::auth::JwtBearerFlow;

// ─── Auth Flow Enum ────────────────────────────────────────────────────────

/// Unified authenticator that dispatches to whichever flow has credentials.
#[derive(Debug, Clone)]
enum LiveAuth {
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

// ─── Bare Token Authenticator (fallback) ───────────────────────────────────

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

// ─── Environment Helpers ───────────────────────────────────────────────────

fn env_string(key: &str) -> Option<String> {
    std::env::var(key).ok().and_then(|v| {
        let v = v.trim().to_string();
        if v.is_empty() { None } else { Some(v) }
    })
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
    std::env::var(key).is_ok_and(|value| {
        let value = value.to_ascii_lowercase();
        matches!(value.as_str(), "1" | "true" | "yes" | "on")
    })
}

/// Resolves a private key path, trying CARGO_MANIFEST_DIR as a fallback
/// when a relative path doesn't exist from the current working directory.
#[cfg(feature = "jwt")]
fn resolve_key_path(raw: &str) -> std::path::PathBuf {
    let path = std::path::PathBuf::from(raw);
    if path.exists() {
        return path;
    }
    if path.is_relative() {
        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let from_manifest = std::path::PathBuf::from(&manifest_dir).join(&path);
            if from_manifest.exists() {
                return from_manifest;
            }
            let workspace_root = std::path::PathBuf::from(&manifest_dir)
                .parent()
                .and_then(|p| p.parent())
                .map(|p| p.to_path_buf());
            if let Some(root) = workspace_root {
                let from_root = root.join(&path);
                if from_root.exists() {
                    return from_root;
                }
            }
        }
    }
    path
}

// ─── Credential Loading ────────────────────────────────────────────────────

const PRODUCTION_TOKEN_URL: &str = "https://login.salesforce.com/services/oauth2/token";
const PRODUCTION_LOGIN_URL: &str = "https://login.salesforce.com";

/// Tries to build a JWT Bearer authenticator from env vars.
#[cfg(feature = "jwt")]
fn try_jwt_auth() -> Option<LiveAuth> {
    let client_id = env_string("SF_JWT_CLIENT_ID")?;
    let username = env_string("SF_JWT_USERNAME")?;
    let private_key_path = env_string("SF_JWT_PRIVATE_KEY_PATH")?;

    let resolved = resolve_key_path(&private_key_path);
    let private_key_pem = std::fs::read_to_string(&resolved)
        .unwrap_or_else(|e| panic!("Failed to read private key at {}: {e}", resolved.display()));

    let login_url = env_string("SF_JWT_LOGIN_URL")
        .unwrap_or_else(|| PRODUCTION_LOGIN_URL.to_string());
    let token_url = format!("{login_url}/services/oauth2/token");

    let flow = JwtBearerFlow::new(&client_id, &username, &private_key_pem, &login_url, &token_url)
        .unwrap_or_else(|e| panic!("Invalid JWT config: {e}"));

    Some(LiveAuth::Jwt(flow))
}

/// Tries to build a Client Credentials authenticator from env vars.
fn try_client_credentials_auth() -> Option<LiveAuth> {
    let client_id = env_string("SF_CLIENT_ID")?;
    let client_secret = env_string("SF_CLIENT_SECRET")?;
    let token_url = env_string("SF_TOKEN_URL")
        .unwrap_or_else(|| PRODUCTION_TOKEN_URL.to_string());

    let flow = force::auth::ClientCredentials::new(client_id, client_secret, token_url);
    Some(LiveAuth::ClientCredentials(flow))
}

/// Tries to build a Username-Password authenticator from env vars.
#[cfg(feature = "username_password")]
fn try_username_password_auth() -> Option<LiveAuth> {
    let client_id = env_string("SF_UP_CLIENT_ID")?;
    let client_secret = env_string("SF_UP_CLIENT_SECRET")?;
    let username = env_string("SF_UP_USERNAME")?;
    let password = env_string("SF_UP_PASSWORD")?;
    let security_token = env_string("SF_UP_SECURITY_TOKEN").unwrap_or_default();
    let token_url = env_string("SF_UP_TOKEN_URL")
        .unwrap_or_else(|| PRODUCTION_TOKEN_URL.to_string());

    let flow = force::auth::UsernamePassword::new(
        client_id,
        client_secret,
        username,
        password,
        security_token,
        token_url,
    );
    Some(LiveAuth::UsernamePassword(flow))
}

/// Tries to build a bare-token authenticator from env vars.
fn try_token_auth() -> Option<LiveAuth> {
    let access_token = env_string("SF_ACCESS_TOKEN")?;
    let instance_url = env_string("SF_INSTANCE_URL")?;
    Some(LiveAuth::Token(EnvAuthenticator {
        access_token,
        instance_url,
    }))
}

// ─── Salesforce CLI Fallback ───────────────────────────────────────────────

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

// ─── Config ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
struct LiveRuntimeConfig {
    test_timeout: Duration,
    bulk_poll_policy: BulkPollPolicy,
    bulk_query_row_limit: usize,
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

struct LiveConfig {
    auth: LiveAuth,
    api_version: String,
    runtime: LiveRuntimeConfig,
}

/// Loads credentials trying each auth source in priority order.
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

fn load_live_config() -> Option<LiveConfig> {
    let auth = load_live_auth()?;
    let api_version = std::env::var("SF_API_VERSION").unwrap_or_else(|_| "v62.0".to_string());
    Some(LiveConfig {
        auth,
        api_version,
        runtime: load_runtime_config(),
    })
}

async fn create_live_client(config: &LiveConfig) -> Result<ForceClient<LiveAuth>> {
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

// ─── Assertion Helpers ─────────────────────────────────────────────────────

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

// ═══════════════════════════════════════════════════════════════════════════
//  AUTH FLOW TESTS
// ═══════════════════════════════════════════════════════════════════════════

// ─── JWT Bearer Flow Tests ─────────────────────────────────────────────────

#[cfg(feature = "jwt")]
mod jwt_auth_tests {
    use super::*;
    use force::auth::{Authenticator, JwtBearerFlow};

    fn load_jwt_flow() -> Option<JwtBearerFlow> {
        let client_id = env_string("SF_JWT_CLIENT_ID")?;
        let username = env_string("SF_JWT_USERNAME")?;
        let private_key_path = env_string("SF_JWT_PRIVATE_KEY_PATH")?;

        let resolved = resolve_key_path(&private_key_path);
        let private_key_pem = std::fs::read_to_string(&resolved)
            .unwrap_or_else(|e| {
                panic!("Failed to read private key at {}: {e}", resolved.display())
            });

        let login_url = env_string("SF_JWT_LOGIN_URL")
            .unwrap_or_else(|| PRODUCTION_LOGIN_URL.to_string());
        let token_url = format!("{login_url}/services/oauth2/token");

        Some(
            JwtBearerFlow::new(&client_id, &username, &private_key_pem, &login_url, &token_url)
                .unwrap_or_else(|e| panic!("Invalid JWT config: {e}")),
        )
    }

    #[tokio::test]
    #[ignore = "requires live Salesforce org with JWT Bearer Flow configured"]
    async fn live_jwt_bearer_authenticate_returns_valid_token() -> Result<()> {
        let Some(flow) = load_jwt_flow() else {
            eprintln!("skipping: missing SF_JWT_CLIENT_ID, SF_JWT_USERNAME, or SF_JWT_PRIVATE_KEY_PATH");
            return Ok(());
        };

        let token = tokio::time::timeout(Duration::from_secs(30), flow.authenticate())
            .await
            .map_err(|_| HttpError::Timeout { timeout_seconds: 30 })??;

        assert!(!token.as_str().is_empty(), "access token must not be empty");
        assert!(
            token.instance_url().starts_with("https://"),
            "instance_url must start with https://, got: {}",
            token.instance_url(),
        );
        assert_eq!(token.token_type(), "Bearer", "token_type must be Bearer");
        assert!(!token.is_expired(), "token must not be expired immediately after authenticate()");

        eprintln!("JWT auth succeeded — instance_url: {}", token.instance_url());
        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires live Salesforce org with JWT Bearer Flow configured"]
    async fn live_jwt_bearer_refresh_returns_fresh_token() -> Result<()> {
        let Some(flow) = load_jwt_flow() else {
            eprintln!("skipping: missing JWT env vars");
            return Ok(());
        };

        let token1 = tokio::time::timeout(Duration::from_secs(30), flow.authenticate())
            .await
            .map_err(|_| HttpError::Timeout { timeout_seconds: 30 })??;

        let token2 = tokio::time::timeout(Duration::from_secs(30), flow.refresh())
            .await
            .map_err(|_| HttpError::Timeout { timeout_seconds: 30 })??;

        assert!(!token1.as_str().is_empty());
        assert!(!token2.as_str().is_empty());
        assert_eq!(
            token1.instance_url(),
            token2.instance_url(),
            "refresh must return a token for the same instance",
        );

        eprintln!("JWT refresh succeeded — both tokens valid for {}", token1.instance_url());
        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires live Salesforce org with JWT Bearer Flow configured"]
    async fn live_jwt_bearer_invalid_client_id_fails() -> Result<()> {
        let Some(flow) = load_jwt_flow() else {
            eprintln!("skipping: missing JWT env vars");
            return Ok(());
        };

        // Re-create with bogus client_id but same key + username.
        let private_key_path = env_string("SF_JWT_PRIVATE_KEY_PATH").unwrap();
        let resolved = resolve_key_path(&private_key_path);
        let pem = std::fs::read_to_string(&resolved).unwrap();
        let login_url = env_string("SF_JWT_LOGIN_URL")
            .unwrap_or_else(|| PRODUCTION_LOGIN_URL.to_string());
        let token_url = format!("{login_url}/services/oauth2/token");
        drop(flow);

        let bad_flow =
            JwtBearerFlow::new("INVALID_CLIENT_ID", "user@test.com", &pem, &login_url, &token_url)?;

        let result = tokio::time::timeout(Duration::from_secs(30), bad_flow.authenticate())
            .await
            .map_err(|_| HttpError::Timeout { timeout_seconds: 30 })?;

        assert!(result.is_err(), "authenticate with an invalid client_id must fail");
        let err_str = result.unwrap_err().to_string();
        assert!(
            err_str.contains("invalid_client") || err_str.contains("invalid_grant"),
            "error must mention invalid_client or invalid_grant, got: {err_str}",
        );

        eprintln!("Invalid client_id correctly rejected: {err_str}");
        Ok(())
    }
}

// ─── Client Credentials Flow Tests ─────────────────────────────────────────

mod client_credentials_auth_tests {
    use super::*;
    use force::auth::{Authenticator, ClientCredentials};

    fn load_cc_flow() -> Option<ClientCredentials> {
        let client_id = env_string("SF_CLIENT_ID")?;
        let client_secret = env_string("SF_CLIENT_SECRET")?;
        let token_url = env_string("SF_TOKEN_URL")
            .unwrap_or_else(|| PRODUCTION_TOKEN_URL.to_string());
        Some(ClientCredentials::new(client_id, client_secret, token_url))
    }

    #[tokio::test]
    #[ignore = "requires live Salesforce org with Client Credentials configured"]
    async fn live_client_credentials_authenticate_returns_valid_token() -> Result<()> {
        let Some(flow) = load_cc_flow() else {
            eprintln!("skipping: missing SF_CLIENT_ID or SF_CLIENT_SECRET");
            return Ok(());
        };

        let token = tokio::time::timeout(Duration::from_secs(30), flow.authenticate())
            .await
            .map_err(|_| HttpError::Timeout { timeout_seconds: 30 })??;

        assert!(!token.as_str().is_empty(), "access token must not be empty");
        assert!(
            token.instance_url().starts_with("https://"),
            "instance_url must start with https://, got: {}",
            token.instance_url(),
        );
        assert_eq!(token.token_type(), "Bearer", "token_type must be Bearer");
        assert!(!token.is_expired(), "token must not be expired immediately after authenticate()");

        eprintln!("Client Credentials auth succeeded — instance_url: {}", token.instance_url());
        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires live Salesforce org with Client Credentials configured"]
    async fn live_client_credentials_refresh_returns_fresh_token() -> Result<()> {
        let Some(flow) = load_cc_flow() else {
            eprintln!("skipping: missing SF_CLIENT_ID or SF_CLIENT_SECRET");
            return Ok(());
        };

        let token1 = tokio::time::timeout(Duration::from_secs(30), flow.authenticate())
            .await
            .map_err(|_| HttpError::Timeout { timeout_seconds: 30 })??;

        let token2 = tokio::time::timeout(Duration::from_secs(30), flow.refresh())
            .await
            .map_err(|_| HttpError::Timeout { timeout_seconds: 30 })??;

        assert!(!token1.as_str().is_empty());
        assert!(!token2.as_str().is_empty());
        assert_eq!(
            token1.instance_url(),
            token2.instance_url(),
            "refresh must return a token for the same instance",
        );

        eprintln!(
            "Client Credentials refresh succeeded — both tokens valid for {}",
            token1.instance_url(),
        );
        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires live Salesforce org with Client Credentials configured"]
    async fn live_client_credentials_invalid_secret_fails() -> Result<()> {
        let Some(_) = env_string("SF_CLIENT_ID") else {
            eprintln!("skipping: missing SF_CLIENT_ID");
            return Ok(());
        };

        let client_id = env_string("SF_CLIENT_ID").unwrap();
        let token_url = env_string("SF_TOKEN_URL")
            .unwrap_or_else(|| PRODUCTION_TOKEN_URL.to_string());

        let bad_flow = ClientCredentials::new(client_id, "INVALID_SECRET", token_url);

        let result = tokio::time::timeout(Duration::from_secs(30), bad_flow.authenticate())
            .await
            .map_err(|_| HttpError::Timeout { timeout_seconds: 30 })?;

        assert!(result.is_err(), "authenticate with an invalid secret must fail");
        let err_str = result.unwrap_err().to_string();
        assert!(
            err_str.contains("invalid_client") || err_str.contains("invalid_grant"),
            "error must mention invalid_client or invalid_grant, got: {err_str}",
        );

        eprintln!("Invalid secret correctly rejected: {err_str}");
        Ok(())
    }
}

// ─── Username-Password Flow Tests ──────────────────────────────────────────

#[cfg(feature = "username_password")]
mod username_password_auth_tests {
    use super::*;
    use force::auth::{Authenticator, UsernamePassword};

    fn load_up_flow() -> Option<UsernamePassword> {
        let client_id = env_string("SF_UP_CLIENT_ID")?;
        let client_secret = env_string("SF_UP_CLIENT_SECRET")?;
        let username = env_string("SF_UP_USERNAME")?;
        let password = env_string("SF_UP_PASSWORD")?;
        let security_token = env_string("SF_UP_SECURITY_TOKEN").unwrap_or_default();
        let token_url = env_string("SF_UP_TOKEN_URL")
            .unwrap_or_else(|| PRODUCTION_TOKEN_URL.to_string());

        Some(UsernamePassword::new(
            client_id,
            client_secret,
            username,
            password,
            security_token,
            token_url,
        ))
    }

    #[tokio::test]
    #[ignore = "requires live Salesforce org with Username-Password flow configured"]
    async fn live_username_password_authenticate_returns_valid_token() -> Result<()> {
        let Some(flow) = load_up_flow() else {
            eprintln!("skipping: missing SF_UP_* env vars");
            return Ok(());
        };

        let token = tokio::time::timeout(Duration::from_secs(30), flow.authenticate())
            .await
            .map_err(|_| HttpError::Timeout { timeout_seconds: 30 })??;

        assert!(!token.as_str().is_empty(), "access token must not be empty");
        assert!(
            token.instance_url().starts_with("https://"),
            "instance_url must start with https://, got: {}",
            token.instance_url(),
        );
        assert_eq!(token.token_type(), "Bearer", "token_type must be Bearer");
        assert!(!token.is_expired(), "token must not be expired immediately after authenticate()");

        eprintln!("Username-Password auth succeeded — instance_url: {}", token.instance_url());
        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires live Salesforce org with Username-Password flow configured"]
    async fn live_username_password_refresh_returns_fresh_token() -> Result<()> {
        let Some(flow) = load_up_flow() else {
            eprintln!("skipping: missing SF_UP_* env vars");
            return Ok(());
        };

        let token1 = tokio::time::timeout(Duration::from_secs(30), flow.authenticate())
            .await
            .map_err(|_| HttpError::Timeout { timeout_seconds: 30 })??;

        let token2 = tokio::time::timeout(Duration::from_secs(30), flow.refresh())
            .await
            .map_err(|_| HttpError::Timeout { timeout_seconds: 30 })??;

        assert!(!token1.as_str().is_empty());
        assert!(!token2.as_str().is_empty());
        assert_eq!(
            token1.instance_url(),
            token2.instance_url(),
            "refresh must return a token for the same instance",
        );

        eprintln!(
            "Username-Password refresh succeeded — both tokens valid for {}",
            token1.instance_url(),
        );
        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires live Salesforce org with Username-Password flow configured"]
    async fn live_username_password_invalid_password_fails() -> Result<()> {
        let Some(_) = env_string("SF_UP_CLIENT_ID") else {
            eprintln!("skipping: missing SF_UP_CLIENT_ID");
            return Ok(());
        };

        let client_id = env_string("SF_UP_CLIENT_ID").unwrap();
        let client_secret = env_string("SF_UP_CLIENT_SECRET").unwrap_or_default();
        let username = env_string("SF_UP_USERNAME").unwrap_or_else(|| "user@test.com".to_string());
        let token_url = env_string("SF_UP_TOKEN_URL")
            .unwrap_or_else(|| PRODUCTION_TOKEN_URL.to_string());

        let bad_flow = UsernamePassword::new(
            client_id,
            client_secret,
            username,
            "INVALID_PASSWORD",
            "",
            token_url,
        );

        let result = tokio::time::timeout(Duration::from_secs(30), bad_flow.authenticate())
            .await
            .map_err(|_| HttpError::Timeout { timeout_seconds: 30 })?;

        assert!(result.is_err(), "authenticate with an invalid password must fail");
        let err_str = result.unwrap_err().to_string();
        assert!(
            err_str.contains("invalid_grant")
                || err_str.contains("invalid_client")
                || err_str.contains("authentication_failure"),
            "error must mention invalid_grant, invalid_client, or authentication_failure, got: {err_str}",
        );

        eprintln!("Invalid password correctly rejected: {err_str}");
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  API SURFACE TESTS  (use whichever auth flow is available)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
struct LiveAccountRow {
    #[serde(rename = "Id")]
    id: String,
}

#[tokio::test]
#[ignore = "requires a live Salesforce org"]
async fn live_rest_query_smoke() -> Result<()> {
    let Some(config) = load_live_config() else {
        eprintln!("skipping live_rest_query_smoke: no credentials available");
        return Ok(());
    };

    eprintln!("using auth: {}", config.auth);

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

#[tokio::test]
#[ignore = "requires a live Salesforce org"]
async fn live_bulk_query_stream_smoke() -> Result<()> {
    let Some(config) = load_live_config() else {
        eprintln!("skipping live_bulk_query_stream_smoke: no credentials available");
        return Ok(());
    };

    eprintln!("using auth: {}", config.auth);

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
#[ignore = "requires a live Salesforce org"]
async fn live_rest_query_malformed_soql_error_payload() -> Result<()> {
    let Some(config) = load_live_config() else {
        eprintln!("skipping: no credentials available");
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
#[ignore = "requires a live Salesforce org"]
async fn live_rest_query_invalid_locator_error_payload() -> Result<()> {
    let Some(config) = load_live_config() else {
        eprintln!("skipping: no credentials available");
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
#[ignore = "requires a live Salesforce org"]
async fn live_bulk_ingest_partial_failure_results() -> Result<()> {
    if !env_flag("SF_LIVE_RUN_PARTIAL_FAILURE") {
        eprintln!(
            "skipping live_bulk_ingest_partial_failure_results: set SF_LIVE_RUN_PARTIAL_FAILURE=1 to enable"
        );
        return Ok(());
    }

    let Some(config) = load_live_config() else {
        eprintln!("skipping: no credentials available");
        return Ok(());
    };

    let result = tokio::time::timeout(config.runtime.test_timeout, async {
        let client = create_live_client(&config).await?;
        let handler = client.bulk();
        let job = IngestJob::create(&handler, "Account", JobOperation::Insert, None).await?;

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
#[ignore = "requires a live Salesforce org"]
async fn live_rest_throttling_error_payload() -> Result<()> {
    if !env_flag("SF_LIVE_RUN_THROTTLE") {
        eprintln!(
            "skipping live_rest_throttling_error_payload: set SF_LIVE_RUN_THROTTLE=1 to enable"
        );
        return Ok(());
    }

    let Some(config) = load_live_config() else {
        eprintln!("skipping: no credentials available");
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

// ─── Data Cloud Token Exchange Tests ──────────────────────────────────────

#[cfg(feature = "data_cloud")]
mod data_cloud_tests {
    use super::*;
    use force::auth::DataCloudConfig;

    /// Creates a client with Data Cloud configured.
    async fn create_dc_client(config: &LiveConfig) -> Result<ForceClient<LiveAuth>> {
        let client_config = ClientConfig {
            api_version: config.api_version.clone(),
            ..Default::default()
        };

        builder()
            .config(client_config)
            .authenticate(config.auth.clone())
            .with_data_cloud(DataCloudConfig::default())
            .build()
            .await
    }

    /// Exercises the full Data Cloud two-step token exchange against a live org.
    ///
    /// The token exchange performs:
    /// 1. Authenticate with the platform (reuses the configured auth flow)
    /// 2. POST to `/services/a360/token` to exchange for a DC token
    ///
    /// On orgs *with* Data Cloud provisioned this returns a valid DC token and
    /// the query succeeds.  On orgs *without* Data Cloud the exchange endpoint
    /// returns an error — we verify it surfaces as a structured `ForceError`
    /// (not a deserialization panic or silent failure).
    #[tokio::test]
    #[ignore = "requires a live Salesforce org"]
    async fn live_data_cloud_token_exchange_smoke() -> Result<()> {
        let Some(config) = load_live_config() else {
            eprintln!("skipping: no credentials available");
            return Ok(());
        };

        eprintln!("using auth: {} (testing Data Cloud token exchange)", config.auth);

        let result = tokio::time::timeout(config.runtime.test_timeout, async {
            let client = create_dc_client(&config).await?;

            // .data_cloud() is infallible when the builder had .with_data_cloud()
            let dc = client.data_cloud()?;

            // Execute a minimal SQL query — this triggers the full token exchange.
            // On a non-DC org the exchange itself will fail, which is fine.
            dc.query_sql("SELECT 1").await
        })
        .await
        .map_err(|_| HttpError::Timeout {
            timeout_seconds: config.runtime.test_timeout.as_secs(),
        })?;

        match result {
            Ok(response) => {
                // Org has Data Cloud — exchange succeeded!
                eprintln!(
                    "Data Cloud query succeeded — row_count: {:?}, columns: {}",
                    response.row_count,
                    response.metadata.len(),
                );
            }
            Err(ref err) => {
                // Expected on non-DC orgs: the /services/a360/token endpoint
                // returns an error.  Verify it's a structured HTTP/auth error
                // (not a deserialization crash, panic, or unhandled variant).
                let err_str = err.to_string();
                let is_structured = matches!(
                    err,
                    ForceError::Http(HttpError::StatusError { .. })
                        | ForceError::Authentication(_)
                );
                assert!(
                    is_structured,
                    "Data Cloud token exchange error should be a structured HTTP or Auth error, \
                     got: {err:?}",
                );
                eprintln!(
                    "Data Cloud token exchange correctly returned structured error \
                     (org likely not DC-provisioned): {err_str}",
                );
            }
        }

        Ok(())
    }

    /// Verifies that a DC-configured client can still perform regular REST queries.
    ///
    /// This catches regressions where enabling Data Cloud could break the
    /// platform session (e.g., by incorrectly sharing token managers).
    #[tokio::test]
    #[ignore = "requires a live Salesforce org"]
    async fn live_data_cloud_platform_session_unaffected() -> Result<()> {
        let Some(config) = load_live_config() else {
            eprintln!("skipping: no credentials available");
            return Ok(());
        };

        eprintln!("using auth: {} (DC enabled, testing platform REST)", config.auth);

        let result = tokio::time::timeout(config.runtime.test_timeout, async {
            let client = create_dc_client(&config).await?;

            // Platform REST must still work even when DC is configured
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
        eprintln!(
            "Platform REST query succeeded with DC enabled — {} record(s)",
            result.records.len(),
        );
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  UNIT TESTS (config resolution, no network)
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod config_resolution_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_sf_cli_org_display_extracts_credentials() {
        let payload = json!({
            "status": 0,
            "result": {
                "accessToken": "00Dxx!token",
                "instanceUrl": "https://dev-org.my.salesforce.com"
            }
        });

        let Ok(envelope) =
            serde_json::from_str::<SfCliOrgDisplayEnvelope>(&payload.to_string())
        else {
            panic!("expected verbose sf payload");
        };

        assert_eq!(
            envelope.result.access_token.as_deref(),
            Some("00Dxx!token"),
        );
        assert_eq!(
            envelope.result.instance_url.as_deref(),
            Some("https://dev-org.my.salesforce.com"),
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
