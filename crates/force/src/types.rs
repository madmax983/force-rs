//! Core Salesforce types.
//!
//! This module contains fundamental types used throughout the Salesforce API,
//! including IDs, API versions, and other domain primitives.

pub mod api_version;
pub mod common;
pub mod query;
pub mod salesforce_id;
pub mod sobject;
pub mod validator;

pub use api_version::{ApiVersion, ApiVersionSupportTier};
pub use common::{ApiError, CreateResponse, DeleteResponse, UpdateResponse, UpsertResponse};
pub use query::{QueryIterator, QueryLocator, QueryResult};
pub use salesforce_id::SalesforceId;
pub use sobject::{Attributes, DynamicSObject};

// Re-exports for backward compatibility (moved to auth)
#[deprecated(since = "0.2.0", note = "Import directly from `force::auth` instead")]
pub use crate::auth::{AccessToken, Authenticator, TokenResponse};
