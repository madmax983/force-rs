#![allow(clippy::redundant_pub_crate)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::similar_names)]
//! Salesforce Marketing Cloud Engagement REST API client.
//!
//! `force-marketingcloud` is a standalone, REST-first client for the Salesforce
//! Marketing Cloud Engagement (formerly ExactTarget) platform. It is intentionally
//! decoupled from the core `force` crate: Marketing Cloud uses a wholly separate
//! authentication model (per-tenant auth subdomain, Installed-Package JSON client
//! credentials, ~20-minute tokens with no refresh token, and business-unit/MID
//! tenancy), so it does not share the core client's session or auth types.
//!
//! # Quick start
//!
//! ```no_run
//! use force_marketingcloud::{MarketingCloudClient, Recipient, SendEmailRequest};
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let client = MarketingCloudClient::builder()
//!     .tenant_subdomain("mc9xxxxxxxxxxxxxxxx")
//!     .client_credentials("client-id", "client-secret")
//!     .account_id("1234567") // optional default business unit (MID)
//!     .build()?;
//!
//! let request = SendEmailRequest::new(
//!     "transactional_welcome",
//!     Recipient::new("contact-001", "jane@example.com"),
//! );
//! let response = client.transactional().send_email("MSG-KEY-123", &request).await?;
//! println!("request id: {:?}", response.request_id);
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// API surface handlers (transactional, assets, contacts, data extensions, journeys).
pub(crate) mod api;
/// Authentication: Installed-Package credentials, tokens, and the token manager.
pub(crate) mod auth;
/// The client and its builder.
pub(crate) mod client;
/// Error types.
pub(crate) mod error;
/// Shared types used across API handlers.
pub(crate) mod types;

/// Returns the crate version for smoke tests and runtime diagnostics.
#[must_use]
pub const fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub use api::assets::{Asset, AssetType, AssetsHandler};
pub use api::contacts::{Contact, ContactsHandler};
pub use api::data_extensions::{AsyncRowsResponse, DataExtensionsHandler, Row};
pub use api::journeys::{FireEventResponse, InteractionEvent, Journey, JourneysHandler};
pub use api::transactional::{
    MessageResponseItem, Recipient, SendEmailRequest, SendMessageResponse, SendSmsRequest,
    TransactionalHandler,
};
pub use auth::{
    AccessToken, Authenticator, InstalledPackageCredentials, TokenManager, TokenResponse,
};
pub use client::{MarketingCloudClient, MarketingCloudClientBuilder};
pub use error::{MarketingCloudError, Result};
pub use types::{Attributes, Paged};
