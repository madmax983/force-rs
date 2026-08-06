//! OAuth 2.0 Authorization Code Flow with PKCE.
//!
//! This module implements the OAuth 2.0 Authorization Code grant type extended
//! with Proof Key for Code Exchange (PKCE, [RFC 7636]). Salesforce recommends
//! this flow — combined with PKCE — as the replacement for the deprecated
//! user-agent and username-password flows for interactive (browser-based)
//! clients.
//!
//! # Requirements
//!
//! This module requires the `auth_code` feature flag to be enabled.
//!
//! # A Non-Headless Flow
//!
//! Unlike [`ClientCredentials`](crate::auth::ClientCredentials) or JWT Bearer,
//! the Authorization Code flow **cannot complete without a user interaction**.
//! The library therefore splits the work into three stages:
//!
//! 1. **Generate** a [`PkceChallenge`] and build an authorization URL with
//!    [`AuthorizeUrlBuilder`]. Redirect the user's browser there.
//! 2. Salesforce authenticates the user and redirects back to your
//!    `redirect_uri` with a single-use `code` query parameter.
//! 3. **Exchange** that `code` (together with the original `code_verifier`) for
//!    tokens by constructing an [`AuthorizationCode`] authenticator and calling
//!    [`Authenticator::authenticate`](crate::auth::Authenticator::authenticate).
//!
//! After the initial exchange the authenticator stores the returned
//! `refresh_token` and transparently rotates it on
//! [`Authenticator::refresh`](crate::auth::Authenticator::refresh), exactly like
//! the username-password authenticator.
//!
//! # PKCE
//!
//! * `code_verifier` — 43–128 characters from the unreserved set
//!   `[A-Za-z0-9-._~]`, generated with a cryptographically secure RNG.
//! * `code_challenge` — `base64url(SHA-256(code_verifier))` with padding
//!   removed.
//! * `code_challenge_method` — always `S256`.
//!
//! # Example
//!
//! ```ignore
//! use force::auth::{AuthorizationCode, AuthorizeUrlBuilder, PkceChallenge};
//! use force::auth::Authenticator;
//!
//! // 1. Generate PKCE material and the authorize URL.
//! let pkce = PkceChallenge::generate();
//! let url = AuthorizeUrlBuilder::new_production(
//!     "client_id",
//!     "https://app.example.com/callback",
//!     &pkce,
//! )
//! .scope("api")
//! .scope("refresh_token")
//! .state("opaque-state")
//! .build();
//! println!("Open in browser: {url}");
//!
//! // 2. ... user authorizes, browser redirects with ?code=... ...
//!
//! // 3. Exchange the code for tokens (public client — no secret).
//! let auth = AuthorizationCode::new_production(
//!     "client_id",
//!     None,
//!     "https://app.example.com/callback",
//!     received_code,
//!     pkce.verifier(),
//! );
//! let token = auth.authenticate().await?;
//! ```
//!
//! [RFC 7636]: https://datatracker.ietf.org/doc/html/rfc7636

#![cfg(feature = "auth_code")]

use crate::auth::token::{AccessToken, TokenResponse};
use crate::error::{AuthenticationError, ForceError, HttpError, Result};
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::sync::RwLock;

/// The PKCE code challenge method. Salesforce only supports `S256`.
const CODE_CHALLENGE_METHOD: &str = "S256";

/// Number of random bytes used to seed a generated `code_verifier`.
///
/// 32 bytes base64url-encode to a 43-character string, the minimum permitted
/// `code_verifier` length and the value recommended by RFC 7636 Appendix B.
const VERIFIER_ENTROPY_BYTES: usize = 32;

/// Minimum permitted `code_verifier` length (RFC 7636 §4.1).
const MIN_VERIFIER_LEN: usize = 43;

/// Maximum permitted `code_verifier` length (RFC 7636 §4.1).
const MAX_VERIFIER_LEN: usize = 128;

/// Proof Key for Code Exchange (PKCE) material.
///
/// Holds a `code_verifier` (kept secret) together with its derived
/// `code_challenge` and the challenge method (`S256`). Generate a fresh
/// instance for every authorization attempt with [`PkceChallenge::generate`],
/// or reconstruct one from a previously generated verifier with
/// [`PkceChallenge::from_verifier`].
#[derive(Clone)]
pub struct PkceChallenge {
    /// The high-entropy `code_verifier` (kept secret).
    verifier: SecretString,

    /// The derived `code_challenge` = `base64url(SHA-256(verifier))`.
    challenge: String,
}

