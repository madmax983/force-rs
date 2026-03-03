//! Test-only helper utilities for ergonomic assertions without `unwrap`/`expect`.

use async_trait::async_trait;

use crate::auth::{AccessToken, Authenticator, TokenResponse};
use crate::error::Result as ForceResult;

/// Mock authenticator for testing.
#[derive(Debug, Clone)]
pub struct MockAuthenticator {
    token: String,
    instance_url: String,
}

impl MockAuthenticator {
    /// Creates a new mock authenticator.
    pub fn new(token: &str, instance_url: &str) -> Self {
        Self {
            token: token.to_string(),
            instance_url: instance_url.to_string(),
        }
    }
}

#[async_trait]
impl Authenticator for MockAuthenticator {
    async fn authenticate(&self) -> ForceResult<AccessToken> {
        Ok(AccessToken::from_response(TokenResponse {
            access_token: self.token.clone(),
            instance_url: self.instance_url.clone(),
            token_type: "Bearer".to_string(),
            issued_at: "1704067200000".to_string(),
            signature: "test_sig".to_string(),
            expires_in: Some(7200),
            refresh_token: None,
        }))
    }

    async fn refresh(&self) -> ForceResult<AccessToken> {
        self.authenticate().await
    }
}
