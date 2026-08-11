//! Snapshot orchestration driver.
//!
//! [`SnapshotSink`] ties the pipeline together: it describes a Salesforce object,
//! derives the Iceberg + Arrow schemas, bulk-queries every field, assembles Arrow
//! record batches, encodes them to Parquet, and commits the result through a
//! [`LakeCatalog`].
//!
//! The pure, Salesforce-free core of the pipeline is factored into
//! [`SnapshotSink::snapshot_from_records`], which the public
//! [`SnapshotSink::snapshot_object`] delegates to after fetching records. Tests
//! exercise the pure core directly with a [`crate::catalog::MockCatalog`] and
//! hand-fed records — no live org required.

use force::api::RestOperation;
use force::auth::Authenticator;
use force::client::ForceClient;
use serde_json::Value;

use crate::catalog::{LakeCatalog, SnapshotData};
use crate::config::LakeConfig;
use crate::error::Result;
use crate::parquet_writer::write_parquet;
use crate::record_batch::build_record_batch;
use crate::schema_map::{MappedSchema, map_schema};

/// Outcome of snapshotting a single Salesforce object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotReport {
    /// The Salesforce object that was snapshotted.
    pub object: String,
    /// The total number of records written.
    pub record_count: usize,
    /// The number of Arrow record batches produced.
    pub batch_count: usize,
    /// The size in bytes of the encoded Parquet payload.
    pub parquet_bytes: usize,
}

/// One-way Salesforce → Iceberg snapshot sink.
///
/// Generic over the authenticator `A` of the underlying [`ForceClient`] and the
/// catalog `C` snapshots are committed through.
#[derive(Debug, Clone)]
pub struct SnapshotSink<A: Authenticator, C: LakeCatalog> {
    client: ForceClient<A>,
    config: LakeConfig,
    catalog: C,
}

impl<A: Authenticator, C: LakeCatalog> SnapshotSink<A, C> {
    /// Creates a new snapshot sink.
    #[must_use]
    pub fn new(client: ForceClient<A>, config: LakeConfig, catalog: C) -> Self {
        Self {
            client,
            config,
            catalog,
        }
    }

    /// The sink's configuration.
    #[must_use]
    pub fn config(&self) -> &LakeConfig {
        &self.config
    }

    /// The catalog snapshots are committed through.
    #[must_use]
    pub fn catalog(&self) -> &C {
        &self.catalog
    }

    /// Snapshots a single Salesforce object end-to-end.
    ///
    /// Describes the object, derives its schema, bulk-queries all fields, and
    /// commits the encoded snapshot through the configured catalog.
    ///
    /// # Errors
    ///
    /// Returns an error if describe, the bulk query, schema derivation, Parquet
    /// encoding, or the catalog commit fails.
    pub async fn snapshot_object(&self, sobject: &str) -> Result<SnapshotReport> {
        let describe = self.client.rest().describe(sobject).await?;
        let mapped = map_schema(&describe)?;

        // ⚡ Bolt: Construct comma-separated string dynamically to avoid intermediate Vec allocation from `.join()`
        let mut field_list = String::with_capacity({
            if describe.fields.is_empty() {
                0
            } else {
                describe.fields.iter().map(|f| f.name.len()).sum::<usize>()
                    + (describe.fields.len() - 1) * 2
            }
        });
        for (i, field) in describe.fields.iter().enumerate() {
            if i > 0 {
                field_list.push_str(", ");
            }
            field_list.push_str(&field.name);
        }
        let soql = format!("SELECT {field_list} FROM {sobject}");

        let mut stream = self.client.bulk().query::<Value>(&soql).await?;
        let mut records = Vec::new();
        while let Some(record) = stream.next().await? {
            records.push(record);
        }

        self.snapshot_from_records(sobject, &mapped, &records).await
    }

