# force-sync

`force-sync` is the Postgres-first sync engine for bidirectional Salesforce and Postgres convergence.

## Scope

This crate owns the sync control plane: capture, planning, task leasing, apply routing, reconciliation, and recovery state. Postgres is the only v0.1 backend.

## Identity

Canonical identity is `(tenant, object_name, external_id)`. Salesforce IDs and local database IDs are derived links, not primary identity.

## Required Schema

Run the embedded migrations before using the crate:

- `sync_journal`
- `sync_link`
- `sync_task`
- `sync_checkpoint`
- `sync_conflict`
- `sync_dead_letter`
- `sync_export_watermark`
- `force_sync_schema_migrations`
- `force_sync_outbox`

## Outbox Contract

Outbox rows must carry a raw, non-empty source cursor and a valid operation/tombstone pair. The current contract rejects:

- empty tenant, object, or external ID
- already-encoded cursors such as `postgres-lsn:...`
- contradictory `op` / `tombstone` combinations

Malformed rows are quarantined into `sync_dead_letter` and marked processed so one poison row does not wedge the batch.

## Runtime Entry Points

The runtime is explicit, not magical:

- `run_capture_postgres_once()`
- `run_apply_once()`
- `run_reconcile_once()`

These are available through `SyncEngine` in [`src/runtime.rs`](src/runtime.rs).

## Durable Runtime (feature `durable`)

The explicit runtime leaves scheduling, retries and crash recovery to you.
The opt-in `durable` feature runs the same pipeline on
[autumn-harvest](https://crates.io/crates/autumn-harvest), a Postgres-backed
durable workflow engine. See [ADR-036](../../docs/adr/036-durable-sync-on-autumn-harvest.md).

```toml
force-sync = { version = "0.5", features = ["durable"] }
```

```rust,ignore
use force_sync::durable::{DurableSync, DurableSyncConfig, harvest_pool, migrate_harvest};

migrate_harvest(&harvest_url).await?;
let runtime = DurableSync::new(engine, harvest_pool(&harvest_url, 16)?)
    .with_config(DurableSyncConfig::default())?
    .start()?;
// ... until shutdown ...
runtime.stop().await?;
```

What it does:

- A **pump** schedule (`force_sync_pump`, every `pump_interval`) captures
  outbox rows, leases ready `sync_task` rows, and starts one
  `force_sync_apply` workflow per task, with ID `force-sync-apply-{task_id}`.
- Each apply workflow runs **prepare**, the **Salesforce call**, and
  **finalize** as three activities. Harvest records each result, so a crash
  after the Salesforce call resumes at finalize and does not call Salesforce
  again.
- Transient Salesforce errors retry with `remote_retry` backoff. When the
  retries run out, the task fails with `Salesforce call retries exhausted: ...`.
  A permanent error fails the task at once.
- At most one apply runs at a time for each `(tenant, object, external_id)`.
- A **reconcile** schedule (`force_sync_reconcile`, every
  `reconcile_interval`) enqueues repair tasks for drift.
- With `with_cdc`, each Salesforce Change Data Capture stream gets one
  long-running `force_sync_cdc_stream` workflow. It reads bounded windows
  from the stream checkpoint, and each event commits its journal row, apply
  task and checkpoint together. A restart resumes from the checkpoint with no
  lost or duplicate event. An event that can never be captured, such as one
  without the external ID field, goes to `sync_dead_letter`. The
  `force_sync_cdc_supervisor` schedule keeps every stream workflow running.

```rust,ignore
use std::sync::Arc;
use force_pubsub::{PubSubConfig, PubSubHandler};
use force_sync::{CdcStart, CdcStream};

let pubsub = PubSubHandler::connect(client.session(), PubSubConfig::default()).await?;
let runtime = DurableSync::new(engine, harvest_pool(&harvest_url, 16)?)
    .with_cdc(
        Arc::new(pubsub),
        [CdcStream::new("acme", "Account").starting_at(CdcStart::Earliest)],
    )?
    .start()?;
```

`CdcStream::new` uses the standard channel (`/data/AccountChangeEvent`, or
`/data/Invoice__ChangeEvent` for `Invoice__c`). `with_topic` picks a custom
channel. Without harvest, `SyncEngine::run_cdc_window` runs one window.

`start()` runs a core harvest worker and scheduler, with no web framework.
To add durable sync to an app that already runs harvest, call
`DurableSync::register(builder)`, and make your worker poll the
`force-sync` queue (`durable::FORCE_SYNC_QUEUE`).

The harvest tables can live in the force-sync database or in a separate one.
The feature needs Rust 1.88 or later. Do not run the explicit runtime on the
same store with the durable lease owner.

## Non-Goals For v0.1

- SQLite support
- direct warehouse sinks
- generic storage plugins
- a background supervisor hidden behind the explicit API (the `durable`
  feature is the opt-in supervisor)

Warehouse delivery should consume from Postgres downstream.

## Example

See [`examples/postgres_to_salesforce.rs`](examples/postgres_to_salesforce.rs) for a compile-checked vertical slice,
and [`examples/durable_sync.rs`](examples/durable_sync.rs) for the durable runtime.
