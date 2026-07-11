//! Authentication trait definitions.
//!
//! This module defines the core traits for authentication with Salesforce.

use crate::auth::token::AccessToken;
use crate::error::Result;
use async_trait::async_trait;
use std::fmt::Debug;

/// Core trait for Salesforce authentication.
///
/// Implementors of this trait provide different OAuth flows for authenticating
/// with Salesforce (e.g., client credentials, JWT bearer, username-password).
///
/// # Object Safety
///
/// This trait is object-safe thanks to `async_trait`, enabling dynamic dispatch
/// for authentication strategies.
///
/// # Contract
///
/// Implementations must:
/// - Handle token acquisition and renewal
/// - Return valid `AccessToken` instances on success
/// - Propagate authentication errors appropriately
/// - Be thread-safe (Send + Sync)
///
/// # Examples
///
/// ```ignore
/// use force::auth::Authenticator;
///
/// async fn authenticate_and_use<A: Authenticator>(auth: &A) -> Result<()> {
///     let token = auth.authenticate().await?;
///     // Use token for API requests
///     Ok(())
/// }
/// ```
#[async_trait]
pub trait Authenticator: Debug + Send + Sync {
    /// Authenticate with Salesforce and obtain an access token.
    ///
    /// This method performs the initial authentication flow specific to the
    /// implementation (e.g., OAuth client credentials, JWT bearer).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Credentials are invalid
    /// - Network request fails
    /// - Salesforce returns an authentication error
    /// - Token response cannot be parsed
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let token = authenticator.authenticate().await?;
    /// ```
    async fn authenticate(&self) -> Result<AccessToken>;

    /// Refresh an existing access token.
    ///
    /// This method attempts to refresh an expired or expiring access token.
    /// Not all authentication flows support refresh (e.g., client credentials
    /// requires re-authentication, while refresh token flow can extend sessions).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Refresh token is invalid or expired
    /// - Network request fails
    /// - Salesforce rejects the refresh attempt
    /// - Implementation does not support refresh (should re-authenticate instead)
    ///
    /// # Implementation Notes
    ///
    /// Implementations that do not support refresh should either:
    /// - Call `authenticate()` again, or
    /// - Return `Err(ForceError::Authentication(AuthenticationError::TokenRefreshFailed(...)))`
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let new_token = authenticator.refresh().await?;
    /// ```
    async fn refresh(&self) -> Result<AccessToken>;
}

#[async_trait]
impl<T: ?Sized + Authenticator> Authenticator for Box<T> {
    async fn authenticate(&self) -> Result<AccessToken> {
        (**self).authenticate().await
    }

    async fn refresh(&self) -> Result<AccessToken> {
        (**self).refresh().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;

    #[derive(Debug)]
    struct MockAuthenticator;

    #[async_trait]
    impl Authenticator for MockAuthenticator {
        async fn authenticate(&self) -> Result<AccessToken> {
            Ok(AccessToken::new(
                "auth_token".to_string(),
                "https://test.salesforce.com".to_string(),
                None,
            ))
        }

        async fn refresh(&self) -> Result<AccessToken> {
            Ok(AccessToken::new(
                "refresh_token".to_string(),
                "https://test.salesforce.com".to_string(),
                None,
            ))
        }
    }

    #[tokio::test]
    async fn test_box_authenticator() {
        let boxed_auth: Box<dyn Authenticator> = Box::new(MockAuthenticator);

        let auth_token = boxed_auth.authenticate().await.must();
        assert_eq!(auth_token.as_str(), "auth_token");
        assert_eq!(auth_token.instance_url(), "https://test.salesforce.com");

        let refresh_token = boxed_auth.refresh().await.must();
        assert_eq!(refresh_token.as_str(), "refresh_token");
        assert_eq!(refresh_token.instance_url(), "https://test.salesforce.com");
    }
}