// Manual Debug to avoid leaking the verifier.
impl std::fmt::Debug for PkceChallenge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PkceChallenge")
            .field("verifier", &"[REDACTED]")
            .field("challenge", &self.challenge)
            .field("method", &CODE_CHALLENGE_METHOD)
            .finish()
    }
}

impl PkceChallenge {
    /// Generates a fresh PKCE challenge using a cryptographically secure RNG.
    ///
    /// The `code_verifier` is 43 characters drawn from the URL-safe base64
    /// alphabet (a subset of the unreserved set `[A-Za-z0-9-._~]`), derived
    /// from 32 bytes of OS entropy.
    ///
    /// # Panics
    ///
    /// Panics only if the operating system's secure random number generator is
    /// unavailable, which indicates a critically misconfigured platform.
    #[must_use]
    pub fn generate() -> Self {
        let mut bytes = [0u8; VERIFIER_ENTROPY_BYTES];
        getrandom::getrandom(&mut bytes)
            .unwrap_or_else(|e| panic!("secure RNG unavailable for PKCE verifier: {e}"));
        let verifier = URL_SAFE_NO_PAD.encode(bytes);
        // A generated verifier is always valid by construction.
        let challenge = derive_challenge(&verifier);
        Self {
            verifier: SecretString::new(verifier.into()),
            challenge,
        }
    }

    /// Reconstructs a PKCE challenge from an existing `code_verifier`.
    ///
    /// Useful when the verifier was generated earlier (for example, persisted
    /// across an HTTP redirect round-trip) and must be paired again with its
    /// challenge for the token exchange.
    ///
    /// # Errors
    ///
    /// Returns [`AuthenticationError::InvalidCredentials`] if the verifier is
    /// not 43–128 characters long or contains characters outside the unreserved
    /// set `[A-Za-z0-9-._~]`.
    pub fn from_verifier(verifier: impl Into<String>) -> Result<Self> {
        let verifier = verifier.into();
        validate_verifier(&verifier)?;
        let challenge = derive_challenge(&verifier);
        Ok(Self {
            verifier: SecretString::new(verifier.into()),
            challenge,
        })
    }

    /// Returns the `code_verifier` value.
    ///
    /// # Security
    ///
    /// This exposes the secret verifier. Pass it only to the token exchange and
    /// avoid logging it.
    #[must_use]
    pub fn verifier(&self) -> &str {
        self.verifier.expose_secret()
    }

    /// Returns the derived `code_challenge` (safe to send in an authorize URL).
    #[must_use]
    pub fn challenge(&self) -> &str {
        &self.challenge
    }

    /// Returns the challenge method, always `S256`.
    #[must_use]
    pub const fn method(&self) -> &'static str {
        CODE_CHALLENGE_METHOD
    }
}

/// Derives `base64url(SHA-256(verifier))` with padding removed.
fn derive_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

/// Validates a `code_verifier` against RFC 7636 §4.1.
fn validate_verifier(verifier: &str) -> Result<()> {
    let len = verifier.len();
    if !(MIN_VERIFIER_LEN..=MAX_VERIFIER_LEN).contains(&len) {
        return Err(ForceError::Authentication(
            AuthenticationError::InvalidCredentials(format!(
                "code_verifier must be {MIN_VERIFIER_LEN}-{MAX_VERIFIER_LEN} characters, got {len}"
            )),
        ));
    }
    if let Some(bad) = verifier
        .bytes()
        .find(|&b| !is_unreserved(b))
        .map(|b| b as char)
    {
        return Err(ForceError::Authentication(
            AuthenticationError::InvalidCredentials(format!(
                "code_verifier contains illegal character {bad:?}; only [A-Za-z0-9-._~] are allowed"
            )),
        ));
    }
    Ok(())
}

/// Returns `true` if `byte` is in the unreserved set `[A-Za-z0-9-._~]`.
const fn is_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}

/// Builder for the OAuth 2.0 authorization URL (PKCE web server flow).
///
/// Produces the `/services/oauth2/authorize` URL the user's browser is
/// redirected to. The generated URL always carries `response_type=code`,
/// `code_challenge`, and `code_challenge_method=S256`.
#[derive(Debug, Clone)]
pub struct AuthorizeUrlBuilder {
    /// Login base URL (for example `https://login.salesforce.com`).
    login_url: String,
    /// OAuth client ID (Connected App consumer key).
    client_id: String,
    /// Registered redirect URI.
    redirect_uri: String,
    /// Requested OAuth scopes (space-separated when serialized).
    scopes: Vec<String>,
    /// Opaque CSRF `state` value.
    state: Option<String>,
    /// PKCE `code_challenge`.
    code_challenge: String,
}

