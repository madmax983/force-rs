//! Catalog abstraction for committing Salesforce snapshots as Iceberg tables.
//!
//! [`LakeCatalog`] is the seam between the pure snapshot pipeline (describe →
//! schema → record batches → Parquet) and a physical Iceberg catalog. Two
//! implementations are provided:
//!
//! * [`MockCatalog`] — an in-memory double that records every call, used by the
//!   snapshot orchestration tests.
//! * [`S3TablesCatalog`] — a real catalog binding backed by the dedicated
//!   `iceberg-catalog-s3tables` crate (SigV4-signed Amazon S3 Tables REST).
//!
//! # S3 Tables and the append commit
//!
//! [`S3TablesCatalog::from_config`] builds the real
//! [`iceberg_catalog_s3tables::S3TablesCatalog`] from a [`LakeConfig`]
//! (table-bucket ARN + single-level namespace), and [`S3TablesCatalog`] drives it
//! through iceberg-rust's generic [`iceberg::Catalog`] trait so the rest of the
//! pipeline stays catalog-agnostic. [`S3TablesCatalog::ensure_table`] creates the
//! (single-level) namespace and table, and [`S3TablesCatalog::commit_snapshot`]
//! stages the Parquet payload to the table's data location via
//! [`iceberg::io::FileIO`], then builds the Iceberg [`iceberg::spec::DataFile`]
//! manifest entry and issues a `fast_append` metadata commit through the catalog.
//!
//! Enabling the real S3 Tables catalog required bumping the workspace MSRV to
//! rustc 1.92 (see ADR-030).
//!
//! Snapshots are **append / full-partition overwrite** only: iceberg-rust 0.9 has
//! no row-level (positional / equality) delete support here, so per-row deletes
//! are out of scope and deletes are modelled as full-partition rewrites.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use iceberg::spec::{
    DataContentType, DataFileBuilder, DataFileFormat, Schema as IcebergSchema, Struct,
};
use iceberg::transaction::{ApplyTransactionAction, Transaction};
use iceberg::{Catalog, CatalogBuilder, NamespaceIdent, TableCreation, TableIdent};
use iceberg_catalog_s3tables::{S3TABLES_CATALOG_PROP_TABLE_BUCKET_ARN, S3TablesCatalogBuilder};

use crate::config::LakeConfig;
use crate::error::{LakeError, Result};

/// A staged snapshot payload ready to be committed to a table.
#[derive(Debug, Clone)]
pub struct SnapshotData {
    /// The target table name (single object per table).
    pub table: String,
    /// The encoded Parquet bytes for this snapshot.
    pub parquet: Vec<u8>,
    /// The number of records represented by [`Self::parquet`].
    pub record_count: usize,
}

impl SnapshotData {
    /// Creates a new staged snapshot payload.
    #[must_use]
    pub fn new(table: impl Into<String>, parquet: Vec<u8>, record_count: usize) -> Self {
        Self {
            table: table.into(),
            parquet,
            record_count,
        }
    }
}

/// A catalog capable of provisioning tables and committing snapshots.
#[async_trait]
pub trait LakeCatalog: Send + Sync {
    /// Ensures the target table exists with the given Iceberg schema.
    ///
    /// Implementations should be idempotent: creating the table when absent and
    /// otherwise treating an existing table as success.
    ///
    /// # Errors
    ///
    /// Returns an error if the namespace or table cannot be provisioned.
    async fn ensure_table(&self, table: &str, schema: &IcebergSchema) -> Result<()>;

    /// Commits a staged snapshot to the target table.
    ///
    /// The v0.1 semantics are append / full-partition overwrite; row-level
    /// deletes are out of scope (see the crate documentation).
    ///
    /// # Errors
    ///
    /// Returns an error if the snapshot cannot be staged or committed.
    async fn commit_snapshot(&self, data: &SnapshotData) -> Result<()>;
}

/// Records of the calls made against a [`MockCatalog`].
#[derive(Debug, Default, Clone)]
pub struct MockCatalogLog {
    /// Tables passed to `ensure_table`, in call order.
    pub ensured: Vec<String>,
    /// `(table, record_count, parquet_len)` for each `commit_snapshot` call.
    pub committed: Vec<(String, usize, usize)>,
}

/// In-memory [`LakeCatalog`] test double that records every interaction.
#[derive(Debug, Default, Clone)]
pub struct MockCatalog {
    log: Arc<Mutex<MockCatalogLog>>,
}

