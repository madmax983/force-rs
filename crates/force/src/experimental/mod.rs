//! Experimental features.
//!
//! # Deprecation
//!
//! `SmartIngest` has been promoted to `crate::api::bulk::smart_ingest`.

#[cfg(feature = "bulk")]
#[deprecated(since = "0.2.0", note = "Use `crate::api::bulk::smart_ingest` instead")]
pub use crate::api::bulk::smart_ingest;
