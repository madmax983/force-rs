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
//!      Accepts a bare host, base URL, or full OAuth token endpoint.
//!
//! 2. **Client Credentials** — renewable, preferred for server-to-server:
//!    - `SF_CLIENT_ID`, `SF_CLIENT_SECRET`
//!    - `SF_TOKEN_URL` must be set explicitly for the target org/environment.
//!      It may be either the base org URL or the full OAuth token endpoint.
//!
//! 3. **Username-Password** (feature `username_password`) — deprecated by Salesforce:
//!    - `SF_UP_CLIENT_ID`, `SF_UP_CLIENT_SECRET`, `SF_UP_USERNAME`,
//!      `SF_UP_PASSWORD`, `SF_UP_SECURITY_TOKEN`
//!    - optional `SF_UP_TOKEN_URL` (defaults to production)
//!      Accepts a bare host, base URL, or full OAuth token endpoint.
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

//  OAUTH URL CONTRACT TESTS
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

/// Resolves a private key path, trying `CARGO_MANIFEST_DIR` as a fallback
/// when a relative path doesn't exist from the current working directory.
#[cfg(feature = "jwt")]
fn resolve_key_path(raw: &str) -> std::path::PathBuf {
    let path = std::path::PathBuf::from(raw);
    if path.exists() {
        return path;
    }

    if !path.is_relative() {
        return path;
    }

    let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") else {
        return path;
    };

    let from_manifest = std::path::PathBuf::from(&manifest_dir).join(&path);
    if from_manifest.exists() {
        return from_manifest;
    }

    let workspace_root = std::path::PathBuf::from(&manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .map(std::path::Path::to_path_buf);
    if let Some(root) = workspace_root {
        let from_root = root.join(&path);
        if from_root.exists() {
            return from_root;
        }
    }

    path
}

// ─── Credential Loading ────────────────────────────────────────────────────

const PRODUCTION_TOKEN_URL: &str = "https://login.salesforce.com/services/oauth2/token";
const PRODUCTION_LOGIN_URL: &str = "https://login.salesforce.com";

fn invalid_config(field: &str, reason: impl Into<String>) -> ForceError {
    ForceError::Config(force::error::ConfigError::InvalidValue {
        field: field.to_string(),
        reason: reason.into(),
    })
}

fn parse_live_https_url(field: &str, value: &str) -> Result<url::Url> {
    let value = value.trim();
    let candidate = if value.contains("://") {
        value.to_string()
    } else {
        format!("https://{value}")
    };

    let parsed = url::Url::parse(&candidate).map_err(|error| {
        invalid_config(
            field,
            format!("must be a valid HTTPS Salesforce base URL or OAuth token endpoint: {error}"),
        )
    })?;

    if parsed.scheme() != "https" {
        return Err(invalid_config(
            field,
            "must use https for Salesforce OAuth requests",
        ));
    }

    if parsed.host_str().is_none() {
        return Err(invalid_config(field, "must include a Salesforce host"));
    }

    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(invalid_config(
            field,
            "must not include embedded credentials",
        ));
    }

    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(invalid_config(
            field,
            "must not include query parameters or fragments",
        ));
    }

    Ok(parsed)
}

fn canonical_base_url(mut parsed: url::Url) -> String {
    parsed.set_path("");
    parsed.set_query(None);
    parsed.set_fragment(None);
    parsed.to_string().trim_end_matches('/').to_string()
}

fn normalize_oauth_token_url(field: &str, token_url: &str) -> Result<String> {
    let mut parsed = parse_live_https_url(field, token_url)?;
    let normalized_path = parsed.path().trim_end_matches('/');
    match normalized_path {
        "" | "/services/oauth2/token" => parsed.set_path("/services/oauth2/token"),
        _ => {
            return Err(invalid_config(
                field,
                "must be either a Salesforce base URL like https://MyDomainName.my.salesforce.com or a token endpoint ending in /services/oauth2/token",
            ));
        }
    }

    Ok(parsed.to_string())
}

fn normalize_client_credentials_token_url(token_url: &str) -> Result<String> {
    normalize_oauth_token_url("SF_TOKEN_URL", token_url)
}

#[cfg(feature = "username_password")]
fn normalize_username_password_token_url(token_url: &str) -> Result<String> {
    normalize_oauth_token_url("SF_UP_TOKEN_URL", token_url)
}

#[cfg(feature = "jwt")]
#[derive(Debug, Clone, PartialEq, Eq)]
struct JwtEndpointConfig {
    audience: String,
    token_url: String,
}

