//! OAuth 2.0 JWT Bearer Flow.
//!
//! This module implements the OAuth 2.0 JWT Bearer flow for server-to-server
//! authentication with Salesforce using RSA-signed JSON Web Tokens.
//!
//! # Requirements
//!
//! This module requires the `jwt` feature flag to be enabled.
//!
//! # Example
//!
//! ```ignore
//! use force::auth::JwtBearerFlow;
//!
//! let private_key = std::fs::read_to_string("private_key.pem")?;
//!
//! let flow = JwtBearerFlow::builder()
//!     .client_id("your_client_id")
//!     .username("user@example.com")
//!     .private_key(&private_key)
//!     .audience("https://login.salesforce.com")
//!     .build()?;
//!
//! let token = flow.authenticate().await?;
//! ```

#[cfg(feature = "jwt")]
use crate::error::{AuthenticationError, ForceError, HttpError, Result};
#[cfg(feature = "jwt")]
use crate::types::authenticator::Authenticator;
#[cfg(feature = "jwt")]
use crate::types::token::{AccessToken, TokenResponse};
#[cfg(feature = "jwt")]
use async_trait::async_trait;
#[cfg(feature = "jwt")]
use jsonwebtoken::{EncodingKey, Header, encode};
#[cfg(feature = "jwt")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "jwt")]
use std::time::{SystemTime, UNIX_EPOCH};

/// JWT claims for Salesforce OAuth JWT bearer assertion.
#[cfg(feature = "jwt")]
#[derive(Debug, Serialize, Deserialize)]
struct JwtClaims {
    /// Issuer (OAuth client ID).
    iss: String,

    /// Subject (Salesforce username).
    sub: String,

    /// Audience (token endpoint).
    aud: String,

    /// Expiration time (Unix timestamp).
    exp: u64,
}

/// OAuth 2.0 JWT Bearer flow authenticator.
///
/// This authenticator uses RSA-signed JWTs for server-to-server authentication.
/// It's suitable for scenarios where a service acts on behalf of a user without
/// requiring user interaction.
#[cfg(feature = "jwt")]
#[derive(Clone)]
pub struct JwtBearerFlow {
    /// OAuth client ID.
    client_id: String,

    /// Salesforce username.
    username: String,

    /// RSA private key for signing JWTs.
    private_key: EncodingKey,

    /// OAuth audience (token endpoint URL).
    audience: String,

    /// Token endpoint URL.
    token_url: String,

    /// HTTP client for requests.
    http_client: reqwest::Client,
}

#[cfg(feature = "jwt")]
impl JwtBearerFlow {
    /// Creates a new builder for configuring the JWT bearer flow.
    #[must_use]
    pub fn builder() -> JwtBearerBuilder {
        JwtBearerBuilder::default()
    }

    /// Generates a signed JWT for OAuth authentication.
    fn generate_jwt(&self) -> Result<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| {
                ForceError::Authentication(AuthenticationError::JwtCreationFailed(format!(
                    "System time error: {e}"
                )))
            })?
            .as_secs();

        let claims = JwtClaims {
            iss: self.client_id.clone(),
            sub: self.username.clone(),
            aud: self.audience.clone(),
            exp: now + 300, // JWT valid for 5 minutes
        };

        encode(
            &Header::new(jsonwebtoken::Algorithm::RS256),
            &claims,
            &self.private_key,
        )
        .map_err(|e| {
            ForceError::Authentication(AuthenticationError::JwtCreationFailed(format!(
                "JWT encoding failed: {e}"
            )))
        })
    }
}

#[cfg(feature = "jwt")]
impl std::fmt::Debug for JwtBearerFlow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtBearerFlow")
            .field("client_id", &self.client_id)
            .field("username", &self.username)
            .field("private_key", &"[REDACTED]")
            .field("audience", &self.audience)
            .field("token_url", &self.token_url)
            .finish()
    }
}