impl MockCatalog {
    /// Creates an empty mock catalog.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a snapshot of the recorded call log.
    ///
    /// # Panics
    ///
    /// Panics only if the internal lock has been poisoned by a prior panic.
    #[must_use]
    pub fn log(&self) -> MockCatalogLog {
        self.log
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

#[async_trait]
impl LakeCatalog for MockCatalog {
    async fn ensure_table(&self, table: &str, _schema: &IcebergSchema) -> Result<()> {
        {
            let mut log = self
                .log
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            log.ensured.push(table.to_owned());
        }
        Ok(())
    }

    async fn commit_snapshot(&self, data: &SnapshotData) -> Result<()> {
        {
            let mut log = self
                .log
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            log.committed
                .push((data.table.clone(), data.record_count, data.parquet.len()));
        }
        Ok(())
    }
}

/// Real Iceberg catalog binding for Amazon S3 Tables.
///
/// Backed by the dedicated `iceberg-catalog-s3tables` crate and driven through
/// the generic [`iceberg::Catalog`] trait. Build one from a [`LakeConfig`] with
/// [`S3TablesCatalog::from_config`], or wrap an arbitrary catalog with
/// [`S3TablesCatalog::new`]. All tables live in a single-level namespace,
/// matching the S3 Tables constraint.
#[derive(Clone)]
pub struct S3TablesCatalog {
    catalog: Arc<dyn Catalog>,
    namespace: NamespaceIdent,
}

impl std::fmt::Debug for S3TablesCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3TablesCatalog")
            .field("namespace", &self.namespace)
            .finish_non_exhaustive()
    }
}

impl S3TablesCatalog {
    /// Creates a catalog binding for a single-level `namespace`.
    ///
    /// # Errors
    ///
    /// Returns [`LakeError::Config`] if `namespace` is empty or multi-level.
    pub fn new(catalog: Arc<dyn Catalog>, namespace: &str) -> Result<Self> {
        if namespace.is_empty() {
            return Err(LakeError::config("namespace must not be empty"));
        }
        if namespace.contains('.') {
            return Err(LakeError::config(
                "S3 Tables namespaces are single-level and must not contain '.'",
            ));
        }
        Ok(Self {
            catalog,
            namespace: NamespaceIdent::new(namespace.to_owned()),
        })
    }

    /// Builds a live Amazon S3 Tables catalog binding from a [`LakeConfig`].
    ///
    /// Constructs the real [`iceberg_catalog_s3tables::S3TablesCatalog`] from the
    /// configured table-bucket ARN and single-level namespace. AWS credentials
    /// and region are resolved from the ambient environment by the AWS SDK.
    ///
    /// # Errors
    ///
    /// Returns [`LakeError::Config`] if the namespace is empty or multi-level, or
    /// [`LakeError::Iceberg`] if the S3 Tables catalog cannot be initialised.
    pub async fn from_config(config: &LakeConfig) -> Result<Self> {
        let props = HashMap::from([(
            S3TABLES_CATALOG_PROP_TABLE_BUCKET_ARN.to_owned(),
            config.table_bucket_arn().to_owned(),
        )]);
        let catalog = S3TablesCatalogBuilder::default()
            .load("s3tables", props)
            .await?;
        Self::new(Arc::new(catalog), config.namespace())
    }

    fn table_ident(&self, table: &str) -> TableIdent {
        TableIdent::new(self.namespace.clone(), table.to_owned())
    }
}

#[async_trait]
impl LakeCatalog for S3TablesCatalog {
    async fn ensure_table(&self, table: &str, schema: &IcebergSchema) -> Result<()> {
        if !self.catalog.namespace_exists(&self.namespace).await? {
            self.catalog
                .create_namespace(&self.namespace, HashMap::new())
                .await?;
        }

        let ident = self.table_ident(table);
        if self.catalog.table_exists(&ident).await? {
            return Ok(());
        }

        let creation = TableCreation::builder()
            .name(table.to_owned())
            .schema(schema.clone())
            .build();
        self.catalog.create_table(&self.namespace, creation).await?;
        Ok(())
    }

    async fn commit_snapshot(&self, data: &SnapshotData) -> Result<()> {
        let ident = self.table_ident(&data.table);
        let table = self.catalog.load_table(&ident).await?;

        // Physical staging: write the Parquet payload into the table's data
        // directory via the table's configured FileIO. This is real I/O.
        let location = table.metadata().location();
        let data_path = format!("{location}/data/{}.parquet", data_file_name(data));
        let output = table.file_io().new_output(&data_path)?;
        output
            .write(bytes::Bytes::from(data.parquet.clone()))
            .await?;

        // Build the Iceberg DataFile manifest entry for the staged Parquet file.
        // The table is unpartitioned at v0.1, so the partition tuple is empty;
        // record count and file size are the metrics S3 Tables requires. Richer
        // per-column stats (bounds / null counts) are left empty for now.
        let record_count = u64::try_from(data.record_count).unwrap_or(u64::MAX);
        let file_size = u64::try_from(data.parquet.len()).unwrap_or(u64::MAX);
        let data_file = DataFileBuilder::default()
            .content(DataContentType::Data)
            .file_path(data_path)
            .file_format(DataFileFormat::Parquet)
            .partition(Struct::empty())
            .partition_spec_id(table.metadata().default_partition_spec_id())
            .record_count(record_count)
            .file_size_in_bytes(file_size)
            .build()
            .map_err(|e| {
                iceberg::Error::new(
                    iceberg::ErrorKind::DataInvalid,
                    format!("failed to build Iceberg data file: {e}"),
                )
            })?;

        // Append-only commit: fast_append writes a new manifest + snapshot
        // referencing the staged data file, then commits the metadata update
        // through the S3 Tables catalog. No existing data files are rewritten.
        let tx = Transaction::new(&table);
        let action = tx.fast_append().add_data_files(vec![data_file]);
        let tx = action.apply(tx)?;
        tx.commit(self.catalog.as_ref()).await?;
        Ok(())
    }
}

