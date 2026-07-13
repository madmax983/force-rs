//! Authentication for Marketing Cloud Engagement.
//!
//! This module implements the Installed-Package server-to-server (client
//! credentials) flow, along with a proactive, per-business-unit token cache.

pub(crate) mod credentials;
pub(crate) mod token;
pub(crate) mod token_manager;

pub use credentials::{Authenticator, InstalledPackageCredentials};
pub use token::{AccessToken, TokenResponse};
pub use token_manager::TokenManager;
