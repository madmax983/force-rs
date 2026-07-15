//! One-way Salesforce → S3 Tables / Apache Iceberg analytics snapshot sink.
//!
//! `force-lake` periodically snapshots Salesforce objects into Apache Iceberg
//! tables (targeting Amazon S3 Tables) for analytics. It is a *sink*, not a sync
//! engine: data flows one way, Salesforce → lake, and the lake is never read back
//! to drive Salesforce.
//!
//! # Scope
//!
//! * **Snapshot, not CDC.** The first cut materializes a full snapshot per object
//!   via the Bulk API. Change-data-capture ingestion (via `force-pubsub`) is a
//!   documented follow-up and shares no backfill path with the snapshot flow.
//! * **Append / full-partition overwrite only.** iceberg-rust does not yet offer
//!   native row-level deletes from Rust, so this sink appends data files (and, for
//!   period-scoped refreshes, overwrites whole partitions). Row-level
//!   upsert / `MERGE` is deferred to a documented Athena-`MERGE` path.
//!
//! # Pipeline
//!
//! ```text
//! describe → Iceberg schema → Arrow schema → Bulk query → RecordBatch
//!          → Parquet bytes → LakeCatalog::commit_snapshot
//! ```
//!
//! The canonical Salesforce → Iceberg type mapping lives in the `force` crate
//! ([`force::schema::generate_iceberg_schema`]); this crate consumes it and adds
//! the Arrow / Parquet / catalog machinery.
//!
//! # Example
//!
//! ```no_run
//! use force_lake::{LakeConfig, MockCatalog, SnapshotSink};
//!
//! # async fn run<A: force::auth::Authenticator>(client: force::client::ForceClient<A>)
//! # -> force_lake::Result<()> {
//! let config = LakeConfig::builder()
//!     .namespace("analytics")
//!     .table_bucket_arn("arn:aws:s3tables:us-east-1:123456789012:bucket/lake")
//!     .warehouse("s3://lake/warehouse")
//!     .region("us-east-1")
//!     .target_objects(["Account", "Opportunity"])
//!     .build()?;
//!
//! let sink = SnapshotSink::new(client, config, MockCatalog::new());
//! let report = sink.snapshot_object("Account").await?;
//! println!("wrote {} records", report.record_count);
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::redundant_pub_crate)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::option_if_let_else)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub(crate) mod catalog;
pub(crate) mod config;
pub(crate) mod error;
pub(crate) mod parquet_writer;
pub(crate) mod record_batch;
pub(crate) mod schema_map;
pub(crate) mod snapshot;

#[cfg(test)]
pub(crate) mod test_fixtures;

pub use catalog::{LakeCatalog, MockCatalog, MockCatalogLog, S3TablesCatalog, SnapshotData};
pub use config::{LakeConfig, LakeConfigBuilder, PartitionPeriod};
pub use error::{LakeError, Result};
pub use parquet_writer::write_parquet;
pub use record_batch::build_record_batch;
pub use schema_map::{
    MappedSchema, arrow_schema_from_iceberg, iceberg_schema_from_describe, map_schema,
};
pub use snapshot::{SnapshotReport, SnapshotSink};

/// Returns the crate version for smoke tests and runtime diagnostics.
#[must_use]
pub const fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