/// Derives a unique-per-commit Parquet file name for the staged snapshot without
/// pulling in a UUID dependency. Combines the current wall-clock time (nanos)
/// with the record count and payload length to avoid collisions across commits.
fn data_file_name(data: &SnapshotData) -> String {
    let nanos = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    format!("{nanos}-{}-{}", data.record_count, data.parquet.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema_map::iceberg_schema_from_describe;
    use crate::test_fixtures::{describe_json, field_json};
    use serde_json::json;

    fn schema() -> IcebergSchema {
        let describe = serde_json::from_value(describe_json(
            "Account",
            vec![field_json("Id", "id", json!({"nillable": false}))],
        ))
        .expect("describe");
        iceberg_schema_from_describe(&describe).expect("iceberg schema")
    }

    #[tokio::test]
    async fn mock_records_ensure_and_commit() {
        let catalog = MockCatalog::new();
        catalog
            .ensure_table("Account", &schema())
            .await
            .expect("ensure");
        catalog
            .commit_snapshot(&SnapshotData::new("Account", vec![1, 2, 3], 2))
            .await
            .expect("commit");

        let log = catalog.log();
        assert_eq!(log.ensured, vec!["Account".to_owned()]);
        assert_eq!(log.committed, vec![("Account".to_owned(), 2, 3)]);
    }

    #[test]
    fn rejects_multilevel_namespace() {
        // Construction is validated without needing a live catalog; use the mock
        // as the backing catalog purely to satisfy the type.
        let backing: Arc<dyn Catalog> = Arc::new(NoopCatalog);
        let err = S3TablesCatalog::new(backing, "a.b").unwrap_err();
        assert!(matches!(err, LakeError::Config(_)));
    }

    // Minimal Catalog stub so S3TablesCatalog construction can be type-checked in
    // tests without a live AWS backend. Methods are unreachable in these tests.
    #[derive(Debug)]
    struct NoopCatalog;

    #[async_trait]
    impl Catalog for NoopCatalog {
        async fn list_namespaces(
            &self,
            _parent: Option<&NamespaceIdent>,
        ) -> iceberg::Result<Vec<NamespaceIdent>> {
            Ok(vec![])
        }
        async fn create_namespace(
            &self,
            _namespace: &NamespaceIdent,
            _properties: HashMap<String, String>,
        ) -> iceberg::Result<iceberg::Namespace> {
            Err(iceberg::Error::new(
                iceberg::ErrorKind::FeatureUnsupported,
                "noop",
            ))
        }
        async fn get_namespace(
            &self,
            _namespace: &NamespaceIdent,
        ) -> iceberg::Result<iceberg::Namespace> {
            Err(iceberg::Error::new(
                iceberg::ErrorKind::FeatureUnsupported,
                "noop",
            ))
        }
        async fn namespace_exists(&self, _namespace: &NamespaceIdent) -> iceberg::Result<bool> {
            Ok(false)
        }
        async fn update_namespace(
            &self,
            _namespace: &NamespaceIdent,
            _properties: HashMap<String, String>,
        ) -> iceberg::Result<()> {
            Ok(())
        }
        async fn drop_namespace(&self, _namespace: &NamespaceIdent) -> iceberg::Result<()> {
            Ok(())
        }
        async fn list_tables(
            &self,
            _namespace: &NamespaceIdent,
        ) -> iceberg::Result<Vec<TableIdent>> {
            Ok(vec![])
        }
        async fn create_table(
            &self,
            _namespace: &NamespaceIdent,
            _creation: TableCreation,
        ) -> iceberg::Result<iceberg::table::Table> {
            Err(iceberg::Error::new(
                iceberg::ErrorKind::FeatureUnsupported,
                "noop",
            ))
        }
        async fn load_table(&self, _table: &TableIdent) -> iceberg::Result<iceberg::table::Table> {
            Err(iceberg::Error::new(
                iceberg::ErrorKind::FeatureUnsupported,
                "noop",
            ))
        }
        async fn drop_table(&self, _table: &TableIdent) -> iceberg::Result<()> {
            Ok(())
        }
        async fn table_exists(&self, _table: &TableIdent) -> iceberg::Result<bool> {
            Ok(false)
        }
        async fn rename_table(&self, _src: &TableIdent, _dest: &TableIdent) -> iceberg::Result<()> {
            Ok(())
        }
        async fn register_table(
            &self,
            _table: &TableIdent,
            _metadata_location: String,
        ) -> iceberg::Result<iceberg::table::Table> {
            Err(iceberg::Error::new(
                iceberg::ErrorKind::FeatureUnsupported,
                "noop",
            ))
        }
        async fn update_table(
            &self,
            _commit: iceberg::TableCommit,
        ) -> iceberg::Result<iceberg::table::Table> {
            Err(iceberg::Error::new(
                iceberg::ErrorKind::FeatureUnsupported,
                "noop",
            ))
        }
    }
}
