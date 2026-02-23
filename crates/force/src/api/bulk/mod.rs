//! Bulk API 2.0 handler for Salesforce.
//!
//! This module provides the `BulkHandler` which serves as the foundation for all
//! Bulk API 2.0 operations including ingest jobs and bulk queries.

pub mod handler;
pub mod ingest;
pub mod policy;
pub mod query;
pub mod types;

#[cfg(feature = "bulk")]
pub mod csv;

#[cfg(feature = "bulk")]
pub mod smart_ingest;

pub use handler::BulkHandler;
pub use policy::BulkPollPolicy;
pub use query::BulkQueryStream;
