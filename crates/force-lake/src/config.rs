//! Configuration for the Salesforce → S3 Tables / Iceberg snapshot sink.
//!
//! [`LakeConfig`] describes *where* snapshots land (the S3 Tables catalog /
//! table-bucket and namespace) and *what* is snapshotted (target objects, an
//! optional partition column and period). Build one with [`LakeConfig::builder`].
//!
//! # S3 Tables constraints
//!
//! Amazon S3 Tables exposes an Iceberg REST catalog with a few constraints that
//! shape this configuration:
//!
//! * **Single-level namespaces.** S3 Tables namespaces are flat — a namespace is
//!   a single identifier, not a dotted multi-level path. [`LakeConfig::namespace`]
//!   is therefore a single [`String`].
//! * **Table-bucket scoped.** Tables live inside a *table bucket* addressed by an
//!   ARN; [`LakeConfig::table_bucket_arn`] carries it.
//! * **No `CREATE TABLE AS SELECT`.** Tables must be created explicitly, then
//!   data files appended — see the `catalog` module.

use crate::error::{LakeError, Result};

/// Partition granularity for period-scoped snapshot refreshes.
///
/// When a [`LakeConfig`] specifies a partition column, this selects the temporal
/// bucket used for full-partition overwrite semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PartitionPeriod {
    /// One partition per calendar day.
    Day,
    /// One partition per calendar month.
    Month,
    /// One partition per calendar year.
    Year,
}

/// Immutable configuration for a [`crate::snapshot::SnapshotSink`].
#[derive(Debug, Clone)]
pub struct LakeConfig {
    namespace: String,
    table_bucket_arn: String,
    warehouse: String,
    region: String,
    target_objects: Vec<String>,
    partition_column: Option<String>,
    partition_period: Option<PartitionPeriod>,
    batch_size: usize,
}

impl LakeConfig {
    /// Default number of records assembled into a single Arrow `RecordBatch`.
    pub const DEFAULT_BATCH_SIZE: usize = 10_000;

    /// Starts building a [`LakeConfig`].
    #[must_use]
    pub fn builder() -> LakeConfigBuilder {
        LakeConfigBuilder::default()
    }

    /// The single-level S3 Tables namespace snapshots are written to.
    #[must_use]
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// The ARN of the S3 Tables table bucket that hosts the namespace.
    #[must_use]
    pub fn table_bucket_arn(&self) -> &str {
        &self.table_bucket_arn
    }

    /// The catalog warehouse location / identifier.
    #[must_use]
    pub fn warehouse(&self) -> &str {
        &self.warehouse
    }

    /// The AWS region of the table bucket.
    #[must_use]
    pub fn region(&self) -> &str {
        &self.region
    }

    /// The Salesforce objects configured for snapshotting.
    #[must_use]
    pub fn target_objects(&self) -> &[String] {
        &self.target_objects
    }

    /// The optional partition column (typically a Salesforce datetime field).
    #[must_use]
    pub fn partition_column(&self) -> Option<&str> {
        self.partition_column.as_deref()
    }

    /// The optional partition period paired with [`Self::partition_column`].
    #[must_use]
    pub fn partition_period(&self) -> Option<PartitionPeriod> {
        self.partition_period
    }

    /// The record batch size used when assembling Arrow record batches.
    #[must_use]
    pub fn batch_size(&self) -> usize {
        self.batch_size
    }
}

/// Builder for [`LakeConfig`].
#[derive(Debug, Default, Clone)]
pub struct LakeConfigBuilder {
    namespace: Option<String>,
    table_bucket_arn: Option<String>,
    warehouse: Option<String>,
    region: Option<String>,
    target_objects: Vec<String>,
    partition_column: Option<String>,
    partition_period: Option<PartitionPeriod>,
    batch_size: Option<usize>,
}

impl LakeConfigBuilder {
    /// Sets the single-level S3 Tables namespace.
    #[must_use]
    pub fn namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    /// Sets the S3 Tables table-bucket ARN.
    #[must_use]
    pub fn table_bucket_arn(mut self, arn: impl Into<String>) -> Self {
        self.table_bucket_arn = Some(arn.into());
        self
    }

