# Sibling crates

These are **separate crates** in the `force-rs` workspace, each with its own `Cargo`
dependencies — **not** feature flags of `force`. Add the one you need directly. Some reuse the
core `force` crate for auth (`force-pubsub`, `force-lake`); others are fully standalone
(`force-marketingcloud`). Each links back into the core client where relevant.

## force-pubsub — Pub/Sub API (gRPC)

Streaming gRPC client for the Salesforce **Pub/Sub API**: subscribe to Change Data Capture,
Platform Events, and custom channels (automatic Avro decoding), and publish events with
schema-aware Avro encoding. Includes schema caching, configurable reconnection/backoff, and
replay support. Authenticates by reusing a `force` `Session` (Client Credentials, JWT, etc.).

> **Pub/Sub lives here, in `force-pubsub` — not in `force`.** The old "`pub_sub` feature" note in
> the core `CLAUDE.md` is out of date; there is no such feature on `force`.

**Entry point:** `PubSubHandler::connect(session, config)`, then `subscribe(topic, replay)` /
`publish(topic, events)` / `get_schema(id)` / `get_topic(name)`.

```rust
use force_pubsub::{PubSubConfig, PubSubHandler, PubSubEvent, ReplayPreset};

let handler = PubSubHandler::connect(force_client.session(), PubSubConfig::default()).await?;
let mut stream = handler.subscribe("/data/AccountChangeEvent", ReplayPreset::Latest).await?;
while let Some(item) = stream.next().await {
    if let Ok(PubSubEvent::Event(msg)) = item {
        println!("event_id={}", msg.event_id);
    }
}
```

See [`crates/force-pubsub/README.md`](../../../crates/force-pubsub/README.md) and
[`examples/subscribe_events.rs`](../../../crates/force-pubsub/examples/subscribe_events.rs).
Design: [ADR-018](../../adr/018-force-pubsub-crate.md).

## force-sync — Salesforce ↔ Postgres sync

Correctness-first, **bidirectional** Salesforce ↔ Postgres sync engine. Owns the sync control
plane: capture, planning, task leasing, apply routing, reconciliation, and recovery state.
Canonical identity is `(tenant, object_name, external_id)` — Salesforce IDs and local DB IDs are
derived links, not primary identity. **Postgres is the only v0.1 backend**; SQLite, direct
warehouse sinks, generic storage plugins, and any hidden background supervisor are explicit
non-goals (warehouse delivery should consume from Postgres downstream).

**Entry point:** `SyncEngine` (via `SyncEngineBuilder`) with explicit runtime steps —
`run_capture_postgres_once()`, `run_apply_once()`, `run_reconcile_once()`. Requires the embedded
migrations (`sync_journal`, `sync_link`, `sync_task`, `sync_checkpoint`, … via `migrate`).

See [`crates/force-sync/README.md`](../../../crates/force-sync/README.md) and the compile-checked
vertical slice in
[`examples/postgres_to_salesforce.rs`](../../../crates/force-sync/examples/postgres_to_salesforce.rs).
Design: [ADR-026](../../adr/026-force-sync-crate.md).

## force-lake — Salesforce → Iceberg snapshot sink

**One-way** analytics sink: snapshots Salesforce objects into Apache Iceberg tables (targeting
Amazon S3 Tables). It is a sink, not a sync engine — data flows Salesforce → lake only, and the
lake is never read back to drive Salesforce. The pipeline is
`describe → Iceberg schema → Arrow schema → Bulk query → RecordBatch → Parquet bytes →
LakeCatalog::commit_snapshot`. It **consumes** `force::schema::generate_iceberg_schema` (feature
`schema`) for the canonical Salesforce → Iceberg type mapping. Scope is **snapshot, not CDC**, and
**append / full-partition overwrite only** (no row-level deletes; upsert/`MERGE` is a deferred
Athena path). MSRV 1.92.

**Entry point:** `SnapshotSink::new(client, config, catalog)` then `snapshot_object("Account")`.
Config via `LakeConfig::builder()`; catalog is `MockCatalog` (in-memory test double) or
`S3TablesCatalog` (SigV4-signed S3 Tables REST).

```rust
use force_lake::{LakeConfig, MockCatalog, SnapshotSink};

let config = LakeConfig::builder()
    .namespace("analytics")
    .table_bucket_arn("arn:aws:s3tables:us-east-1:123456789012:bucket/lake")
    .warehouse("s3://lake/warehouse")
    .region("us-east-1")
    .target_objects(["Account", "Opportunity"])
    .build()?;
let sink = SnapshotSink::new(client, config, MockCatalog::new());
let report = sink.snapshot_object("Account").await?;
```

See [`crates/force-lake/README.md`](../../../crates/force-lake/README.md).
Design: [ADR-030](../../adr/030-force-lake-crate.md).

## force-marketingcloud — Marketing Cloud Engagement

Standalone, REST-first client for the Salesforce **Marketing Cloud Engagement** (formerly
ExactTarget) platform. Intentionally **decoupled from `force`**: Marketing Cloud uses a wholly
separate auth model — a per-tenant auth subdomain, Installed-Package JSON client credentials,
~20-minute tokens with **no refresh token**, and business-unit / MID tenancy — so it shares none
of the core client's session or auth types. Covers Transactional Messaging (email/SMS + status),
Content Builder Assets, Contacts, Data Extensions, Journeys (Interaction), plus a raw escape
hatch (`raw_get` / `raw_post` / `raw_request`). SOAP is out of scope.

**Entry point:** `MarketingCloudClient::builder()` (`.tenant_subdomain(...)`,
`.client_credentials(...)`, optional `.account_id(mid)`), then handler accessors like
`.transactional()` and `.journeys()`. Target a specific business unit per call with
`.for_business_unit("MID")`.

```rust
use force_marketingcloud::{MarketingCloudClient, Recipient, SendEmailRequest};

let client = MarketingCloudClient::builder()
    .tenant_subdomain("mc9xxxxxxxxxxxxxxxx")
    .client_credentials("client-id", "client-secret")
    .account_id("1234567") // optional default business unit (MID)
    .build()?;

let request = SendEmailRequest::new(
    "transactional_welcome",
    Recipient::new("contact-001", "jane@example.com"),
);
let response = client.transactional().send_email("MSG-KEY-123", &request).await?;
```

See [`crates/force-marketingcloud/README.md`](../../../crates/force-marketingcloud/README.md).
Design: [ADR-034](../../adr/034-marketing-cloud-engagement-crate.md).
