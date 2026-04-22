//! Bulk API 2.0 handler for Salesforce.
//!
//! This module provides the `BulkHandler` which serves as the foundation for all
//! Bulk API 2.0 operations including ingest jobs and bulk queries.

pub(crate) mod handler;
pub(crate) mod ingest;
pub(crate) mod policy;
pub(crate) mod query;
pub(crate) mod types;

#[cfg(feature = "bulk")]
pub(crate) mod csv;

#[cfg(feature = "bulk")]
pub(crate) mod smart_ingest;

pub use handler::BulkHandler;
pub use ingest::{InProgress, IngestJob, JobComplete, Open, UploadComplete};
pub use policy::BulkPollPolicy;
pub use query::{BulkQueryJobInfo, BulkQueryRequest, BulkQueryStream};
pub use types::{
    ContentType, CreateJobRequest, JobInfo, JobOperation, JobState, LineEnding, UpdateJobRequest,
};

#[cfg(feature = "bulk")]
pub use csv::{
    deserialize_from_csv, process_csv_batches, serialize_to_csv, serialize_to_csv_with_options,
};

#[cfg(feature = "bulk")]
pub use smart_ingest::{DEFAULT_MAX_UPLOAD_BYTES, SmartIngest, SmartIngestResult};