impl AuthorizeUrlBuilder {
    /// Creates a builder against an explicit login base URL.
    ///
    /// The `login_url` should be a bare origin such as
    /// `https://login.salesforce.com` or an org's My Domain host; the
    /// `/services/oauth2/authorize` path is appended automatically.
    pub fn new(
        login_url: impl Into<String>,
        client_id: impl Into<String>,
        redirect_uri: impl Into<String>,
        challenge: &PkceChallenge,
    ) -> Self {
        Self {
            login_url: login_url.into(),
            client_id: client_id.into(),
            redirect_uri: redirect_uri.into(),
            scopes: Vec::new(),
            state: None,
            code_challenge: challenge.challenge().to_string(),
        }
    }

    /// Creates a builder targeting the Salesforce production login host.
    pub fn new_production(
        client_id: impl Into<String>,
        redirect_uri: impl Into<String>,
        challenge: &PkceChallenge,
    ) -> Self {
        Self::new(
            crate::auth::PRODUCTION_LOGIN_URL,
            client_id,
            redirect_uri,
            challenge,
        )
    }

    /// Creates a builder targeting the Salesforce sandbox login host.
    pub fn new_sandbox(
        client_id: impl Into<String>,
        redirect_uri: impl Into<String>,
        challenge: &PkceChallenge,
    ) -> Self {
        Self::new(
            crate::auth::SANDBOX_LOGIN_URL,
            client_id,
            redirect_uri,
            challenge,
        )
    }

    /// Adds a single OAuth scope.
    #[must_use]
    pub fn scope(mut self, scope: impl Into<String>) -> Self {
        self.scopes.push(scope.into());
        self
    }

    /// Adds multiple OAuth scopes.
    #[must_use]
    pub fn scopes<I, S>(mut self, scopes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.scopes.extend(scopes.into_iter().map(Into::into));
        self
    }

    /// Sets the opaque `state` value used for CSRF protection.
    #[must_use]
    pub fn state(mut self, state: impl Into<String>) -> Self {
        self.state = Some(state.into());
        self
    }

    /// Builds the fully-encoded authorization URL.
    ///
    /// All parameter values are percent-encoded via the `url` crate, and scopes
    /// are joined with spaces per the OAuth specification.
    #[must_use]
    pub fn build(&self) -> String {
        let base = format!(
            "{}/services/oauth2/authorize",
            self.login_url.trim_end_matches('/')
        );

        // `Url::parse` on our own well-formed base cannot fail, but avoid
        // unwrap: fall back to manual concatenation if it somehow does.
        let Ok(mut url) = url::Url::parse(&base) else {
            return base;
        };

        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("response_type", "code");
            qp.append_pair("client_id", &self.client_id);
            qp.append_pair("redirect_uri", &self.redirect_uri);
            if !self.scopes.is_empty() {
                qp.append_pair("scope", &self.scopes.join(" "));
            }
            if let Some(state) = &self.state {
                qp.append_pair("state", state);
            }
            qp.append_pair("code_challenge", &self.code_challenge);
            qp.append_pair("code_challenge_method", CODE_CHALLENGE_METHOD);
        }

        url.into()
    }
}

/// OAuth 2.0 Authorization Code + PKCE authenticator.
///
/// Constructed *after* the user has authorized the app and the browser has been
/// redirected back with a single-use authorization `code`. The initial
/// [`Authenticator::authenticate`](crate::auth::Authenticator::authenticate)
/// call exchanges that code (plus the PKCE `code_verifier`) for tokens;
/// [`Authenticator::refresh`](crate::auth::Authenticator::refresh) then rotates
/// the stored refresh token.
///
/// The `client_secret` is optional: public clients (SPAs, native/mobile apps)
/// omit it and rely on PKCE, whereas confidential clients supply it.
#[derive(Clone)]
pub struct AuthorizationCode {
    /// OAuth client ID (Connected App consumer key).
    client_id: String,

    /// Optional OAuth client secret (confidential clients only).
    client_secret: Option<SecretString>,

    /// Registered redirect URI (must match the authorize request).
    redirect_uri: String,

    /// Token endpoint URL.
    token_url: String,

    /// Single-use authorization code received on the redirect callback.
    code: SecretString,

    /// PKCE `code_verifier` matching the challenge sent to `/authorize`.
    code_verifier: SecretString,

    /// HTTP client for token requests.
    client: reqwest::Client,

    /// Stored refresh token from the most recent successful exchange.
    refresh_token: Arc<RwLock<Option<SecretString>>>,
}

