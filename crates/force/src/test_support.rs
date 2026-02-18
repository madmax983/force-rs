//! Test-only helper utilities for ergonomic assertions without `unwrap`/`expect`.

use core::fmt::Debug;

use async_trait::async_trait;

use crate::auth::{AccessToken, Authenticator, TokenResponse};
use crate::error::Result as ForceResult;

/// Extension trait for unwrapping `Result`/`Option` in tests without `unwrap()`.
pub trait Must<T> {
    /// Extracts the inner value or panics with a default diagnostic message.
    fn must(self) -> T;
}

impl<T, E: Debug> Must<T> for std::result::Result<T, E> {
    fn must(self) -> T {
        match self {
            Ok(value) => value,
            Err(error) => panic!("unexpected Err: {error:?}"),
        }
    }
}

impl<T> Must<T> for Option<T> {
    fn must(self) -> T {
        match self {
            Some(value) => value,
            None => panic!("unexpected None"),
        }
    }
}

/// Extension trait for unwrapping with custom panic messages.
pub trait MustMsg<T> {
    /// Extracts the inner value or panics with `message`.
    fn must_msg(self, message: &str) -> T;
}

impl<T, E: Debug> MustMsg<T> for std::result::Result<T, E> {
    fn must_msg(self, message: &str) -> T {
        match self {
            Ok(value) => value,
            Err(error) => panic!("{message}: {error:?}"),
        }
    }
}

impl<T> MustMsg<T> for Option<T> {
    fn must_msg(self, message: &str) -> T {
        match self {
            Some(value) => value,
            None => panic!("{message}"),
        }
    }
}

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
