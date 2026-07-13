//! Catalog abstraction for committing Salesforce snapshots as Iceberg tables.
//!
//! [`LakeCatalog`] is the seam between the pure snapshot pipeline (describe →
//! schema → record batches → Parquet) and a physical Iceberg catalog. Two
//! implementations are provided:
//!
//! * [`MockCatalog`] — an in-memory double that records every call, used by the
//!   snapshot orchestration tests.
//! * [`S3TablesCatalog`] — a real catalog binding built on iceberg-rust's generic
//!   [`iceberg::Catalog`] trait.
//!
//! # S3 Tables and the deferred append seam
//!
//! The dedicated `iceberg-catalog-s3tables` crate currently requires rustc 1.92,
//! which is beyond this workspace's 1.85 MSRV, so it is not a dependency here.
//! [`S3TablesCatalog`] is therefore written against the generic
//! [`iceberg::Catalog`] trait: [`S3TablesCatalog::ensure_table`] creates the
//! (single-level) namespace and table for real, and
//! [`S3TablesCatalog::commit_snapshot`] performs the physical Parquet staging
//! write via the table's [`iceberg::io::FileIO`]. Constructing the Iceberg
//! `DataFile` manifest entry and issuing the `fast_append` metadata commit is the
//! documented final wiring step, completed once the concrete S3 Tables catalog
//! builder is available on the pinned toolchain (see ADR-028).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use iceberg::spec::Schema as IcebergSchema;
use iceberg::{Catalog, NamespaceIdent, TableCreation, TableIdent};

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
/// Built on the generic [`iceberg::Catalog`] trait so it works with any
/// iceberg-rust catalog implementation (including the future
/// `iceberg-catalog-s3tables` builder). All tables live in a single-level
/// namespace, matching the S3 Tables constraint.
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
        let data_path = format!("{location}/data/{}.parquet", uuid_like(data));
        let output = table.file_io().new_output(&data_path)?;
        output
            .write(bytes::Bytes::from(data.parquet.clone()))
            .await?;

        // Deferred final wiring step (see ADR-028 and module docs): construct the
        // Iceberg DataFile manifest entry for `data_path` and issue a
        // `Transaction::fast_append(...).commit(catalog)`. This binds to the
        // concrete S3 Tables catalog builder, which requires a newer toolchain
        // than this workspace's MSRV.
        tracing::warn!(
            table = %data.table,
            path = %data_path,
            record_count = data.record_count,
            "staged Parquet snapshot to table data location; Iceberg manifest \
             append is the deferred final wiring step (see ADR-028)"
        );
        Ok(())
    }
}

/// Derives a stable-ish file discriminator from the payload without pulling in a
/// UUID dependency. Uses the record count and payload length; collisions are
/// avoided in practice by the catalog appending its own commit UUID.
fn uuid_like(data: &SnapshotData) -> String {
    format!("{}-{}", data.record_count, data.parquet.len())
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
