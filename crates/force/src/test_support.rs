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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_must_result_ok() {
        let result: Result<i32, &str> = Ok(42);
        assert_eq!(result.must(), 42);
    }

    #[test]
    #[should_panic(expected = "unexpected Err: \"error message\"")]
    fn test_must_result_err() {
        let result: Result<i32, &str> = Err("error message");
        let _ = result.must();
    }

    #[test]
    fn test_must_option_some() {
        let option: Option<i32> = Some(42);
        assert_eq!(option.must(), 42);
    }

    #[test]
    #[should_panic(expected = "unexpected None")]
    fn test_must_option_none() {
        let option: Option<i32> = None;
        let _ = option.must();
    }

    #[test]
    fn test_must_msg_result_ok() {
        let result: Result<i32, &str> = Ok(42);
        assert_eq!(result.must_msg("Custom panic message"), 42);
    }

    #[test]
    #[should_panic(expected = "Custom panic message: \"error message\"")]
    fn test_must_msg_result_err() {
        let result: Result<i32, &str> = Err("error message");
        let _ = result.must_msg("Custom panic message");
    }

    #[test]
    fn test_must_msg_option_some() {
        let option: Option<i32> = Some(42);
        assert_eq!(option.must_msg("Custom panic message"), 42);
    }

    #[test]
    #[should_panic(expected = "Custom panic message")]
    fn test_must_msg_option_none() {
        let option: Option<i32> = None;
        let _ = option.must_msg("Custom panic message");
    }

    #[tokio::test]
    async fn test_mock_authenticator() {
        let auth = MockAuthenticator::new("my_token", "https://mock.salesforce.com");
        let token = auth.authenticate().await.must();
        assert_eq!(token.as_str(), "my_token");
        assert_eq!(token.instance_url(), "https://mock.salesforce.com");

        let refresh_token = auth.refresh().await.must();
        assert_eq!(refresh_token.as_str(), "my_token");
        assert_eq!(refresh_token.instance_url(), "https://mock.salesforce.com");
    }
}