// Manual Debug to redact secrets.
impl std::fmt::Debug for AuthorizationCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthorizationCode")
            .field("client_id", &self.client_id)
            .field(
                "client_secret",
                &self.client_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .field("redirect_uri", &self.redirect_uri)
            .field("token_url", &self.token_url)
            .field("code", &"[REDACTED]")
            .field("code_verifier", &"[REDACTED]")
            .finish()
    }
}

impl AuthorizationCode {
    /// Creates a new `AuthorizationCode` authenticator.
    ///
    /// # Arguments
    ///
    /// * `client_id` - OAuth client ID from the Connected App
    /// * `client_secret` - Optional client secret; pass `None` for public
    ///   (PKCE-only) clients, `Some(secret)` for confidential clients
    /// * `redirect_uri` - Redirect URI, identical to the one used in the
    ///   authorize request
    /// * `code` - Single-use authorization code from the redirect callback
    /// * `code_verifier` - PKCE `code_verifier` matching the challenge sent to
    ///   `/authorize` (see [`PkceChallenge::verifier`])
    /// * `token_url` - Token endpoint URL
    ///
    /// # Panics
    ///
    /// Panics if the default HTTP client cannot be initialized.
    pub fn new(
        client_id: impl Into<String>,
        client_secret: Option<String>,
        redirect_uri: impl Into<String>,
        code: impl Into<String>,
        code_verifier: impl Into<String>,
        token_url: impl Into<String>,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: client_secret.map(|s| SecretString::new(s.into())),
            redirect_uri: redirect_uri.into(),
            token_url: token_url.into(),
            code: SecretString::new(code.into().into()),
            code_verifier: SecretString::new(code_verifier.into().into()),
            client: crate::auth::default_auth_http_client(),
            refresh_token: Arc::new(RwLock::new(None)),
        }
    }

    /// Creates an authenticator against the Salesforce production token URL.
    pub fn new_production(
        client_id: impl Into<String>,
        client_secret: Option<String>,
        redirect_uri: impl Into<String>,
        code: impl Into<String>,
        code_verifier: impl Into<String>,
    ) -> Self {
        Self::new(
            client_id,
            client_secret,
            redirect_uri,
            code,
            code_verifier,
            crate::auth::PRODUCTION_TOKEN_URL,
        )
    }

    /// Creates an authenticator against the Salesforce sandbox token URL.
    pub fn new_sandbox(
        client_id: impl Into<String>,
        client_secret: Option<String>,
        redirect_uri: impl Into<String>,
        code: impl Into<String>,
        code_verifier: impl Into<String>,
    ) -> Self {
        Self::new(
            client_id,
            client_secret,
            redirect_uri,
            code,
            code_verifier,
            crate::auth::SANDBOX_TOKEN_URL,
        )
    }

    /// Sets a custom HTTP client.
    #[must_use]
    pub fn with_client(mut self, client: reqwest::Client) -> Self {
        self.client = client;
        self
    }

    /// Returns the currently stored refresh token, if any.
    ///
    /// Primarily useful for persisting the rotating refresh token across
    /// process restarts.
    ///
    /// # Security
    ///
    /// Exposes the secret refresh token; handle and store it securely.
    pub async fn refresh_token(&self) -> Option<String> {
        self.refresh_token
            .read()
            .await
            .as_ref()
            .map(|s| s.expose_secret().to_string())
    }

    /// Seeds the authenticator with a previously persisted refresh token.
    ///
    /// Allows resuming a session — subsequent
    /// [`Authenticator::refresh`](crate::auth::Authenticator::refresh) calls
    /// will use it without needing a fresh authorization code.
    pub async fn set_refresh_token(&self, refresh_token: impl Into<String>) {
        let mut stored = self.refresh_token.write().await;
        *stored = Some(SecretString::new(refresh_token.into().into()));
    }

    /// Derives the token revocation endpoint from the token endpoint.
    fn revoke_url(&self) -> String {
        match self.token_url.rfind("/token") {
            Some(idx) => format!("{}/revoke", &self.token_url[..idx]),
            None => format!(
                "{}/services/oauth2/revoke",
                self.token_url.trim_end_matches('/')
            ),
        }
    }

    /// Revokes an access or refresh token at `/services/oauth2/revoke`.
    ///
    /// Per the OAuth spec, revoking a refresh token also invalidates the
    /// associated access tokens.
    ///
    /// # Errors
    ///
    /// Returns an error if the network request fails or Salesforce returns a
    /// non-success status.
    pub async fn revoke(&self, token: &str) -> Result<()> {
        let params = [("token", token)];
        let response = self
            .client
            .post(self.revoke_url())
            .form(&params)
            .send()
            .await
            .map_err(|e| ForceError::Http(HttpError::RequestFailed(e)))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(crate::auth::handle_oauth_error(response, Some("token revocation failed")).await)
        }
    }

    /// Revokes and clears the stored refresh token, if present.
    ///
    /// A no-op returning `Ok(())` when no refresh token is stored.
    ///
    /// # Errors
    ///
    /// Returns an error if the revocation request fails.
    pub async fn revoke_stored_refresh_token(&self) -> Result<()> {
        let stored = self.refresh_token.read().await.clone();
        if let Some(rt) = stored {
            self.revoke(rt.expose_secret()).await?;
            let mut guard = self.refresh_token.write().await;
            if guard.as_ref().map(|s| s.expose_secret()) == Some(rt.expose_secret()) {
                *guard = None;
            }
        }
        Ok(())
    }

    /// Sends a token request and parses the response.
    ///
    /// Shared between the authorization-code exchange and the refresh grant.
    async fn send_token_request(&self, params: &[(&str, &str)]) -> Result<TokenResponse> {
        let response = self
            .client
            .post(&self.token_url)
            .form(params)
            .send()
            .await
            .map_err(|e| ForceError::Http(HttpError::RequestFailed(e)))?;

        if !response.status().is_success() {
            return Err(crate::auth::handle_oauth_error(response, None).await);
        }

        let bytes = crate::http::error::read_capped_body_bytes(response, 1024 * 1024).await?;
        serde_json::from_slice::<TokenResponse>(&bytes)
            .map_err(crate::error::SerializationError::from)
            .map_err(Into::into)
    }

    /// Stores the refresh token from a token response (if present).
    async fn store_refresh_token(&self, response: &TokenResponse) {
        if let Some(ref rt) = response.refresh_token {
            let mut stored = self.refresh_token.write().await;
            *stored = Some(rt.clone());
        }
    }
}

