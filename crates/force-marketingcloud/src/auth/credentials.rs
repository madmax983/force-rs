//! Installed-package (server-to-server) authentication for Marketing Cloud.
//!
//! Marketing Cloud Engagement authenticates via an **Installed Package** with a
//! Server-to-Server API Integration component. Unlike the core Salesforce OAuth
//! flows, the token request body is JSON (not form-encoded), the auth host is a
//! per-tenant subdomain, and the response carries no refresh token.

use crate::auth::token::{AccessToken, TokenResponse};
use crate::error::{MarketingCloudError, Result};
use async_trait::async_trait;
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Map, Value, json};
use std::fmt::Debug;

/// Maximum token-response body size accepted from the auth endpoint (1 MiB).
const MAX_TOKEN_BODY_BYTES: usize = 1024 * 1024;

/// Obtains Marketing Cloud access tokens, optionally scoped to a business unit.
#[async_trait]
pub trait Authenticator: Debug + Send + Sync {
    /// Authenticates and returns a fresh [`AccessToken`].
    ///
    /// `account_id` optionally overrides the default business unit (MID). When
    /// `None`, the authenticator's configured default is used (if any).
    ///
    /// # Errors
    ///
    /// Returns an error if the network request fails, the credentials are
    /// rejected, or the response cannot be parsed.
    async fn authenticate(&self, account_id: Option<&str>) -> Result<AccessToken>;
}

/// Client-credentials authenticator backed by an Installed Package integration.
#[derive(Debug, Clone)]
pub struct InstalledPackageCredentials {
    /// Client id from the Installed Package API Integration.
    client_id: String,

    /// Client secret from the Installed Package API Integration.
    client_secret: SecretString,

    /// Fully-qualified `v2/token` endpoint URL.
    token_url: String,

    /// Default business unit (MID) applied when no per-call override is given.
    default_account_id: Option<String>,

    /// Optional space-separated scope subset.
    scope: Option<String>,

    /// HTTP client used for token requests.
    http: reqwest::Client,
}

impl InstalledPackageCredentials {
    /// Creates a new authenticator.
    #[must_use]
    pub fn new(
        client_id: impl Into<String>,
        client_secret: SecretString,
        token_url: impl Into<String>,
        default_account_id: Option<String>,
        scope: Option<String>,
        http: reqwest::Client,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret,
            token_url: token_url.into(),
            default_account_id,
            scope,
            http,
        }
    }

    /// Returns the configured token endpoint URL.
    #[must_use]
    pub fn token_url(&self) -> &str {
        &self.token_url
    }

    /// Returns the default business unit (MID), if configured.
    #[must_use]
    pub fn default_account_id(&self) -> Option<&str> {
        self.default_account_id.as_deref()
    }

    /// Builds the JSON request body for a token request.
    fn request_body(&self, account_id: Option<&str>) -> Value {
        let mut body = Map::new();
        body.insert("grant_type".to_string(), json!("client_credentials"));
        body.insert("client_id".to_string(), json!(self.client_id));
        body.insert(
            "client_secret".to_string(),
            json!(self.client_secret.expose_secret()),
        );

        let effective_account = account_id.or(self.default_account_id.as_deref());
        if let Some(account) = effective_account {
            body.insert("account_id".to_string(), json!(account));
        }
        if let Some(scope) = &self.scope {
            body.insert("scope".to_string(), json!(scope));
        }
        Value::Object(body)
    }
}

#[async_trait]
impl Authenticator for InstalledPackageCredentials {
    async fn authenticate(&self, account_id: Option<&str>) -> Result<AccessToken> {
        let body = self.request_body(account_id);

        let response = self
            .http
            .post(&self.token_url)
            .json(&body)
            .send()
            .await
            .map_err(MarketingCloudError::Http)?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(MarketingCloudError::Auth(
                MarketingCloudError::from_error_body(status.as_u16(), &text).to_string(),
            ));
        }

        let bytes = response.bytes().await.map_err(MarketingCloudError::Http)?;
        if bytes.len() > MAX_TOKEN_BODY_BYTES {
            return Err(MarketingCloudError::Auth(
                "token response body exceeded the maximum accepted size".to_string(),
            ));
        }

        let token_response: TokenResponse = serde_json::from_slice(&bytes)?;
        AccessToken::from_response(token_response)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn creds(
        default_account: Option<String>,
        scope: Option<String>,
    ) -> InstalledPackageCredentials {
        InstalledPackageCredentials::new(
            "cid",
            SecretString::new("csecret".to_string().into()),
            "https://sub.auth.marketingcloudapis.com/v2/token",
            default_account,
            scope,
            reqwest::Client::new(),
        )
    }

    #[test]
    fn body_includes_required_fields() {
        let body = creds(None, None).request_body(None);
        assert_eq!(body["grant_type"], json!("client_credentials"));
        assert_eq!(body["client_id"], json!("cid"));
        assert_eq!(body["client_secret"], json!("csecret"));
        assert!(body.get("account_id").is_none());
        assert!(body.get("scope").is_none());
    }

    #[test]
    fn body_uses_default_account_when_no_override() {
        let body = creds(Some("111".to_string()), None).request_body(None);
        assert_eq!(body["account_id"], json!("111"));
    }

    #[test]
    fn override_account_takes_precedence_over_default() {
        let body = creds(Some("111".to_string()), None).request_body(Some("222"));
        assert_eq!(body["account_id"], json!("222"));
    }

    #[test]
    fn scope_included_when_configured() {
        let body = creds(None, Some("email_send".to_string())).request_body(None);
        assert_eq!(body["scope"], json!("email_send"));
    }

    #[test]
    fn secret_is_not_leaked_in_debug() {
        let debug = format!("{:?}", creds(None, None));
        assert!(!debug.contains("csecret"));
    }
}
