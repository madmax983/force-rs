#![allow(missing_docs)]
#![allow(clippy::large_enum_variant)]

//! Live Salesforce Pub/Sub contract tests.
//!
//! These tests are ignored by default and require:
//! - one supported Salesforce auth configuration (`SF_JWT_*`, `SF_CLIENT_*`,
//!   `SF_UP_*`, `SF_ACCESS_TOKEN`/`SF_INSTANCE_URL`, or Salesforce CLI)
//! - `SF_PUBSUB_TOPIC`, for example `/data/AccountChangeEvent`
//! - optional `SF_PUBSUB_ENDPOINT` (defaults to Salesforce's global endpoint)

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result};
use async_trait::async_trait;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::{ForceClient, builder};
use force::config::ClientConfig;
use force::error::{ConfigError, ForceError};
use force_pubsub::{PubSubConfig, PubSubHandler};
use serde::Deserialize;

const PRODUCTION_TOKEN_URL: &str = "https://login.salesforce.com/services/oauth2/token";
const PRODUCTION_LOGIN_URL: &str = "https://login.salesforce.com";
const DEFAULT_PUBSUB_ENDPOINT: &str = "https://api.pubsub.salesforce.com:7443";

#[derive(Debug, Clone)]
enum LiveAuth {
    Jwt(force::auth::JwtBearerFlow),
    ClientCredentials(force::auth::ClientCredentials),
    UsernamePassword(force::auth::UsernamePassword),
    Token(EnvAuthenticator),
}

#[async_trait]
impl Authenticator for LiveAuth {
    async fn authenticate(&self) -> force::error::Result<AccessToken> {
        match self {
            Self::Jwt(flow) => flow.authenticate().await,
            Self::ClientCredentials(flow) => flow.authenticate().await,
            Self::UsernamePassword(flow) => flow.authenticate().await,
            Self::Token(env) => env.authenticate().await,
        }
    }

    async fn refresh(&self) -> force::error::Result<AccessToken> {
        match self {
            Self::Jwt(flow) => flow.refresh().await,
            Self::ClientCredentials(flow) => flow.refresh().await,
            Self::UsernamePassword(flow) => flow.refresh().await,
            Self::Token(env) => env.refresh().await,
        }
    }
}

impl std::fmt::Display for LiveAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Jwt(_) => write!(f, "JWT Bearer"),
            Self::ClientCredentials(_) => write!(f, "Client Credentials"),
            Self::UsernamePassword(_) => write!(f, "Username-Password"),
            Self::Token(_) => write!(f, "Access Token"),
        }
    }
}

#[derive(Debug, Clone)]
struct EnvAuthenticator {
    access_token: String,
    instance_url: String,
}

#[async_trait]
impl Authenticator for EnvAuthenticator {
    async fn authenticate(&self) -> force::error::Result<AccessToken> {
        let issued_at = SystemTime::now().duration_since(UNIX_EPOCH).map_or_else(
            |_| "0".to_string(),
            |duration| duration.as_millis().to_string(),
        );

        Ok(AccessToken::from_response(TokenResponse {
            access_token: secrecy::SecretString::new(self.access_token.clone().into()),
            instance_url: self.instance_url.clone(),
            token_type: "Bearer".to_string(),
            issued_at,
            expires_in: Some(7_200),
            refresh_token: None,
            signature: "live-test".to_string(),
        }))
    }

    async fn refresh(&self) -> force::error::Result<AccessToken> {
        self.authenticate().await
    }
}

#[derive(Debug, Clone)]
struct LiveConfig {
    auth: LiveAuth,
    api_version: String,
    test_timeout: Duration,
    pubsub_endpoint: String,
    pubsub_topic: String,
}