#[async_trait]
impl crate::auth::authenticator::Authenticator for AuthorizationCode {
    async fn authenticate(&self) -> Result<AccessToken> {
        // Build the authorization-code exchange parameters. `client_secret` is
        // appended only for confidential clients.
        let mut params: Vec<(&str, &str)> = vec![
            ("grant_type", "authorization_code"),
            ("client_id", self.client_id.as_str()),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("code", self.code.expose_secret()),
            ("code_verifier", self.code_verifier.expose_secret()),
        ];
        if let Some(secret) = &self.client_secret {
            params.push(("client_secret", secret.expose_secret()));
        }

        let token_response = self.send_token_request(&params).await?;
        self.store_refresh_token(&token_response).await;
        Ok(AccessToken::from_response(token_response))
    }

    async fn refresh(&self) -> Result<AccessToken> {
        // Try the stored refresh token first.
        let stored_rt = self.refresh_token.read().await.clone();

        if let Some(rt) = stored_rt {
            let mut params: Vec<(&str, &str)> = vec![
                ("grant_type", "refresh_token"),
                ("client_id", self.client_id.as_str()),
                ("refresh_token", rt.expose_secret()),
            ];
            if let Some(secret) = &self.client_secret {
                params.push(("client_secret", secret.expose_secret()));
            }

            if let Ok(token_response) = self.send_token_request(&params).await {
                self.store_refresh_token(&token_response).await;
                return Ok(AccessToken::from_response(token_response));
            }
            // Refresh token revoked or expired — clear it and fall back to the
            // authorization-code exchange below.
            let mut stored = self.refresh_token.write().await;
            // Only clear if another thread hasn't already stored a *newer*
            // refresh token while our failing request was in flight.
            if stored.as_ref().map(|s| s.expose_secret()) == Some(rt.expose_secret()) {
                *stored = None;
            }
        }

        // No refresh token (or refresh failed) — attempt the initial exchange.
        //
        // NOTE: the authorization `code` is single-use. If it has already been
        // redeemed this call will fail; the Authorization Code flow cannot
        // re-authenticate headlessly and the caller must restart the flow with
        // a fresh `PkceChallenge` and authorization URL.
        self.authenticate().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::Authenticator;
    use crate::error::AuthenticationError;
    use crate::test_utils::must::Must;
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn sample_token_response() -> serde_json::Value {
        serde_json::json!({
            "access_token": "00Dxx0000001gPL!auth_code_token",
            "instance_url": "https://test.my.salesforce.com",
            "token_type": "Bearer",
            "issued_at": "1704067200000",
            "signature": "testSignature==",
            "refresh_token": "fake_refresh_token_for_testing"
        })
    }

    // ── PKCE tests ───────────────────────────────────────────────────

    #[test]
    fn test_generate_verifier_length_and_charset() {
        let pkce = PkceChallenge::generate();
        let verifier = pkce.verifier();
        assert!(
            (MIN_VERIFIER_LEN..=MAX_VERIFIER_LEN).contains(&verifier.len()),
            "verifier length {} out of range",
            verifier.len()
        );
        assert!(
            verifier.bytes().all(is_unreserved),
            "verifier contains characters outside the unreserved set"
        );
    }

    #[test]
    fn test_generate_produces_unique_verifiers() {
        let a = PkceChallenge::generate();
        let b = PkceChallenge::generate();
        assert_ne!(a.verifier(), b.verifier());
        assert_ne!(a.challenge(), b.challenge());
    }

    #[test]
    fn test_challenge_derivation_known_vector() {
        // RFC 7636 Appendix B worked example.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let expected_challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        let pkce = PkceChallenge::from_verifier(verifier).must();
        assert_eq!(pkce.challenge(), expected_challenge);
        assert_eq!(pkce.verifier(), verifier);
        assert_eq!(pkce.method(), "S256");
    }

    #[test]
    fn test_from_verifier_rejects_short() {
        let short = "tooshort";
        let result = PkceChallenge::from_verifier(short);
        assert!(matches!(
            result,
            Err(ForceError::Authentication(
                AuthenticationError::InvalidCredentials(_)
            ))
        ));
    }

    #[test]
    fn test_from_verifier_rejects_long() {
        let long = "a".repeat(129);
        let result = PkceChallenge::from_verifier(long);
        assert!(matches!(
            result,
            Err(ForceError::Authentication(
                AuthenticationError::InvalidCredentials(_)
            ))
        ));
    }

    #[test]
    fn test_from_verifier_rejects_illegal_char() {
        // 43 chars but contains a space, which is not in the unreserved set.
        let bad = format!("{}{}", "a".repeat(42), " ");
        let result = PkceChallenge::from_verifier(bad);
        assert!(matches!(
            result,
            Err(ForceError::Authentication(
                AuthenticationError::InvalidCredentials(_)
            ))
        ));
    }

    #[test]
    fn test_from_verifier_accepts_boundary_lengths() {
        assert!(PkceChallenge::from_verifier("a".repeat(MIN_VERIFIER_LEN)).is_ok());
        assert!(PkceChallenge::from_verifier("a".repeat(MAX_VERIFIER_LEN)).is_ok());
    }

    #[test]
    fn test_pkce_debug_redacts_verifier() {
        let pkce = PkceChallenge::from_verifier("a".repeat(MIN_VERIFIER_LEN)).must();
        let debug = format!("{pkce:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(debug.contains(pkce.challenge()));
        assert!(!debug.contains(&"a".repeat(MIN_VERIFIER_LEN)));
    }

    // ── Authorize URL tests ──────────────────────────────────────────

    #[test]
    fn test_authorize_url_contains_required_params() {
        let pkce =
            PkceChallenge::from_verifier("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk").must();
        let url = AuthorizeUrlBuilder::new_production(
            "myClientId",
            "https://app.example.com/callback",
            &pkce,
        )
        .scope("api")
        .scope("refresh_token")
        .state("xyz-state")
        .build();

        assert!(url.starts_with("https://login.salesforce.com/services/oauth2/authorize?"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("client_id=myClientId"));
        // redirect_uri is percent-encoded.
        assert!(url.contains("redirect_uri=https%3A%2F%2Fapp.example.com%2Fcallback"));
        // scopes are space-joined then percent-encoded ('+' for space).
        assert!(url.contains("scope=api+refresh_token"));
        assert!(url.contains("state=xyz-state"));
        assert!(url.contains(&format!("code_challenge={}", pkce.challenge())));
        assert!(url.contains("code_challenge_method=S256"));
    }

    #[test]
    fn test_authorize_url_sandbox_host() {
        let pkce = PkceChallenge::generate();
        let url = AuthorizeUrlBuilder::new_sandbox("cid", "https://cb.example.com", &pkce).build();
        assert!(url.starts_with("https://test.salesforce.com/services/oauth2/authorize?"));
    }

    #[test]
    fn test_authorize_url_omits_scope_and_state_when_absent() {
        let pkce = PkceChallenge::generate();
        let url = AuthorizeUrlBuilder::new("https://my.example.com/", "cid", "cb", &pkce).build();
        assert!(url.starts_with("https://my.example.com/services/oauth2/authorize?"));
        assert!(!url.contains("scope="));
        assert!(!url.contains("state="));
    }

    #[test]
    fn test_authorize_url_scopes_iter() {
        let pkce = PkceChallenge::generate();
        let url = AuthorizeUrlBuilder::new_production("cid", "cb", &pkce)
            .scopes(["api", "web", "refresh_token"])
            .build();
        assert!(url.contains("scope=api+web+refresh_token"));
    }

    // ── Constructor / debug tests ────────────────────────────────────

    #[test]
    fn test_new_production_token_url() {
        let auth = AuthorizationCode::new_production(
            "cid",
            None,
            "https://cb.example.com",
            "the_code",
            "the_verifier",
        );
        assert_eq!(
            auth.token_url,
            "https://login.salesforce.com/services/oauth2/token"
        );
    }

    #[test]
    fn test_new_sandbox_token_url() {
        let auth = AuthorizationCode::new_sandbox(
            "cid",
            None,
            "https://cb.example.com",
            "the_code",
            "the_verifier",
        );
        assert_eq!(
            auth.token_url,
            "https://test.salesforce.com/services/oauth2/token"
        );
    }

    #[test]
    fn test_debug_redacts_secrets() {
        let auth = AuthorizationCode::new(
            "cid",
            Some("super_secret".to_string()),
            "https://cb.example.com",
            "secret_code",
            "secret_verifier",
            "https://login.salesforce.com/services/oauth2/token",
        );
        let debug = format!("{auth:?}");
        assert!(debug.contains("cid"));
        assert!(!debug.contains("super_secret"));
        assert!(!debug.contains("secret_code"));
        assert!(!debug.contains("secret_verifier"));
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn test_revoke_url_derivation() {
        let auth = AuthorizationCode::new(
            "cid",
            None,
            "cb",
            "code",
            "verifier",
            "https://login.salesforce.com/services/oauth2/token",
        );
        assert_eq!(
            auth.revoke_url(),
            "https://login.salesforce.com/services/oauth2/revoke"
        );
    }

    // ── Token exchange tests ─────────────────────────────────────────

    #[tokio::test]
    async fn test_authenticate_success_public_client() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/oauth2/token"))
            .and(body_string_contains("grant_type=authorization_code"))
            .and(body_string_contains("code=the_code"))
            .and(body_string_contains("code_verifier=the_verifier"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_token_response()))
            .mount(&server)
            .await;

        let auth = AuthorizationCode::new(
            "cid",
            None,
            "https://cb.example.com",
            "the_code",
            "the_verifier",
            format!("{}/services/oauth2/token", server.uri()),
        );

        let token = auth.authenticate().await.must();
        assert_eq!(token.as_str(), "00Dxx0000001gPL!auth_code_token");
        assert_eq!(token.instance_url(), "https://test.my.salesforce.com");
    }

    #[tokio::test]
    async fn test_authenticate_confidential_client_sends_secret() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/oauth2/token"))
            .and(body_string_contains("client_secret=my_secret"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_token_response()))
            .mount(&server)
            .await;

        let auth = AuthorizationCode::new(
            "cid",
            Some("my_secret".to_string()),
            "https://cb.example.com",
            "the_code",
            "the_verifier",
            format!("{}/services/oauth2/token", server.uri()),
        );

        let token = auth.authenticate().await.must();
        assert_eq!(token.as_str(), "00Dxx0000001gPL!auth_code_token");
    }

    #[tokio::test]
    async fn test_authenticate_stores_refresh_token() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/oauth2/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_token_response()))
            .mount(&server)
            .await;