#[cfg(feature = "jwt")]
fn normalize_jwt_login_url(login_url: &str) -> Result<JwtEndpointConfig> {
    let parsed = parse_live_https_url("SF_JWT_LOGIN_URL", login_url)?;
    let normalized_path = parsed.path().trim_end_matches('/');
    match normalized_path {
        "" | "/services/oauth2/token" => {
            let audience = canonical_base_url(parsed);
            let token_url = format!("{audience}/services/oauth2/token");
            Ok(JwtEndpointConfig {
                audience,
                token_url,
            })
        }
        _ => Err(invalid_config(
            "SF_JWT_LOGIN_URL",
            "must be either a Salesforce base URL like https://MyDomainName.my.salesforce.com or a token endpoint ending in /services/oauth2/token",
        )),
    }
}

fn validate_client_credentials_token_url(token_url: &str) -> Result<()> {
    normalize_client_credentials_token_url(token_url).map(|_| ())
}

fn require_client_credentials_token_url() -> String {
    let token_url = env_string("SF_TOKEN_URL").unwrap_or_else(|| {
        panic!(
            "{}",
            force::error::ConfigError::MissingValue(
                "SF_TOKEN_URL is required when SF_CLIENT_ID/SF_CLIENT_SECRET are configured; set it to the target org base URL or OAuth token endpoint, for example https://MyDomainName.my.salesforce.com"
                    .to_string(),
            )
        )
    });
    normalize_client_credentials_token_url(&token_url).unwrap_or_else(|error| panic!("{error}"))
}

#[cfg(feature = "jwt")]
fn load_jwt_endpoint_config() -> JwtEndpointConfig {
    let login_url =
        env_string("SF_JWT_LOGIN_URL").unwrap_or_else(|| PRODUCTION_LOGIN_URL.to_string());
    normalize_jwt_login_url(&login_url).unwrap_or_else(|error| panic!("{error}"))
}

#[cfg(feature = "username_password")]
fn load_username_password_token_url() -> String {
    let token_url =
        env_string("SF_UP_TOKEN_URL").unwrap_or_else(|| PRODUCTION_TOKEN_URL.to_string());
    normalize_username_password_token_url(&token_url).unwrap_or_else(|error| panic!("{error}"))
}

/// Tries to build a JWT Bearer authenticator from env vars.
#[cfg(feature = "jwt")]
fn try_jwt_auth() -> Option<LiveAuth> {
    let client_id = env_string("SF_JWT_CLIENT_ID")?;
    let username = env_string("SF_JWT_USERNAME")?;
    let private_key_path = env_string("SF_JWT_PRIVATE_KEY_PATH")?;

    let resolved = resolve_key_path(&private_key_path);
    let private_key_pem = std::fs::read_to_string(&resolved)
        .unwrap_or_else(|e| panic!("Failed to read private key at {}: {e}", resolved.display()));

    let endpoints = load_jwt_endpoint_config();

    let flow = JwtBearerFlow::new(
        &client_id,
        &username,
        &private_key_pem,
        endpoints.audience.as_str(),
        endpoints.token_url.as_str(),
    )
    .unwrap_or_else(|e| panic!("Invalid JWT config: {e}"));

    Some(LiveAuth::Jwt(flow))
}

