//! Salesforce API handlers.
//!
//! This module provides access to various Salesforce APIs through feature-gated
//! submodules.

#[cfg(feature = "rest")]
pub(crate) mod rest;

#[cfg(feature = "bulk")]
pub(crate) mod bulk;

#[cfg(feature = "composite")]
pub(crate) mod composite;

#[cfg(feature = "rest")]
pub use rest::{
    ChildRelationship, FieldDescribe, FieldType, FilteredLookupInfo, GlobalDescribe,
    GlobalSObjectDescribe, LimitInfo, OrgLimits, PicklistValue, QueryStream, RecordTypeInfo,
    SObjectDescribe, SearchAttributes, SearchQueryBuilder, SearchRecords, SearchResult,
    SoqlQueryBuilder, escape_soql,
};

#[cfg(feature = "bulk")]
pub use bulk::{
    BulkHandler, BulkPollPolicy, BulkQueryJobInfo, BulkQueryRequest, BulkQueryStream, ContentType,
    CreateJobRequest, InProgress, IngestJob, JobComplete, JobInfo, JobOperation, JobState,
    LineEnding, Open, UpdateJobRequest, UploadComplete, deserialize_from_csv, process_csv_batches,
    serialize_to_csv,
};

#[cfg(feature = "bulk")]
pub use bulk::SmartIngest;

#[cfg(feature = "composite")]
pub use composite::{BatchBuilder, BatchResponse, BatchSubResponse};

#[cfg(all(feature = "composite", feature = "nova"))]
pub use composite::{
    Graph, GraphBuilder, GraphErrorResponse, GraphRequest, GraphResponse, GraphResult,
    GraphSubResponse,
};
