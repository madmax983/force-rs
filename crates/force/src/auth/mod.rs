//! Authentication for Salesforce APIs.
//!
//! This module provides traits and implementations for various OAuth 2.0 flows
//! supported by Salesforce, including:
//!
//! - Client Credentials (machine-to-machine)
//! - JWT Bearer (server-to-server with certificates)
//! - Username-Password (legacy, not recommended)
//! - Refresh Token (session extension)
//!
//! # Features
//!
//! - `jwt`: Enables JWT bearer token flow (requires `jsonwebtoken` dependency)

pub(crate) mod authenticator;
pub(crate) mod client_credentials;
#[cfg(feature = "data_cloud")]
pub(crate) mod data_cloud;
#[cfg(feature = "jwt")]
pub(crate) mod jwt_bearer;
pub(crate) mod token;
pub(crate) mod token_manager;

pub use authenticator::Authenticator;
pub use client_credentials::ClientCredentials;
#[cfg(feature = "data_cloud")]
pub use data_cloud::{DataCloudAuthenticator, DataCloudConfig};
#[cfg(feature = "jwt")]
pub use jwt_bearer::JwtBearerFlow;
pub use token::{AccessToken, TokenResponse};
pub use token_manager::TokenManager;