/// Tries to build a Client Credentials authenticator from env vars.
fn try_client_credentials_auth() -> Option<LiveAuth> {
    let client_id = env_string("SF_CLIENT_ID")?;
    let client_secret = env_string("SF_CLIENT_SECRET")?;
    let token_url = require_client_credentials_token_url();

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
    let token_url = load_username_password_token_url();

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

#[test]
fn client_credentials_contract_rejects_non_token_endpoint() {
    let result = validate_client_credentials_token_url(
        "https://example.my.salesforce.com/services/oauth2/authorize",
    );

    let Err(err) = result else {
        panic!("client credentials must reject non-token OAuth endpoints");
    };

    let message = err.to_string();
    assert!(
        message.contains("SF_TOKEN_URL") && message.contains("/services/oauth2/token"),
        "expected token endpoint guidance in error, got: {message}",
    );
}

#[test]
fn client_credentials_contract_accepts_explicit_token_endpoints() {
    let result = validate_client_credentials_token_url(PRODUCTION_TOKEN_URL);
    assert!(
        result.is_ok(),
        "client credentials should accept explicit Salesforce token endpoints: {result:?}",
    );

    let result = validate_client_credentials_token_url(
        "https://example.my.salesforce.com/services/oauth2/token",
    );

    assert!(
        result.is_ok(),
        "client credentials should accept My Domain token endpoints: {result:?}",
    );
}

#[test]
fn client_credentials_contract_normalizes_my_domain_base_urls() {
    let token_url = normalize_client_credentials_token_url("example.my.salesforce.com")
        .unwrap_or_else(|error| panic!("bare My Domain host should normalize: {error}"));

    assert_eq!(
        token_url,
        "https://example.my.salesforce.com/services/oauth2/token",
    );

    let token_url = normalize_client_credentials_token_url("https://example.my.salesforce.com")
        .unwrap_or_else(|error| panic!("base My Domain URL should normalize: {error}"));

    assert_eq!(
        token_url,
        "https://example.my.salesforce.com/services/oauth2/token",
    );

    let token_url = normalize_client_credentials_token_url("https://example.my.salesforce.com/")
        .unwrap_or_else(|error| panic!("trailing-slash My Domain URL should normalize: {error}"));

    assert_eq!(
        token_url,
        "https://example.my.salesforce.com/services/oauth2/token",
    );
}

// ═══════════════════════════════════════════════════════════════════════════
#[cfg(feature = "jwt")]
#[test]
fn jwt_login_url_contract_normalizes_bare_hosts_and_token_endpoints() {
    let endpoints = normalize_jwt_login_url("example.my.salesforce.com")
        .unwrap_or_else(|error| panic!("bare JWT login host should normalize: {error}"));

    assert_eq!(endpoints.audience, "https://example.my.salesforce.com");
    assert_eq!(
        endpoints.token_url,
        "https://example.my.salesforce.com/services/oauth2/token",
    );

    let endpoints =
        normalize_jwt_login_url("https://example.my.salesforce.com/services/oauth2/token")
            .unwrap_or_else(|error| panic!("JWT token endpoint should normalize: {error}"));

    assert_eq!(endpoints.audience, "https://example.my.salesforce.com");
    assert_eq!(
        endpoints.token_url,
        "https://example.my.salesforce.com/services/oauth2/token",
    );
}

#[cfg(feature = "username_password")]
#[test]
fn username_password_contract_normalizes_base_token_urls() {
    let token_url = normalize_username_password_token_url("example.my.salesforce.com")
        .unwrap_or_else(|error| panic!("bare username-password host should normalize: {error}"));

    assert_eq!(
        token_url,
        "https://example.my.salesforce.com/services/oauth2/token",
    );

    let token_url = normalize_username_password_token_url("https://example.my.salesforce.com/")
        .unwrap_or_else(|error| panic!("base username-password URL should normalize: {error}"));

    assert_eq!(
        token_url,
        "https://example.my.salesforce.com/services/oauth2/token",
    );
}

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
        let private_key_pem = std::fs::read_to_string(&resolved).unwrap_or_else(|e| {
            panic!("Failed to read private key at {}: {e}", resolved.display())
        });

        let endpoints = load_jwt_endpoint_config();

        Some(
            JwtBearerFlow::new(
                &client_id,
                &username,
                &private_key_pem,
                endpoints.audience.as_str(),
                endpoints.token_url.as_str(),
            )
            .unwrap_or_else(|e| panic!("Invalid JWT config: {e}")),
        )
    }

    #[tokio::test]
    #[ignore = "requires live Salesforce org with JWT Bearer Flow configured"]
    async fn live_jwt_bearer_authenticate_returns_valid_token() -> Result<()> {
        let Some(flow) = load_jwt_flow() else {
            eprintln!(
                "skipping: missing SF_JWT_CLIENT_ID, SF_JWT_USERNAME, or SF_JWT_PRIVATE_KEY_PATH"
            );
            return Ok(());
        };

        let token = tokio::time::timeout(Duration::from_secs(30), flow.authenticate())
            .await
            .map_err(|_| HttpError::Timeout {
                timeout_seconds: 30,
            })??;

        assert!(!token.as_str().is_empty(), "access token must not be empty");
        assert!(
            token.instance_url().starts_with("https://"),
            "instance_url must start with https://, got: {}",
            token.instance_url(),
        );
        assert_eq!(token.token_type(), "Bearer", "token_type must be Bearer");
        assert!(
            !token.is_expired(),
            "token must not be expired immediately after authenticate()"
        );

        eprintln!(
            "JWT auth succeeded — instance_url: {}",
            token.instance_url()
        );
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
            .map_err(|_| HttpError::Timeout {
                timeout_seconds: 30,
            })??;

        let token2 = tokio::time::timeout(Duration::from_secs(30), flow.refresh())
            .await
            .map_err(|_| HttpError::Timeout {
                timeout_seconds: 30,
            })??;

        assert!(!token1.as_str().is_empty());
        assert!(!token2.as_str().is_empty());
        assert_eq!(
            token1.instance_url(),
            token2.instance_url(),
            "refresh must return a token for the same instance",
        );

        eprintln!(
            "JWT refresh succeeded — both tokens valid for {}",
            token1.instance_url()
        );
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
        let Some(private_key_path) = env_string("SF_JWT_PRIVATE_KEY_PATH") else {
            eprintln!("skipping: missing SF_JWT_PRIVATE_KEY_PATH");
            return Ok(());
        };
        let resolved = resolve_key_path(&private_key_path);
        let pem = std::fs::read_to_string(&resolved)?;
        let endpoints = load_jwt_endpoint_config();
        drop(flow);

        let bad_flow = JwtBearerFlow::new(
            "INVALID_CLIENT_ID",
            "user@test.com",
            &pem,
            endpoints.audience.as_str(),
            endpoints.token_url.as_str(),
        )?;

        let result = tokio::time::timeout(Duration::from_secs(30), bad_flow.authenticate())
            .await
            .map_err(|_| HttpError::Timeout {
                timeout_seconds: 30,
            })?;

        let Err(err) = result else {
            panic!("authenticate with an invalid client_id must fail");
        };
        let err_str = err.to_string();
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
        let token_url = require_client_credentials_token_url();
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
            .map_err(|_| HttpError::Timeout {
                timeout_seconds: 30,
            })??;

        assert!(!token.as_str().is_empty(), "access token must not be empty");
        assert!(
            token.instance_url().starts_with("https://"),
            "instance_url must start with https://, got: {}",
            token.instance_url(),
        );
        assert_eq!(token.token_type(), "Bearer", "token_type must be Bearer");
        assert!(
            !token.is_expired(),
            "token must not be expired immediately after authenticate()"
        );

        eprintln!(
            "Client Credentials auth succeeded — instance_url: {}",
            token.instance_url()
        );
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
            .map_err(|_| HttpError::Timeout {
                timeout_seconds: 30,
            })??;

        let token2 = tokio::time::timeout(Duration::from_secs(30), flow.refresh())
            .await
            .map_err(|_| HttpError::Timeout {
                timeout_seconds: 30,
            })??;

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
        let Some(client_id) = env_string("SF_CLIENT_ID") else {
            eprintln!("skipping: missing SF_CLIENT_ID");
            return Ok(());
        };

        let token_url = require_client_credentials_token_url();

        let bad_flow = ClientCredentials::new(client_id, "INVALID_SECRET", token_url);

        let result = tokio::time::timeout(Duration::from_secs(30), bad_flow.authenticate())
            .await
            .map_err(|_| HttpError::Timeout {
                timeout_seconds: 30,
            })?;

        let Err(err) = result else {
            panic!("authenticate with an invalid secret must fail");
        };
        let err_str = err.to_string();
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
        let token_url = load_username_password_token_url();

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
            .map_err(|_| HttpError::Timeout {
                timeout_seconds: 30,
            })??;

        assert!(!token.as_str().is_empty(), "access token must not be empty");
        assert!(
            token.instance_url().starts_with("https://"),
            "instance_url must start with https://, got: {}",
            token.instance_url(),
        );
        assert_eq!(token.token_type(), "Bearer", "token_type must be Bearer");
        assert!(
            !token.is_expired(),
            "token must not be expired immediately after authenticate()"
        );

        eprintln!(
            "Username-Password auth succeeded — instance_url: {}",
            token.instance_url()
        );
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
            .map_err(|_| HttpError::Timeout {
                timeout_seconds: 30,
            })??;

        let token2 = tokio::time::timeout(Duration::from_secs(30), flow.refresh())
            .await
            .map_err(|_| HttpError::Timeout {
                timeout_seconds: 30,
            })??;

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
        let Some(client_id) = env_string("SF_UP_CLIENT_ID") else {
            eprintln!("skipping: missing SF_UP_CLIENT_ID");
            return Ok(());
        };

        let client_secret = env_string("SF_UP_CLIENT_SECRET").unwrap_or_default();
        let username = env_string("SF_UP_USERNAME").unwrap_or_else(|| "user@test.com".to_string());
        let token_url = load_username_password_token_url();

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
            .map_err(|_| HttpError::Timeout {
                timeout_seconds: 30,
            })?;

        let Err(err) = result else {
            panic!("authenticate with an invalid password must fail");
        };
        let err_str = err.to_string();
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

#[cfg(feature = "tooling")]
#[tokio::test]
#[ignore = "requires a live Salesforce org"]
async fn live_tooling_query_smoke() -> Result<()> {
    let Some(config) = load_live_config() else {
        eprintln!("skipping live_tooling_query_smoke: no credentials available");
        return Ok(());
    };

    eprintln!("using auth: {} (testing Tooling API)", config.auth);

    let result = tokio::time::timeout(config.runtime.test_timeout, async {
        let client = create_live_client(&config).await?;
        client
            .tooling()
            .query::<serde_json::Value>("SELECT Id FROM ApexClass LIMIT 1")
            .await
    })
    .await
    .map_err(|_| HttpError::Timeout {
        timeout_seconds: config.runtime.test_timeout.as_secs(),
    })??;

    assert!(result.records.len() <= 1);
    Ok(())
}

#[cfg(feature = "composite")]
#[tokio::test]
#[ignore = "requires a live Salesforce org"]
async fn live_composite_batch_smoke() -> Result<()> {
    let Some(config) = load_live_config() else {
        eprintln!("skipping live_composite_batch_smoke: no credentials available");
        return Ok(());
    };

    eprintln!("using auth: {} (testing Composite Batch)", config.auth);

    let result = tokio::time::timeout(config.runtime.test_timeout, async {
        let client = create_live_client(&config).await?;
        client
            .composite()
            .batch()
            .add_request("GET", "limits", None)?
            .add_request("GET", "sobjects/Account/describe", None)?
            .execute()
            .await
    })
    .await
    .map_err(|_| HttpError::Timeout {
        timeout_seconds: config.runtime.test_timeout.as_secs(),
    })??;

    assert!(
        !result.has_errors,
        "Composite batch returned errors: {result:?}"
    );
    assert_eq!(result.results.len(), 2);
    for subresponse in &result.results {
        assert_eq!(subresponse.status_code, 200);
        assert!(subresponse.result.is_some());
    }
    Ok(())
}

#[cfg(feature = "ui")]
#[tokio::test]
#[ignore = "requires a live Salesforce org"]
async fn live_ui_object_info_smoke() -> Result<()> {
    let Some(config) = load_live_config() else {
        eprintln!("skipping live_ui_object_info_smoke: no credentials available");
        return Ok(());
    };

    eprintln!("using auth: {} (testing UI API)", config.auth);

    let object_info = tokio::time::timeout(config.runtime.test_timeout, async {
        let client = create_live_client(&config).await?;
        client.ui().object_info("Account").await
    })
    .await
    .map_err(|_| HttpError::Timeout {
        timeout_seconds: config.runtime.test_timeout.as_secs(),
    })??;

    assert_eq!(object_info.api_name, "Account");
    assert!(object_info.fields.contains_key("Id"));
    assert!(object_info.fields.contains_key("Name"));
    Ok(())
}

#[cfg(feature = "graphql")]
#[tokio::test]
#[ignore = "requires a live Salesforce org"]
async fn live_graphql_query_smoke() -> Result<()> {
    let Some(config) = load_live_config() else {
        eprintln!("skipping live_graphql_query_smoke: no credentials available");
        return Ok(());
    };

    eprintln!("using auth: {} (testing GraphQL API)", config.auth);

    let data = tokio::time::timeout(config.runtime.test_timeout, async {
        let client = create_live_client(&config).await?;
        client
            .graphql()
            .query_raw(
                r"{
                    uiapi {
                        query {
                            Account(first: 1) {
                                edges {
                                    node {
                                        Id
                                    }
                                }
                                totalCount
                            }
                        }
                    }
                }",
                None,
            )
            .await
    })
    .await
    .map_err(|_| HttpError::Timeout {
        timeout_seconds: config.runtime.test_timeout.as_secs(),
    })??;

    let account = &data["uiapi"]["query"]["Account"];
    assert!(
        account.is_object(),
        "expected Account GraphQL object, got {account:?}",
    );
    assert!(
        account["edges"].is_array(),
        "expected Account edges array, got {account:?}",
    );
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

// ─── Bulk Round-Trip Tests (SmartIngest, Convenience Methods, DataFaker) ──

/// Self-cleaning bulk tests that exercise `SmartIngest`, convenience methods,
/// and the datafaker against a live org. Every record created is deleted
/// before the test returns.
///
/// Gated behind `SF_LIVE_RUN_BULK_ROUNDTRIP=1` because they mutate org data.
mod bulk_roundtrip_tests {
    use super::*;
    use futures::stream;
    use serde::{Deserialize, Serialize};

    /// Minimal Account for bulk insert (CSV-friendly flat struct).
    #[derive(Debug, Clone, Serialize)]
    struct NewAccount {
        #[serde(rename = "Name")]
        name: String,
    }

    /// Account row returned from bulk query (needs Id for cleanup).
    #[derive(Debug, Clone, Deserialize)]
    struct AccountRow {
        #[serde(rename = "Id")]
        id: String,
        #[serde(rename = "Name")]
        name: String,
    }

    /// Generates N accounts with a unique prefix for this test run.
    fn generate_accounts(prefix: &str, count: usize) -> Vec<NewAccount> {
        (0..count)
            .map(|i| NewAccount {
                name: format!("{prefix}_{i:04}"),
            })
            .collect()
    }

    fn record_count_as_usize(value: Option<i64>, field: &str) -> Result<usize> {
        let count = value.unwrap_or(0);
        usize::try_from(count)
            .map_err(|_| ForceError::InvalidInput(format!("{field} is out of range: {count}")))
    }

    /// Deletes all Account records matching a Name prefix via bulk delete.
    /// Returns the number of records deleted.
    async fn cleanup_accounts(
        client: &force::client::ForceClient<LiveAuth>,
        prefix: &str,
        policy: force::api::bulk::BulkPollPolicy,
    ) -> Result<usize> {
        // Query for IDs matching the prefix
        let soql = format!("SELECT Id FROM Account WHERE Name LIKE '{prefix}%'");
        let result = client
            .rest()
            .query::<force::types::DynamicSObject>(&soql)
            .await?;

        if result.records.is_empty() {
            return Ok(0);
        }

        let ids: Vec<String> = result
            .records
            .iter()
            .filter_map(|r| r.get_field("Id").and_then(|v| v.as_str().map(String::from)))
            .collect();

        let count = ids.len();
        if count > 0 {
            let job = force::api::bulk::IngestJob::create(
                &client.bulk(),
                "Account",
                JobOperation::Delete,
                None,
            )
            .await?;

            let csv = ids.iter().fold("Id\n".to_string(), |mut acc, id| {
                acc.push_str(id);
                acc.push('\n');
                acc
            });

            let job = job.upload(csv).await?;
            let job = job.close().await?;
            let _job = job.poll_until_complete_with_policy(policy).await?;
        }

        Ok(count)
    }

    // ── SmartIngest round-trip ───────────────────────────────────────────

    #[tokio::test]
    #[ignore = "requires a live Salesforce org"]
    async fn live_smart_ingest_round_trip() -> Result<()> {
        if !env_flag("SF_LIVE_RUN_BULK_ROUNDTRIP") {
            eprintln!(
                "skipping live_smart_ingest_round_trip: set SF_LIVE_RUN_BULK_ROUNDTRIP=1 to enable"
            );
            return Ok(());
        }

        let Some(config) = load_live_config() else {
            eprintln!("skipping: no credentials available");
            return Ok(());
        };

        let prefix = format!("FRSSI_{}", chrono::Utc::now().timestamp_millis());

        // Use a small local buffer and a tiny upload ceiling so the test
        // exercises SmartIngest's multi-job logic in a live context.
        let record_count = 150;
        let batch_size = 50;
        let max_upload_bytes = 1_500;

        eprintln!(
            "using auth: {} | prefix={prefix} | records={record_count} | batch_size={batch_size} | max_upload_bytes={max_upload_bytes}",
            config.auth,
        );

        let result: force::error::Result<()> =
            tokio::time::timeout(config.runtime.test_timeout, async {
                let client = create_live_client(&config).await?;
                let accounts = generate_accounts(&prefix, record_count);

                // ── INSERT via SmartIngest ──────────────────────────────
                let record_stream = stream::iter(accounts);

                let ingest_result = client
                    .bulk()
                    .smart_ingest("Account", JobOperation::Insert)
                    .batch_size(batch_size)
                    .max_upload_bytes(max_upload_bytes)
                    .execute_stream(record_stream)
                    .await?;

                assert!(
                    ingest_result.job_count() > 1,
                    "SmartIngest should split this payload across multiple jobs",
                );

                assert_eq!(
                    record_count_as_usize(
                        Some(ingest_result.total_records_processed()),
                        "total_records_processed"
                    )?,
                    record_count,
                    "SmartIngest should process all {record_count} records",
                );
                assert_eq!(
                    ingest_result.total_records_failed(),
                    0,
                    "SmartIngest should have 0 failures",
                );
                eprintln!(
                    "SmartIngest inserted {record_count} records across {} job(s) in {} ms",
                    ingest_result.job_count(),
                    ingest_result.total_processing_time(),
                );

                // ── QUERY back via Bulk Query ──────────────────────────
                let soql = format!(
                    "SELECT Id, Name FROM Account WHERE Name LIKE '{prefix}%' ORDER BY Name"
                );
                let mut query_stream = client
                    .bulk()
                    .bulk_query_with_policy::<AccountRow>(&soql, config.runtime.bulk_poll_policy)
                    .await?;

                let mut queried = Vec::new();
                while let Some(row) = query_stream.next().await? {
                    queried.push(row);
                }

                assert_eq!(
                    queried.len(),
                    record_count,
                    "Bulk query should return all {record_count} inserted records",
                );

                // Spot-check: first and last names should match our pattern
                assert!(
                    queried[0].name.starts_with(&prefix),
                    "First record name should start with prefix",
                );
                assert!(
                    queried.iter().all(|row| !row.id.is_empty()),
                    "Bulk query should return non-empty Account IDs",
                );

                // ── CLEANUP via bulk delete ─────────────────────────────
                let deleted =
                    cleanup_accounts(&client, &prefix, config.runtime.bulk_poll_policy).await?;
                eprintln!("Cleaned up {deleted} records");
                assert_eq!(deleted, record_count);

                Ok(())
            })
            .await
            .map_err(|_| HttpError::Timeout {
                timeout_seconds: config.runtime.test_timeout.as_secs(),
            })?;

        // If the test body failed, still try cleanup (best-effort)
        if result.is_err()
            && let Some(config) = load_live_config()
            && let Ok(client) = create_live_client(&config).await
        {
            let _ = cleanup_accounts(&client, &prefix, config.runtime.bulk_poll_policy).await;
        }

        result
    }

    // ── Convenience method round-trip (insert + delete) ─────────────────

    #[tokio::test]
    #[ignore = "requires a live Salesforce org"]
    async fn live_bulk_insert_delete_convenience() -> Result<()> {
        if !env_flag("SF_LIVE_RUN_BULK_ROUNDTRIP") {
            eprintln!(
                "skipping live_bulk_insert_delete_convenience: set SF_LIVE_RUN_BULK_ROUNDTRIP=1 to enable"
            );
            return Ok(());
        }

        let Some(config) = load_live_config() else {
            eprintln!("skipping: no credentials available");
            return Ok(());
        };

        let prefix = format!("FRSCONV_{}", chrono::Utc::now().timestamp_millis());
        let record_count = 10;

        eprintln!(
            "using auth: {} | prefix={prefix} | records={record_count}",
            config.auth,
        );

        let result: force::error::Result<()> =
            tokio::time::timeout(config.runtime.test_timeout, async {
                let client = create_live_client(&config).await?;
                let accounts = generate_accounts(&prefix, record_count);

                // ── INSERT via convenience method ──────────────────────
                let job_info = client.bulk().insert("Account", &accounts).await?;

                assert_eq!(
                    record_count_as_usize(
                        job_info.number_records_processed,
                        "number_records_processed"
                    )?,
                    record_count,
                );
                assert_eq!(job_info.number_records_failed.unwrap_or(-1), 0);
                eprintln!("bulk insert completed: {record_count} records");

                // ── Query IDs for deletion ─────────────────────────────
                let soql = format!("SELECT Id FROM Account WHERE Name LIKE '{prefix}%'");
                let result = client
                    .rest()
                    .query::<force::types::DynamicSObject>(&soql)
                    .await?;

                let ids: Vec<String> = result
                    .records
                    .iter()
                    .filter_map(|r| r.get_field("Id").and_then(|v| v.as_str().map(String::from)))
                    .collect();

                assert_eq!(
                    ids.len(),
                    record_count,
                    "Should find all {record_count} inserted records",
                );

                // ── DELETE via convenience method ──────────────────────
                let del_info = client.bulk().delete("Account", &ids).await?;

                assert_eq!(
                    record_count_as_usize(
                        del_info.number_records_processed,
                        "number_records_processed"
                    )?,
                    record_count,
                );
                assert_eq!(del_info.number_records_failed.unwrap_or(-1), 0);
                eprintln!("bulk delete completed: {record_count} records");

                Ok(())
            })
            .await
            .map_err(|_| HttpError::Timeout {
                timeout_seconds: config.runtime.test_timeout.as_secs(),
            })?;

        if result.is_err()
            && let Some(config) = load_live_config()
            && let Ok(client) = create_live_client(&config).await
        {
            let _ = cleanup_accounts(&client, &prefix, config.runtime.bulk_poll_policy).await;
        }

        result
    }

    // ── DataFaker → REST create → verify → delete ───────────────────────

    #[cfg(feature = "data_utility")]
    #[tokio::test]
    #[ignore = "requires a live Salesforce org"]
    async fn live_datafaker_rest_round_trip() -> Result<()> {
        if !env_flag("SF_LIVE_RUN_BULK_ROUNDTRIP") {
            eprintln!(
                "skipping live_datafaker_rest_round_trip: set SF_LIVE_RUN_BULK_ROUNDTRIP=1 to enable"
            );
            return Ok(());
        }

        let Some(config) = load_live_config() else {
            eprintln!("skipping: no credentials available");
            return Ok(());
        };

        eprintln!(
            "using auth: {} (testing datafaker → REST round-trip)",
            config.auth,
        );

        tokio::time::timeout(config.runtime.test_timeout, async {
            let client = create_live_client(&config).await?;

            // ── Describe Account to feed the datafaker ─────────────
            let describe = client.rest().describe("Account").await?;
            eprintln!(
                "Account describe: {} fields, {} createable",
                describe.fields.len(),
                describe.fields.iter().filter(|f| f.createable).count(),
            );

            // ── Generate a mock record ─────────────────────────────
            let mock_record = force::data::generate_mock_record(&describe);

            // The mock record has fields populated — serialize to JSON
            // for REST create (strip the dummy attributes/Id).
            let mut payload = serde_json::to_value(&mock_record)
                .map_err(|e| ForceError::Serialization(e.into()))?;
            if let Some(obj) = payload.as_object_mut() {
                obj.remove("attributes");
                obj.remove("Id");
            }

            eprintln!(
                "datafaker generated payload with {} fields",
                payload.as_object().map_or(0, serde_json::Map::len),
            );

            // ── Create via REST ────────────────────────────────────
            let create_result = client.rest().create("Account", &payload).await;

            match create_result {
                Ok(response) => {
                    let Some(sf_id) = response.id else {
                        return Err(ForceError::InvalidInput(
                            "successful create should return an Id".to_string(),
                        ));
                    };
                    eprintln!("datafaker record created: {sf_id}");

                    // ── Read it back ───────────────────────────────
                    let fetched: serde_json::Value = client.rest().get("Account", &sf_id).await?;

                    // The Name field should have been set by the datafaker
                    let name = fetched.get("Name").and_then(|v| v.as_str()).unwrap_or("");
                    assert!(
                        !name.is_empty(),
                        "Fetched record should have a Name field set by datafaker",
                    );
                    eprintln!("read-back confirmed — Name: {name}");

                    // ── Generate a mock SOQL query and run it ──────
                    let mock_query = force::data::generate_mock_query(&describe);
                    eprintln!("datafaker SOQL: {mock_query}");
                    let query_result = client
                        .rest()
                        .query::<force::types::DynamicSObject>(&mock_query)
                        .await?;
                    eprintln!(
                        "datafaker query returned {} record(s)",
                        query_result.records.len(),
                    );

                    // ── Cleanup ────────────────────────────────────
                    client.rest().delete("Account", &sf_id).await?;
                    eprintln!("datafaker record deleted");
                }
                Err(e) => {
                    // Some orgs have validation rules that reject mock data
                    // (required fields, picklist restrictions, etc.).
                    // That's OK — the point is the datafaker ran and produced
                    // a plausible payload; the org just rejected it.
                    let err_str = e.to_string();
                    eprintln!("datafaker record rejected by org (validation rules?): {err_str}");
                    assert!(
                        matches!(
                            e,
                            ForceError::Http(HttpError::StatusError {
                                status_code: 400,
                                ..
                            })
                        ),
                        "Rejection should be a 400 validation error, got: {e:?}",
                    );
                }
            }

            Ok::<(), ForceError>(())
        })
        .await
        .map_err(|_| HttpError::Timeout {
            timeout_seconds: config.runtime.test_timeout.as_secs(),
        })??;

        Ok(())
    }
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

        eprintln!(
            "using auth: {} (testing Data Cloud token exchange)",
            config.auth
        );

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
                    ForceError::Http(HttpError::StatusError { .. }) | ForceError::Authentication(_)
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

        eprintln!(
            "using auth: {} (DC enabled, testing platform REST)",
            config.auth
        );

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

        let Ok(envelope) = serde_json::from_str::<SfCliOrgDisplayEnvelope>(&payload.to_string())
        else {
            panic!("expected verbose sf payload");
        };

        assert_eq!(envelope.result.access_token.as_deref(), Some("00Dxx!token"),);
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