fn env_string(key: &str) -> Option<String> {
    std::env::var(key).ok().and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

fn invalid_config(field: &str, reason: impl Into<String>) -> ForceError {
    ForceError::Config(ConfigError::InvalidValue {
        field: field.to_string(),
        reason: reason.into(),
    })
}

fn parse_live_https_url(field: &str, value: &str) -> force::error::Result<url::Url> {
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

fn normalize_oauth_token_url(field: &str, token_url: &str) -> force::error::Result<String> {
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct JwtEndpointConfig {
    audience: String,
    token_url: String,
}

fn normalize_jwt_login_url(login_url: &str) -> force::error::Result<JwtEndpointConfig> {
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

    let workspace_root = std::path::PathBuf::from(&manifest_dir)
        .parent()
        .and_then(std::path::Path::parent)
        .map(std::path::Path::to_path_buf);
    if let Some(root) = workspace_root {
        let from_root = root.join(&path);
        if from_root.exists() {
            return from_root;
        }
    }

    path
}

fn try_jwt_auth() -> Option<LiveAuth> {
    let client_id = env_string("SF_JWT_CLIENT_ID")?;
    let username = env_string("SF_JWT_USERNAME")?;
    let private_key_path = env_string("SF_JWT_PRIVATE_KEY_PATH")?;

    let resolved = resolve_key_path(&private_key_path);
    let private_key_pem = std::fs::read_to_string(&resolved).unwrap_or_else(|error| {
        panic!(
            "Failed to read private key at {}: {error}",
            resolved.display()
        )
    });
    let login_url =
        env_string("SF_JWT_LOGIN_URL").unwrap_or_else(|| PRODUCTION_LOGIN_URL.to_string());
    let endpoints = normalize_jwt_login_url(&login_url).unwrap_or_else(|error| panic!("{error}"));

    let flow = force::auth::JwtBearerFlow::new(
        client_id,
        username,
        private_key_pem,
        endpoints.audience,
        endpoints.token_url,
    )
    .unwrap_or_else(|error| panic!("Invalid JWT config: {error}"));
    Some(LiveAuth::Jwt(flow))
}

fn try_client_credentials_auth() -> Option<LiveAuth> {
    let client_id = env_string("SF_CLIENT_ID")?;
    let client_secret = env_string("SF_CLIENT_SECRET")?;
    let token_url = env_string("SF_TOKEN_URL").unwrap_or_else(|| {
        panic!(
            "{}",
            ConfigError::MissingValue(
                "SF_TOKEN_URL is required when SF_CLIENT_ID/SF_CLIENT_SECRET are configured; set it to the target org base URL or OAuth token endpoint, for example https://MyDomainName.my.salesforce.com"
                    .to_string(),
            )
        )
    });
    let token_url = normalize_oauth_token_url("SF_TOKEN_URL", &token_url)
        .unwrap_or_else(|error| panic!("{error}"));

    Some(LiveAuth::ClientCredentials(
        force::auth::ClientCredentials::new(client_id, client_secret, token_url),
    ))
}

fn try_username_password_auth() -> Option<LiveAuth> {
    let client_id = env_string("SF_UP_CLIENT_ID")?;
    let client_secret = env_string("SF_UP_CLIENT_SECRET")?;
    let username = env_string("SF_UP_USERNAME")?;
    let password = env_string("SF_UP_PASSWORD")?;
    let security_token = env_string("SF_UP_SECURITY_TOKEN").unwrap_or_default();
    let token_url =
        env_string("SF_UP_TOKEN_URL").unwrap_or_else(|| PRODUCTION_TOKEN_URL.to_string());
    let token_url = normalize_oauth_token_url("SF_UP_TOKEN_URL", &token_url)
        .unwrap_or_else(|error| panic!("{error}"));

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

fn load_live_auth() -> Option<LiveAuth> {
    if let Some(auth) = try_jwt_auth() {
        return Some(auth);
    }

    if let Some(auth) = try_client_credentials_auth() {
        return Some(auth);
    }

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
    let pubsub_topic = env_string("SF_PUBSUB_TOPIC").unwrap_or_else(|| {
        panic!(
            "{}",
            ConfigError::MissingValue(
                "SF_PUBSUB_TOPIC is required for Pub/Sub live tests; set it to a configured topic such as /data/AccountChangeEvent"
                    .to_string(),
            )
        )
    });

    Some(LiveConfig {
        auth,
        api_version: std::env::var("SF_API_VERSION").unwrap_or_else(|_| "v62.0".to_string()),
        test_timeout: Duration::from_secs(env_u64("SF_LIVE_TEST_TIMEOUT_SECS", 120)),
        pubsub_endpoint: env_string("SF_PUBSUB_ENDPOINT")
            .unwrap_or_else(|| DEFAULT_PUBSUB_ENDPOINT.to_string()),
        pubsub_topic,
    })
}

async fn create_live_client(config: &LiveConfig) -> force::error::Result<ForceClient<LiveAuth>> {
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

#[tokio::test]
#[ignore = "requires a live Salesforce org with Pub/Sub API and SF_PUBSUB_TOPIC configured"]
async fn live_pubsub_get_topic_and_schema_smoke() -> Result<()> {
    let Some(config) = load_live_config() else {
        eprintln!("skipping live_pubsub_get_topic_and_schema_smoke: no credentials available");
        return Ok(());
    };

    eprintln!(
        "using auth: {} (testing Pub/Sub topic {})",
        config.auth, config.pubsub_topic
    );

    let (topic, schema) = tokio::time::timeout(config.test_timeout, async {
        let client = create_live_client(&config).await?;
        let handler = PubSubHandler::connect(
            client.session(),
            PubSubConfig {
                endpoint: config.pubsub_endpoint.clone(),
                ..PubSubConfig::default()
            },
        )
        .await?;

        let topic = handler.get_topic(&config.pubsub_topic).await?;
        let schema = handler.get_schema(&topic.schema_id).await?;
        Ok::<_, anyhow::Error>((topic, schema))
    })
    .await
    .with_context(|| {
        format!(
            "Pub/Sub live test timed out after {} second(s)",
            config.test_timeout.as_secs()
        )
    })??;

    assert_eq!(topic.topic_name, config.pubsub_topic);
    assert!(!topic.schema_id.is_empty());
    assert_eq!(schema.schema_id, topic.schema_id);
    apache_avro::Schema::parse_str(&schema.schema_json)
        .with_context(|| format!("Pub/Sub returned invalid Avro schema {}", schema.schema_id))?;

    Ok(())
}