        let auth = AuthorizationCode::new(
            "cid",
            None,
            "cb",
            "the_code",
            "the_verifier",
            format!("{}/services/oauth2/token", server.uri()),
        );

        let _token = auth.authenticate().await.must();
        assert_eq!(
            auth.refresh_token().await,
            Some("fake_refresh_token_for_testing".to_string())
        );
    }

    #[tokio::test]
    async fn test_authenticate_invalid_grant() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/oauth2/token"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "error": "invalid_grant",
                "error_description": "expired authorization code"
            })))
            .mount(&server)
            .await;

        let auth = AuthorizationCode::new(
            "cid",
            None,
            "cb",
            "used_code",
            "the_verifier",
            format!("{}/services/oauth2/token", server.uri()),
        );

        let result = auth.authenticate().await;
        if let Err(ForceError::Authentication(AuthenticationError::TokenRequestFailed(msg))) =
            result
        {
            assert!(msg.contains("invalid_grant"));
            assert!(msg.contains("expired authorization code"));
        } else {
            panic!("Expected TokenRequestFailed error, got {result:?}");
        }
    }

    // ── Refresh tests ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_refresh_uses_refresh_token() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(body_string_contains("grant_type=authorization_code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_token_response()))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(body_string_contains("grant_type=refresh_token"))
            .and(body_string_contains(
                "refresh_token=fake_refresh_token_for_testing",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "refreshed_access_token",
                "instance_url": "https://test.my.salesforce.com",
                "token_type": "Bearer",
                "issued_at": "1704070800000",
                "signature": "newSig==",
                "refresh_token": "rotated_refresh_token"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let auth = AuthorizationCode::new(
            "cid",
            None,
            "cb",
            "the_code",
            "the_verifier",
            format!("{}/services/oauth2/token", server.uri()),
        );

        let _t1 = auth.authenticate().await.must();
        let t2 = auth.refresh().await.must();
        assert_eq!(t2.as_str(), "refreshed_access_token");
        // Refresh token should have rotated.
        assert_eq!(
            auth.refresh_token().await,
            Some("rotated_refresh_token".to_string())
        );
    }

    #[tokio::test]
    async fn test_refresh_falls_back_to_code_exchange_when_revoked() {
        let server = MockServer::start().await;

        // authorization_code grant succeeds (initial + fallback = 2 calls).
        Mock::given(method("POST"))
            .and(body_string_contains("grant_type=authorization_code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_token_response()))
            .expect(2)
            .mount(&server)
            .await;

        // refresh_token grant fails (revoked).
        Mock::given(method("POST"))
            .and(body_string_contains("grant_type=refresh_token"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "error": "invalid_grant",
                "error_description": "token revoked"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let auth = AuthorizationCode::new(
            "cid",
            None,
            "cb",
            "the_code",
            "the_verifier",
            format!("{}/services/oauth2/token", server.uri()),
        );

        let _t1 = auth.authenticate().await.must();
        // Refresh fails, falls back to authorization_code exchange.
        let t2 = auth.refresh().await.must();
        assert_eq!(t2.as_str(), "00Dxx0000001gPL!auth_code_token");
    }

    #[tokio::test]
    async fn test_set_refresh_token_then_refresh() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(body_string_contains("grant_type=refresh_token"))
            .and(body_string_contains("refresh_token=seeded_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "seeded_flow_token",
                "instance_url": "https://test.my.salesforce.com",
                "token_type": "Bearer",
                "issued_at": "1704070800000",
                "signature": "sig=="
            })))
            .mount(&server)
            .await;

        let auth = AuthorizationCode::new(
            "cid",
            None,
            "cb",
            "unused_code",
            "the_verifier",
            format!("{}/services/oauth2/token", server.uri()),
        );

        auth.set_refresh_token("seeded_token").await;
        let token = auth.refresh().await.must();
        assert_eq!(token.as_str(), "seeded_flow_token");
    }

    // ── Revocation tests ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_revoke_success() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/oauth2/revoke"))
            .and(body_string_contains("token=some_token"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let auth = AuthorizationCode::new(
            "cid",
            None,
            "cb",
            "code",
            "verifier",
            format!("{}/services/oauth2/token", server.uri()),
        );

        assert!(auth.revoke("some_token").await.is_ok());
    }

    #[tokio::test]
    async fn test_revoke_stored_refresh_token_clears_it() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/oauth2/revoke"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let auth = AuthorizationCode::new(
            "cid",
            None,
            "cb",
            "code",
            "verifier",
            format!("{}/services/oauth2/token", server.uri()),
        );

        auth.set_refresh_token("to_revoke").await;
        assert!(auth.revoke_stored_refresh_token().await.is_ok());
        assert_eq!(auth.refresh_token().await, None);
    }

    #[tokio::test]
    async fn test_revoke_stored_refresh_token_noop_when_absent() {
        let auth = AuthorizationCode::new(
            "cid",
            None,
            "cb",
            "code",
            "verifier",
            "https://login.salesforce.com/services/oauth2/token",
        );
        // No refresh token stored → should be a no-op success without any HTTP.
        assert!(auth.revoke_stored_refresh_token().await.is_ok());
    }

    #[tokio::test]
    async fn test_with_client_custom_http_client() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/services/oauth2/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_token_response()))
            .mount(&server)
            .await;

        let custom_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .must();

        let auth = AuthorizationCode::new(
            "cid",
            None,
            "cb",
            "the_code",
            "the_verifier",
            format!("{}/services/oauth2/token", server.uri()),
        )
        .with_client(custom_client);

        let token = auth.authenticate().await.must();
        assert_eq!(token.as_str(), "00Dxx0000001gPL!auth_code_token");
    }

    #[tokio::test]
    async fn test_authenticate_network_error() {
        let auth = AuthorizationCode::new(
            "cid",
            None,
            "cb",
            "code",
            "verifier",
            "http://invalid.invalid.localhost:99999/oauth2/token",
        );
        let result = auth.authenticate().await;
        assert!(matches!(result, Err(ForceError::Http(_))));
    }
}
