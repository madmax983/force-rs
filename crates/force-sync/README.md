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

## Non-Goals For v0.1

- SQLite support
- direct warehouse sinks
- generic storage plugins
- a background supervisor or queue daemon hidden behind the API

Warehouse delivery should consume from Postgres downstream.

## Example

See [`examples/postgres_to_salesforce.rs`](examples/postgres_to_salesforce.rs) for a compile-checked vertical slice.
