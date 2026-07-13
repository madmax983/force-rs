//! SOAP fault type and parsing.
//!
//! A SOAP fault is a whole-call failure returned as HTTP 500 with a
//! `<soapenv:Fault>` body. It is distinct from the per-record errors carried
//! inside `SaveResult`/`UpsertResult`/`DeleteResult`, which are successful
//! HTTP 200 responses.

use std::fmt;

/// The machine exception code Salesforce reports for an expired or invalid
/// session. When a fault carries this code the handler transparently refreshes
/// the OAuth token and retries the call once.
pub const INVALID_SESSION_ID: &str = "INVALID_SESSION_ID";

/// A SOAP Partner API fault.
///
/// This is the payload for [`ForceError::Soap`](crate::error::ForceError::Soap).
/// It represents a whole-call failure such as a malformed SOQL query, an
/// invalid field, or an invalid session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoapFault {
    /// The namespaced fault code, for example `sf:MALFORMED_QUERY` or
    /// `soapenv:Client`.
    pub fault_code: String,
    /// The human-readable fault message.
    pub fault_string: String,
    /// The machine exception code from the fault detail, for example
    /// `INVALID_SESSION_ID`, when present.
    pub exception_code: Option<String>,
}

impl SoapFault {
    /// Returns `true` when this fault indicates an expired or invalid session.
    #[must_use]
    pub fn is_invalid_session(&self) -> bool {
        self.exception_code.as_deref() == Some(INVALID_SESSION_ID)
            || self.fault_code.contains(INVALID_SESSION_ID)
            || self.fault_string.contains(INVALID_SESSION_ID)
    }
}

impl fmt::Display for SoapFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.fault_code, self.fault_string)
    }
}

impl std::error::Error for SoapFault {}

/// Parses a `<soapenv:Fault>` from a SOAP response body.
///
/// Returns `None` when the body does not contain a fault (so the caller can
/// fall through to parsing a normal operation response).
pub fn parse_fault(xml: &str) -> Option<SoapFault> {
    let root = super::parse::parse_document(xml).ok()?;
    let fault = root.find_descendant("Fault")?;

    let fault_code = fault.child_text("faultcode").unwrap_or_default();
    let fault_string = fault.child_text("faultstring").unwrap_or_default();

    // The exception code lives at detail/<...>Fault/exceptionCode. Search the
    // whole detail subtree for the first `exceptionCode` leaf.
    let exception_code = fault
        .find_child("detail")
        .and_then(|detail| detail.find_descendant("exceptionCode"))
        .map(super::parse::Node::text_owned)
        .filter(|s| !s.is_empty());

    Some(SoapFault {
        fault_code,
        fault_string,
        exception_code,
    })
}
