# force-lake

> One-way Salesforce → S3 Tables / Apache Iceberg analytics snapshot sink

`force-lake` snapshots Salesforce objects into Apache Iceberg tables (targeting
Amazon S3 Tables) for analytics. It is a **sink**, not a sync engine: data flows
one way, Salesforce → lake, and the lake is never read back to drive Salesforce.

## Scope

- **Snapshot, not CDC.** The first cut materializes a full snapshot per object via
  the Bulk API. Change-data-capture ingestion (via `force-pubsub`) is a documented
  follow-up and shares no backfill path with the snapshot flow.
- **Append / full-partition overwrite only.** iceberg-rust does not yet offer
  native row-level deletes from Rust, so this sink appends data files (and, for
  period-scoped refreshes, overwrites whole partitions). Row-level upsert / `MERGE`
  is deferred to a documented Athena-`MERGE` path.
- **No row-level deletes.** Deletes are handled by full-partition overwrite, not
  Iceberg delete files.

See [ADR-028](../../docs/adr/028-force-lake-crate.md) for the full design.

## Pipeline

```text
describe → Iceberg schema → Arrow schema → Bulk query → RecordBatch
         → Parquet bytes → LakeCatalog::commit_snapshot
```

The canonical Salesforce → Iceberg type mapping lives in the `force` crate
(`force::schema::generate_iceberg_schema`, feature `schema`); `force-lake`
consumes it and adds the Arrow / Parquet / catalog machinery.

## Quick example

```rust,no_run
use force_lake::{LakeConfig, MockCatalog, SnapshotSink};

# async fn run<A: force::auth::Authenticator>(client: force::client::ForceClient<A>)
# -> force_lake::Result<()> {
let config = LakeConfig::builder()
    .namespace("analytics") // S3 Tables namespaces are single-level
    .table_bucket_arn("arn:aws:s3tables:us-east-1:123456789012:bucket/lake")
    .warehouse("s3://lake/warehouse")
    .region("us-east-1")
    .target_objects(["Account", "Opportunity"])
    .build()?;

let sink = SnapshotSink::new(client, config, MockCatalog::new());
let report = sink.snapshot_object("Account").await?;
println!("wrote {} records in {} batch(es)", report.record_count, report.batch_count);
# Ok(())
# }
```

## Catalog implementations

- `MockCatalog` — in-memory test double that records every call.
- `S3TablesCatalog` — real binding on iceberg-rust's generic `iceberg::Catalog`
  trait. `ensure_table` creates the namespace + table; `commit_snapshot` stages
  the Parquet payload to the table's data location via `FileIO`. Wiring the
  Iceberg `DataFile` manifest append is the documented final step (the dedicated
  `iceberg-catalog-s3tables` crate currently exceeds this workspace's MSRV — see
  ADR-028).

## License

MIT OR Apache-2.0
