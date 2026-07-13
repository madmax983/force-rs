//! Salesforce SOAP Partner API handler.
//!
//! This module implements the classic, untyped [SOAP Partner API]. It reuses
//! the client's OAuth token — placed in the SOAP `SessionHeader` as the
//! `sessionId` — rather than the retiring SOAP `login()` call, and speaks XML
//! over the `/services/Soap/u/{version}` endpoint.
//!
//! # Feature flag
//!
//! This module requires the `soap` feature:
//! ```toml
//! [dependencies]
//! force = { version = "...", features = ["soap"] }
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use force::api::soap::SObject;
//!
//! let client = builder().authenticate(auth).build().await?;
//! let soap = client.soap();
//!
//! // Query records
//! let result = soap.query("SELECT Id, Name FROM Account LIMIT 10").await?;
//! for record in &result.records {
//!     println!("{:?}", record.get("Name"));
//! }
//!
//! // Create a record
//! let account = SObject::new("Account").with_field("Name", "Acme Corp");
//! let saves = soap.create(&[account]).await?;
//! assert!(saves[0].success);
//! ```
//!
//! # Typed convenience layer
//!
//! For callers who prefer their own structs over the generic [`SObject`] field
//! bag, the `*_typed` methods ([`query_typed`](SoapHandler::query_typed),
//! [`retrieve_typed`](SoapHandler::retrieve_typed),
//! [`create_typed`](SoapHandler::create_typed),
//! [`update_typed`](SoapHandler::update_typed), and
//! [`upsert_typed`](SoapHandler::upsert_typed)) bridge serde types to and from
//! `SObject` and delegate to the untyped calls.
//!
//! # Error model
//!
//! Whole-call failures (a `<soapenv:Fault>`, an `INVALID_SESSION_ID`, or an XML
//! parse error) surface as [`ForceError`](crate::error::ForceError). Per-record
//! failures are **not** errors: a partially-failed `create` returns
//! `Ok(Vec<SaveResult>)` where individual results carry `success = false` and a
//! populated `errors` list.
//!
//! [SOAP Partner API]: https://developer.salesforce.com/docs/atlas.en-us.api.meta/api/

pub(crate) mod crud;
pub(crate) mod describe;
pub(crate) mod envelope;
pub(crate) mod fault;
pub(crate) mod misc;
pub(crate) mod parse;
pub(crate) mod query;
pub(crate) mod typed;
pub(crate) mod types;

pub use fault::SoapFault;
pub use types::{
    DeleteResult, DescribeGlobalResult, DescribeGlobalSObject, DescribeSObjectResult,
    FieldDescribe, PicklistEntry, QueryResult, SObject, SaveResult, SearchResult, SoapError,
    UpsertResult, UserInfo,
};

use crate::error::{HttpError, Result};
use std::sync::Arc;

/// The `Content-Type` header value required by the SOAP endpoint.
const CONTENT_TYPE: &str = "text/xml; charset=UTF-8";
/// The `SOAPAction` header value. The Partner endpoint requires the header but
/// ignores its value; a quoted empty string is universally accepted.
const SOAP_ACTION: &str = "\"\"";
/// The maximum SOAP response body size read into memory (100 MiB).
const BODY_CAP_BYTES: usize = 100 * 1024 * 1024;
/// The default `CallOptions` client identifier.
const DEFAULT_CLIENT: &str = "force-rs";

/// The outcome of a single SOAP round-trip, before session-retry handling.
enum Dispatch {
    /// A successful operation response body.
    Ok(String),
    /// A whole-call fault that is not a session error.
    Fault(SoapFault),
    /// An `INVALID_SESSION_ID` fault eligible for a token refresh and retry.
    InvalidSession(SoapFault),
}

/// Handler for the Salesforce SOAP Partner API.
///
/// Obtain one from [`ForceClient::soap`](crate::client::ForceClient::soap).
#[derive(Debug)]
pub struct SoapHandler<A: crate::auth::Authenticator> {
    inner: Arc<crate::session::Session<A>>,
    client: Option<String>,
}

impl<A: crate::auth::Authenticator> Clone for SoapHandler<A> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            client: self.client.clone(),
        }
    }
}

impl<A: crate::auth::Authenticator> SoapHandler<A> {
    /// Creates a new SOAP handler wrapping the given session.
    #[must_use]
    pub(crate) fn new(inner: Arc<crate::session::Session<A>>) -> Self {
        Self {
            inner,
            client: Some(DEFAULT_CLIENT.to_string()),
        }
    }

    /// Overrides the `CallOptions` client identifier (≤ 32 characters).
    ///
    /// Pass `None` to omit the `CallOptions` header entirely.
    #[must_use]
    pub fn with_client(mut self, client: Option<impl Into<String>>) -> Self {
        self.client = client.map(Into::into);
        self
    }

    /// Builds the SOAP endpoint URL, stripping the `v` prefix from the API version.
    fn soap_url(&self, instance_url: &str) -> String {
        let version = self.inner.config.api_version.trim_start_matches('v');
        let base = instance_url.trim_end_matches('/');
        let mut url = String::with_capacity(base.len() + version.len() + 16);
        url.push_str(base);
        url.push_str("/services/Soap/u/");
        url.push_str(version);
        url
    }

    /// Sends a SOAP operation body, handling faults and a single session retry.
    ///
    /// Because Salesforce reports an expired session as an HTTP 500
    /// `INVALID_SESSION_ID` fault (not a 401), the shared 401-refresh middleware
    /// does not fire for SOAP. This method therefore refreshes the token and
    /// retries once on such a fault, mirroring the REST 401 retry intent.
    pub(crate) async fn send(&self, body: &str) -> Result<String> {
        match self.dispatch(body).await? {
            Dispatch::Ok(text) => Ok(text),
            Dispatch::Fault(fault) => Err(fault.into()),
            Dispatch::InvalidSession(_) => {
                self.inner.token_manager.force_refresh().await?;
                match self.dispatch(body).await? {
                    Dispatch::Ok(text) => Ok(text),
                    Dispatch::Fault(fault) | Dispatch::InvalidSession(fault) => Err(fault.into()),
                }
            }
        }
    }

    /// Performs one SOAP round-trip and classifies the response.
    async fn dispatch(&self, body: &str) -> Result<Dispatch> {
        let token = self.inner.token_manager.get_token_arc().await?;
        let url = self.soap_url(token.instance_url());
        let envelope = envelope::build_envelope(token.as_str(), self.client.as_deref(), body);

        let request = self
            .inner
            .post(&url)
            .header(reqwest::header::CONTENT_TYPE, CONTENT_TYPE)
            .header("SOAPAction", SOAP_ACTION)
            .body(envelope)
            .build()
            .map_err(HttpError::from)?;

        let response = self.inner.execute_request(request).await?;
        let status = response.status();
        let text = crate::http::error::read_capped_body(response, BODY_CAP_BYTES).await?;

        if let Some(fault) = fault::parse_fault(&text) {
            if fault.is_invalid_session() {
                return Ok(Dispatch::InvalidSession(fault));
            }
            return Ok(Dispatch::Fault(fault));
        }

        if !status.is_success() {
            return Err(HttpError::StatusError {
                status_code: status.as_u16(),
                message: text,
            }
            .into());
        }

        Ok(Dispatch::Ok(text))
    }
}

#[cfg(test)]
mod tests;