    /// Sets the catalog warehouse location / identifier.
    #[must_use]
    pub fn warehouse(mut self, warehouse: impl Into<String>) -> Self {
        self.warehouse = Some(warehouse.into());
        self
    }

    /// Sets the AWS region of the table bucket.
    #[must_use]
    pub fn region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    /// Adds a single Salesforce object to the snapshot target set.
    #[must_use]
    pub fn target_object(mut self, object: impl Into<String>) -> Self {
        self.target_objects.push(object.into());
        self
    }

    /// Replaces the target object set with the provided iterator.
    #[must_use]
    pub fn target_objects<I, S>(mut self, objects: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.target_objects = objects.into_iter().map(Into::into).collect();
        self
    }

    /// Sets the partition column and period for period-scoped refreshes.
    #[must_use]
    pub fn partition(mut self, column: impl Into<String>, period: PartitionPeriod) -> Self {
        self.partition_column = Some(column.into());
        self.partition_period = Some(period);
        self
    }

    /// Overrides the record batch size (default [`LakeConfig::DEFAULT_BATCH_SIZE`]).
    #[must_use]
    pub fn batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = Some(batch_size);
        self
    }

    /// Finalizes the [`LakeConfig`].
    ///
    /// # Errors
    ///
    /// Returns [`LakeError::Config`] when a required field (namespace,
    /// table-bucket ARN, warehouse, or region) is missing, or when the batch
    /// size is zero.
    pub fn build(self) -> Result<LakeConfig> {
        let namespace = self
            .namespace
            .ok_or_else(|| LakeError::config("namespace is required"))?;
        if namespace.contains('.') {
            return Err(LakeError::config(
                "S3 Tables namespaces are single-level; `namespace` must not contain '.'",
            ));
        }
        let table_bucket_arn = self
            .table_bucket_arn
            .ok_or_else(|| LakeError::config("table_bucket_arn is required"))?;
        let warehouse = self
            .warehouse
            .ok_or_else(|| LakeError::config("warehouse is required"))?;
        let region = self
            .region
            .ok_or_else(|| LakeError::config("region is required"))?;
        let batch_size = self.batch_size.unwrap_or(LakeConfig::DEFAULT_BATCH_SIZE);
        if batch_size == 0 {
            return Err(LakeError::config("batch_size must be greater than zero"));
        }

        Ok(LakeConfig {
            namespace,
            table_bucket_arn,
            warehouse,
            region,
            target_objects: self.target_objects,
            partition_column: self.partition_column,
            partition_period: self.partition_period,
            batch_size,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> LakeConfigBuilder {
        LakeConfig::builder()
            .namespace("analytics")
            .table_bucket_arn("arn:aws:s3tables:us-east-1:123456789012:bucket/lake")
            .warehouse("s3://lake/warehouse")
            .region("us-east-1")
    }

    #[test]
    fn builds_with_defaults() {
        let cfg = base().build().unwrap();
        assert_eq!(cfg.namespace(), "analytics");
        assert_eq!(cfg.batch_size(), LakeConfig::DEFAULT_BATCH_SIZE);
        assert!(cfg.partition_column().is_none());
    }

    #[test]
    fn rejects_missing_namespace() {
        let err = LakeConfig::builder()
            .table_bucket_arn("arn")
            .warehouse("w")
            .region("r")
            .build()
            .unwrap_err();
        assert!(matches!(err, LakeError::Config(_)));
    }

    #[test]
    fn rejects_multilevel_namespace() {
        let err = base().namespace("a.b").build().unwrap_err();
        assert!(matches!(err, LakeError::Config(_)));
    }

    #[test]
    fn rejects_zero_batch_size() {
        let err = base().batch_size(0).build().unwrap_err();
        assert!(matches!(err, LakeError::Config(_)));
    }

    #[test]
    fn captures_partition_and_targets() {
        let cfg = base()
            .target_objects(["Account", "Opportunity"])
            .partition("CloseDate", PartitionPeriod::Month)
            .build()
            .unwrap();
        assert_eq!(cfg.target_objects(), ["Account", "Opportunity"]);
        assert_eq!(cfg.partition_column(), Some("CloseDate"));
        assert_eq!(cfg.partition_period(), Some(PartitionPeriod::Month));
    }
}
