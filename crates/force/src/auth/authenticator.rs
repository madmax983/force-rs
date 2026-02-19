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
