//! Experimental features.
//!
//! # Deprecation
//!
//! `SmartIngest` has been promoted to `crate::api::bulk::smart_ingest`.

#[cfg(feature = "bulk")]
#[deprecated(since = "0.2.0", note = "Use `crate::api::bulk::smart_ingest` instead")]
pub use crate::api::bulk::smart_ingest;

#[cfg(feature = "rest")]
pub(crate) mod scanner;

#[cfg(feature = "composite")]
pub mod query_batch;

#[cfg(feature = "nova")]
pub(crate) mod schema_graph;

#[cfg(feature = "nova")]
pub mod data_dictionary;

#[cfg(feature = "nova")]
#[allow(missing_docs)]
pub mod type_generator;

#[cfg(feature = "nova")]
pub(crate) mod schema_diff;
