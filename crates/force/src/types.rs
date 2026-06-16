//! Core Salesforce types.
//!
//! This module contains fundamental types used throughout the Salesforce API,
//! including IDs, API versions, and other domain primitives.

pub(crate) mod api_version;
pub(crate) mod common;
pub(crate) mod describe;
pub(crate) mod query;
pub(crate) mod salesforce_id;
pub(crate) mod sobject;
pub mod validator;

pub use crate::error::ApiError;
pub use api_version::{ApiVersion, ApiVersionSupportTier};
pub use common::{CreateResponse, DeleteResponse, UpdateResponse, UpsertResponse};
pub use describe::{
    ChildRelationship, FieldDescribe, FieldType, FilteredLookupInfo, GlobalDescribe,
    GlobalSObjectDescribe, PicklistValue, RecordTypeInfo, SObjectDescribe,
};
pub use query::{QueryIterator, QueryLocator, QueryResult};
pub use salesforce_id::SalesforceId;
pub use sobject::{Attributes, DynamicSObject};

// Re-exports for backward compatibility (moved to auth)
#[deprecated(since = "0.2.0", note = "Import directly from `force::auth` instead")]
pub use crate::auth::{AccessToken, Authenticator, TokenResponse};
pub(crate) mod explain;
pub use explain::{ExplainResponse, PlanNote, QueryPlan};