#[cfg(feature = "jwt")]
#[async_trait]
impl Authenticator for JwtBearerFlow {
    async fn authenticate(&self) -> Result<AccessToken> {
        let assertion = self.generate_jwt()?;

        let params = [
            ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
            ("assertion", assertion.as_str()),
        ];

        let response = self
            .http_client
            .post(&self.token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| ForceError::Http(HttpError::RequestFailed(e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            // Try to parse OAuth error response
            if let Ok(oauth_error) = serde_json::from_str::<OAuthErrorResponse>(&error_text) {
                return Err(ForceError::Authentication(
                    AuthenticationError::TokenRequestFailed(format!(
                        "{}: {}",
                        oauth_error.error, oauth_error.error_description
                    )),
                ));
            }

            return Err(ForceError::Http(HttpError::StatusError {
                status_code: status.as_u16(),
                message: error_text,
            }));
        }

        let token_response = response
            .json::<TokenResponse>()
            .await
            .map_err(|e| ForceError::Http(HttpError::RequestFailed(e)))?;

        Ok(AccessToken::from_response(token_response))
    }

    async fn refresh(&self) -> Result<AccessToken> {
        // JWT bearer flow doesn't support refresh tokens.
        // Re-authenticate with a new JWT.
        self.authenticate().await
    }
}

/// Builder for creating `JwtBearerFlow` instances.
#[cfg(feature = "jwt")]
#[derive(Default)]
pub struct JwtBearerBuilder {
    client_id: Option<String>,
    username: Option<String>,
    private_key_pem: Option<String>,
    audience: Option<String>,
    token_url: Option<String>,
    http_client: Option<reqwest::Client>,
}

#[cfg(feature = "jwt")]
impl JwtBearerBuilder {
    /// Sets the OAuth client ID.
    #[must_use]
    pub fn client_id(mut self, client_id: impl Into<String>) -> Self {
        self.client_id = Some(client_id.into());
        self
    }

    /// Sets the Salesforce username.
    #[must_use]
    pub fn username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    /// Sets the RSA private key in PEM format.
    #[must_use]
    pub fn private_key(mut self, pem: impl Into<String>) -> Self {
        self.private_key_pem = Some(pem.into());
        self
    }

    /// Sets the OAuth audience (typically the login URL).
    ///
    /// Defaults to `https://login.salesforce.com` for production.
    #[must_use]
    pub fn audience(mut self, audience: impl Into<String>) -> Self {
        self.audience = Some(audience.into());
        self
    }

    /// Sets the token endpoint URL.
    ///
    /// Defaults to `https://login.salesforce.com/services/oauth2/token`.
    #[must_use]
    pub fn token_url(mut self, url: impl Into<String>) -> Self {
        self.token_url = Some(url.into());
        self
    }

    /// Sets a custom HTTP client.
    #[must_use]
    pub fn http_client(mut self, client: reqwest::Client) -> Self {
        self.http_client = Some(client);
        self
    }

    /// Builds the `JwtBearerFlow` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if required fields are missing or the private key is invalid.
    pub fn build(self) -> Result<JwtBearerFlow> {
        let client_id = self.client_id.ok_or_else(|| {
            ForceError::Config(crate::error::ConfigError::MissingValue(
                "client_id".to_string(),
            ))
        })?;

        let username = self.username.ok_or_else(|| {
            ForceError::Config(crate::error::ConfigError::MissingValue(
                "username".to_string(),
            ))
        })?;

        let private_key_pem = self.private_key_pem.ok_or_else(|| {
            ForceError::Config(crate::error::ConfigError::MissingValue(
                "private_key".to_string(),
            ))
        })?;

        let private_key = EncodingKey::from_rsa_pem(private_key_pem.as_bytes()).map_err(|e| {
            ForceError::Authentication(AuthenticationError::InvalidJwtConfig(format!(
                "Invalid RSA private key: {e}"
            )))
        })?;

        let audience = self
            .audience
            .unwrap_or_else(|| "https://login.salesforce.com".to_string());
        let token_url = self
            .token_url
            .unwrap_or_else(|| "https://login.salesforce.com/services/oauth2/token".to_string());
        let http_client = self.http_client.unwrap_or_else(reqwest::Client::new);

        Ok(JwtBearerFlow {
            client_id,
            username,
            private_key,
            audience,
            token_url,
            http_client,
        })
    }
}

/// OAuth error response.
#[cfg(feature = "jwt")]
#[derive(Debug, Deserialize)]
struct OAuthErrorResponse {
    error: String,
    error_description: String,
}

#[cfg(all(test, feature = "jwt"))]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    use super::*;
    use crate::auth::Authenticator;

    // Test RSA key pair for testing (DO NOT use in production)
    const TEST_PRIVATE_KEY: &str = r"-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQCk2XyObD+F8vk9
cTEBJXymtVVFKG/IlXvwY8aPDsjog4O3/iUtmEgQss0JbckPqEAso1GOXeCoC2Sw
mDN3PdTxWIN864BbKI3aR/jzcPPstTK6QpxGiwXyM0Q3Dyi7fmBOxtDRwYTPB/aP
mbsXXhW1ZJM8Fd5pDZSBN9MX41jipwm76MUHyViPyiu5tXcsiMtNDo7KpB7mbHOt
RIHDGTuloxaT7sUVyauNWCHLNlUr5OsIUyS2LrtKeSUu5sWzhE+hrpQF7cjyDYJD
HbRgjAQs2AStI0axhcbawrBdX0TkU9/RFKVzslPm2T1l1clCeip3640ONZsbgz5j
RqPSfP5vAgMBAAECggEASOJNcyzB8yencbZra63WzGAs6KxFrAIHb5O1lMd9JWwM
Hxuy9VM4PYXIKGyNMip54SJ+KvsvmiybYoaQbp58WQ6A6Ai5UdR+zyz2ES/18Mh2
Oqq7rGbIBLsM5GkD4c2wp/O4HJ06akyDgx79fInhADeM70pd8MWLzIvRfWTLhj2N
PjOkUSGUOEVP0p/2SRyzMzOcnmhOujDFMM1yXqmPDGdPQWfVkhJa2umQfqd3dMbe
j8kXtQDhcA9GWm1I0dqpDV9eJg0WZgTXMB1yb0tZjoKNHNx0KPQguwG35bFVuz+X
CYE81kdGb0xIsMFH2FIxQY0yLGAN+bMnnQ0pApLCFQKBgQDWBtGxTz7FMGhT9zWB
ZUJvFQHx9MiRVOMk+mRiZsZ2aZuvMN6kXpBsx0BWIebCJkhFpAfhgwntCRNgD9Gg
+gGNczXeIoYV+Pjo9QNROJaF4QvtZvJgLmqfAesCnNJg4ARCLe/4eFoCp1d89F8s
xok0gmkiu7l8jLDvQ8R/hYCaXQKBgQDFLbz6Hm/WdDuOJ9AyRXdLeRTRTDiw35XQ
gc8OW0tvfHo6KI+Yf2wOZSassm7xMm3iMXbvKPFkAFIzfdL0GGY+hXUtIZstTnpa
zmwSQ8BjHcUkHL7GA2JQ4Vees9JIJw5xBhtNmTVr5Nk0oNHHfgWGD5qrgtCh4l54
T21g/tZnOwKBgDyqEiW/4HrkDa4/E9tpaDs0KSj7yR3ogbmpf2qk1vwZUxeFMpZE
d4tdrs67LT06vKGArPsuuVGGkQdZdIG8W1RMo6gjAP6ZY3Qkfpw2/fNUppzT4T+B
6JbJZGOJL9hlps9bVfmHo3u9Ev9IBPIcFCfeDw7ZRuoWttAa1UeP/7PBAoGAESSy
44Q18Q1WCDwJ6/UCNDuoxbG81BP8cI54tCTX4C+QaPIR2g5qFK5SuH0jDDF4QExQ
rOaAZlNo0jVEXBiq+xCbaXschMnn9XExED13wqZZ95PQOmMc7y9IcPHtfHx40vbW
9N43ONRC1kKNOq0ISemdZwAOp6SI1ikBt4cwmPUCgYEAv2S66uf1hO832lYjPjwv
JGmrmoxGzif0L840eWGb4lJ2relNe6Z5o0Z2a15HVq1wuRh3k09sfnn6bkhPQda7
g1FZTFRZVk+gGC+cHE9oq10Gk/upIGx+4kx/vG5qIg5zBqpzRKCRh5D7+/+pp1uh
QcWLHR6ul3bFRWNhXoThNBQ=
-----END PRIVATE KEY-----";

    #[test]
    fn test_jwt_bearer_builder_missing_client_id() {
        let result = JwtBearerFlow::builder()
            .username("user@example.com")
            .private_key(TEST_PRIVATE_KEY)
            .build();

        assert!(result.is_err());
        if let Err(ForceError::Config(crate::error::ConfigError::MissingValue(field))) = result {
            assert_eq!(field, "client_id");
        } else {
            panic!("Expected ConfigError::MissingValue");
        }
    }

    #[test]
    fn test_jwt_bearer_builder_missing_username() {
        let result = JwtBearerFlow::builder()
            .client_id("test_client")
            .private_key(TEST_PRIVATE_KEY)
            .build();

        assert!(result.is_err());
        if let Err(ForceError::Config(crate::error::ConfigError::MissingValue(field))) = result {
            assert_eq!(field, "username");
        } else {
            panic!("Expected ConfigError::MissingValue");
        }
    }

    #[test]
    fn test_jwt_bearer_builder_missing_private_key() {
        let result = JwtBearerFlow::builder()
            .client_id("test_client")
            .username("user@example.com")
            .build();

        assert!(result.is_err());
        if let Err(ForceError::Config(crate::error::ConfigError::MissingValue(field))) = result {
            assert_eq!(field, "private_key");
        } else {
            panic!("Expected ConfigError::MissingValue");
        }
    }

    #[test]
    fn test_jwt_bearer_builder_invalid_private_key() {
        let result = JwtBearerFlow::builder()
            .client_id("test_client")
            .username("user@example.com")
            .private_key("invalid key")
            .build();

        assert!(result.is_err());
        if let Err(ForceError::Authentication(AuthenticationError::InvalidJwtConfig(msg))) = result
        {
            assert!(msg.contains("Invalid RSA private key"));
        } else {
            panic!("Expected InvalidJwtConfig error");
        }
    }

    #[test]
    fn test_jwt_bearer_debug_redacts_private_key() {
        let flow = JwtBearerFlow::builder()
            .client_id("test_client")
            .username("user@example.com")
            .private_key(TEST_PRIVATE_KEY)
            .build()
            .unwrap();

        let debug_str = format!("{flow:?}");
        assert!(debug_str.contains("test_client"));
        assert!(debug_str.contains("user@example.com"));
        assert!(!debug_str.contains("BEGIN RSA PRIVATE KEY"));
        assert!(debug_str.contains("[REDACTED]"));
    }

    #[test]
    fn test_generate_jwt() {
        let flow = JwtBearerFlow::builder()
            .client_id("test_client_id")
            .username("test@example.com")
            .private_key(TEST_PRIVATE_KEY)
            .audience("https://test.salesforce.com")
            .build()
            .unwrap();

        let jwt = flow.generate_jwt().unwrap();
        assert!(!jwt.is_empty());

        // JWT should have 3 parts separated by dots
        assert_eq!(jwt.split('.').count(), 3);
    }

    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn test_jwt_bearer_authenticate_success() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        let token_response = serde_json::json!({
            "access_token": "jwt_bearer_token",
            "instance_url": "https://test.salesforce.com",
            "id": "https://login.salesforce.com/id/00Dxx/005xx",
            "token_type": "Bearer",
            "issued_at": "1704067200000",
            "signature": "sig=="
        });

        Mock::given(method("POST"))
            .and(path("/services/oauth2/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(token_response))
            .mount(&mock_server)
            .await;

        let flow = JwtBearerFlow::builder()
            .client_id("test_client")
            .username("test@example.com")
            .private_key(TEST_PRIVATE_KEY)
            .token_url(format!("{}/services/oauth2/token", mock_server.uri()))
            .build()
            .unwrap();

        let token = flow.authenticate().await.unwrap();
        assert_eq!(token.as_str(), "jwt_bearer_token");
        assert_eq!(token.instance_url(), "https://test.salesforce.com");
    }

    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn test_jwt_bearer_authenticate_oauth_error() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        let error_response = serde_json::json!({
            "error": "invalid_grant",
            "error_description": "user hasn't approved this consumer"
        });

        Mock::given(method("POST"))
            .and(path("/services/oauth2/token"))
            .respond_with(ResponseTemplate::new(400).set_body_json(error_response))
            .mount(&mock_server)
            .await;

        let flow = JwtBearerFlow::builder()
            .client_id("test_client")
            .username("test@example.com")
            .private_key(TEST_PRIVATE_KEY)
            .token_url(format!("{}/services/oauth2/token", mock_server.uri()))
            .build()
            .unwrap();

        let result = flow.authenticate().await;
        assert!(result.is_err());

        if let Err(ForceError::Authentication(AuthenticationError::TokenRequestFailed(msg))) =
            result
        {
            assert!(msg.contains("invalid_grant"));
            assert!(msg.contains("user hasn't approved"));
        } else {
            panic!("Expected TokenRequestFailed error");
        }
    }

    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn test_jwt_bearer_refresh_calls_authenticate() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        let token_response = serde_json::json!({
            "access_token": "refreshed_jwt_token",
            "instance_url": "https://test.salesforce.com",
            "token_type": "Bearer",
            "issued_at": "1704067200000",
            "signature": "sig=="
        });

        Mock::given(method("POST"))
            .and(path("/services/oauth2/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(token_response))
            .expect(2) // Called twice: authenticate + refresh
            .mount(&mock_server)
            .await;

        let flow = JwtBearerFlow::builder()
            .client_id("test_client")
            .username("test@example.com")
            .private_key(TEST_PRIVATE_KEY)
            .token_url(format!("{}/services/oauth2/token", mock_server.uri()))
            .build()
            .unwrap();

        // First authenticate
        let _token1 = flow.authenticate().await.unwrap();

        // Then refresh (should call authenticate again)
        let token2 = flow.refresh().await.unwrap();
        assert_eq!(token2.as_str(), "refreshed_jwt_token");
    }
}
