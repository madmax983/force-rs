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

#[cfg(feature = "bulk")]
pub use csv::{deserialize_from_csv, process_csv_batches, serialize_to_csv};
pub use handler::BulkHandler;
pub use ingest::{InProgress, IngestJob, JobComplete, Open, UploadComplete};
pub use policy::BulkPollPolicy;
pub use query::{BulkQueryJobInfo, BulkQueryRequest, BulkQueryStream};
#[cfg(feature = "bulk")]
pub use smart_ingest::SmartIngest;
pub use types::{
    ContentType, CreateJobRequest, JobInfo, JobOperation, JobState, LineEnding, UpdateJobRequest,
};