    /// Pure snapshot core: batches records, encodes Parquet, and commits.
    ///
    /// This contains no Salesforce access and is the primary unit-test seam.
    ///
    /// # Errors
    ///
    /// Returns an error if record-batch assembly, Parquet encoding, or the
    /// catalog `ensure_table` / `commit_snapshot` calls fail.
    pub async fn snapshot_from_records(
        &self,
        sobject: &str,
        mapped: &MappedSchema,
        records: &[Value],
    ) -> Result<SnapshotReport> {
        let arrow_schema = mapped.arrow();
        let batch_size = self.config.batch_size();

        let mut batches = Vec::new();
        if records.is_empty() {
            // Still emit an empty batch so the Parquet file carries the schema.
            batches.push(build_record_batch(&arrow_schema, &[])?);
        } else {
            for chunk in records.chunks(batch_size) {
                batches.push(build_record_batch(&arrow_schema, chunk)?);
            }
        }

        let parquet = write_parquet(&arrow_schema, &batches)?;

        self.catalog.ensure_table(sobject, mapped.iceberg()).await?;
        let data = SnapshotData::new(sobject, parquet, records.len());
        self.catalog.commit_snapshot(&data).await?;

        Ok(SnapshotReport {
            object: sobject.to_owned(),
            record_count: records.len(),
            batch_count: batches.len(),
            parquet_bytes: data.parquet.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::MockCatalog;
    use crate::config::LakeConfig;
    use crate::schema_map::map_schema;
    use crate::test_fixtures::{describe_json, field_json};
    use force::auth::ClientCredentials;
    use force::types::SObjectDescribe;
    use serde_json::json;

    // Builds a real ForceClient offline. `build()` performs no network I/O —
    // authentication is lazy — so the pure snapshot core never touches the org.
    async fn offline_sink(
        cfg: LakeConfig,
        catalog: MockCatalog,
    ) -> SnapshotSink<ClientCredentials, MockCatalog> {
        let auth = ClientCredentials::new(
            "id",
            "secret",
            "https://login.salesforce.com/services/oauth2/token",
        );
        let client = force::client::builder()
            .authenticate(auth)
            .build()
            .await
            .expect("client builds offline");
        SnapshotSink::new(client, cfg, catalog)
    }

    fn describe() -> SObjectDescribe {
        serde_json::from_value(describe_json(
            "Opportunity",
            vec![
                field_json("Id", "id", json!({"nillable": false})),
                field_json("Name", "string", json!({"nillable": true})),
                field_json(
                    "Amount",
                    "currency",
                    json!({"nillable": true, "precision": 18, "scale": 2}),
                ),
            ],
        ))
        .expect("describe fixture")
    }

    fn config(batch_size: usize) -> LakeConfig {
        LakeConfig::builder()
            .namespace("analytics")
            .table_bucket_arn("arn:aws:s3tables:us-east-1:1:bucket/lake")
            .warehouse("s3://lake/wh")
            .region("us-east-1")
            .batch_size(batch_size)
            .build()
            .expect("config")
    }

    #[tokio::test]
    async fn snapshots_records_into_batches_and_commits() {
        let mapped = map_schema(&describe()).expect("mapped");
        let catalog = MockCatalog::new();
        let sink = offline_sink(config(2), catalog.clone()).await;
        let records = vec![
            json!({"Id": "001a", "Name": "Acme", "Amount": "100.00"}),
            json!({"Id": "001b", "Name": "Globex", "Amount": null}),
            json!({"Id": "001c", "Name": "Initech", "Amount": "42.50"}),
        ];

        let report = sink
            .snapshot_from_records("Opportunity", &mapped, &records)
            .await
            .expect("snapshot");

        assert_eq!(report.record_count, 3);
        // 3 records, batch size 2 => 2 batches.
        assert_eq!(report.batch_count, 2);
        assert!(report.parquet_bytes > 0);

        let log = catalog.log();
        assert_eq!(log.ensured, vec!["Opportunity".to_owned()]);
        assert_eq!(log.committed.len(), 1);
        assert_eq!(log.committed[0].0, "Opportunity");
        assert_eq!(log.committed[0].1, 3);
    }

    #[tokio::test]
    async fn empty_records_still_commit_schema_only_snapshot() {
        let mapped = map_schema(&describe()).expect("mapped");
        let catalog = MockCatalog::new();
        let sink = offline_sink(config(1000), catalog.clone()).await;

        let report = sink
            .snapshot_from_records("Opportunity", &mapped, &[])
            .await
            .expect("snapshot");

        assert_eq!(report.record_count, 0);
        assert_eq!(report.batch_count, 1);
        assert_eq!(catalog.log().committed.len(), 1);
    }
}
