# Force Sync Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build `crates/force-sync` as a Postgres-first, correctness-first sync engine for bidirectional Salesforce and PostgreSQL convergence.

**Architecture:** `force-sync` is a separate workspace crate that depends on `force` and `force-pubsub` for Salesforce transport, while owning durable sync semantics itself. PostgreSQL is the only v0.1 backend and serves as both control plane and recovery source through an append-only journal, task queue, checkpoints, link state, conflicts, dead letters, and export watermarks.

**Tech Stack:** Rust 2024, Tokio, `force`, `force-pubsub`, `tokio-postgres`, `deadpool-postgres`, `serde`, `serde_json`, `tracing`, `blake3`, `wiremock`, `proptest`

---

## Prerequisites

- Work from a dedicated worktree or feature branch, not `main`.
- All commands run from repo root: `C:\Users\markm\force-rs`
- For PostgreSQL integration tests, set:

```powershell
$env:FORCE_SYNC_TEST_DATABASE_URL="postgres://postgres:postgres@127.0.0.1:5432/force_sync_test"
```

- If the database does not exist, create it before running the integration tests.

```powershell
createdb force_sync_test
```

- Before claiming anything is complete, run:

```powershell
cargo fmt --all
cargo clippy -p force-sync --all-targets -- -D warnings
cargo test -p force-sync
rg -n "TODO|FIXME|Stub:" crates/force-sync docs/plans/2026-03-25-force-sync-implementation.md
```

Expected:

- `cargo fmt --all` exits `0`
- `cargo clippy ...` exits `0`
- `cargo test -p force-sync` exits `0`
- `rg ...` returns no matches under `crates/force-sync`

## Task 1: Scaffold the Workspace Crate

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/force-sync/Cargo.toml`
- Create: `crates/force-sync/src/lib.rs`
- Create: `crates/force-sync/src/error.rs`
- Create: `crates/force-sync/tests/smoke.rs`

**Step 1: Write the failing smoke test**

```rust
use force_sync::version;

#[test]
fn force_sync_exposes_version() {
    assert_eq!(version(), env!("CARGO_PKG_VERSION"));
}
```

**Step 2: Run test to verify it fails**

Run:

```powershell
cargo test -p force-sync --test smoke
```

Expected: fail because package `force-sync` does not exist yet.

**Step 3: Add the workspace member and minimal crate**

Edit root `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["crates/force", "crates/force-pubsub", "crates/force-sync"]
```

Add workspace dependencies:

```toml
deadpool-postgres = "0.14"
tokio-postgres = "0.7"
blake3 = "1.5"
```

Create `crates/force-sync/Cargo.toml`:

```toml
[package]
name = "force-sync"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
repository.workspace = true
description = "Postgres-first sync engine for bidirectional Salesforce and PostgreSQL convergence"

[lints]
workspace = true

[dependencies]
force = { path = "../force", features = ["rest", "bulk", "composite"] }
force-pubsub = { path = "../force-pubsub" }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
chrono = { workspace = true }
bytes = { workspace = true }
futures = { workspace = true }
deadpool-postgres = { workspace = true }
tokio-postgres = { workspace = true }
blake3 = { workspace = true }

[dev-dependencies]
anyhow = { workspace = true }
wiremock = { workspace = true }
proptest = "1.5"
```

Create `crates/force-sync/src/lib.rs`:

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod error;

/// Returns the crate version.
#[must_use]
pub const fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
```

Create `crates/force-sync/src/error.rs`:

```rust
//! Error types for force-sync.

/// Placeholder error type for initial scaffold.
#[derive(Debug, thiserror::Error)]
pub enum ForceSyncError {
    /// Placeholder.
    #[error("not implemented")]
    NotImplemented,
}
```

**Step 4: Run test to verify it passes**

Run:

```powershell
cargo test -p force-sync --test smoke
```

Expected: PASS

**Step 5: Commit**

```powershell
git add Cargo.toml crates/force-sync
git commit -m "feat: scaffold force-sync crate"
```

## Task 2: Add Core Identity, Config, and Model Types

**Files:**
- Modify: `crates/force-sync/src/lib.rs`
- Modify: `crates/force-sync/src/error.rs`
- Create: `crates/force-sync/src/config.rs`
- Create: `crates/force-sync/src/identity.rs`
- Create: `crates/force-sync/src/model.rs`

**Step 1: Write the failing tests**

Add inline tests for:

- `SyncKey::new()` rejects empty values
- `ObjectSync::new("Account").external_id("External_Id__c")` stores config
- `ChangeEnvelope::payload_hash()` is stable for identical payloads

Example:

```rust
#[test]
fn sync_key_requires_non_empty_parts() {
    let err = SyncKey::new("", "Account", "abc").unwrap_err();
    assert!(err.to_string().contains("tenant"));
}
```

**Step 2: Run the tests to verify they fail**

Run:

```powershell
cargo test -p force-sync sync_key_requires_non_empty_parts
```

Expected: FAIL with missing types/functions.

**Step 3: Write the minimal implementation**

Create `src/identity.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SyncKey {
    tenant: String,
    object_name: String,
    external_id: String,
}
```

Create `src/config.rs` with:

- `ObjectSync`
- `ConflictPolicy`
- `Owner`
- `LaneThresholds`

Create `src/model.rs` with:

- `SourceSystem`
- `ChangeOperation`
- `SourceCursor`
- `ChangeEnvelope`

Use `blake3` for payload hashing:

```rust
pub fn payload_hash(payload: &serde_json::Value) -> [u8; 32] {
    *blake3::hash(payload.to_string().as_bytes()).as_bytes()
}
```

**Step 4: Export the modules from `lib.rs`**

```rust
pub mod config;
pub mod error;
pub mod identity;
pub mod model;
```

**Step 5: Run tests to verify they pass**

Run:

```powershell
cargo test -p force-sync sync_key
cargo test -p force-sync payload_hash
```

Expected: PASS

**Step 6: Commit**

```powershell
git add crates/force-sync/src
git commit -m "feat: add force-sync core model types"
```

## Task 3: Add the Postgres Schema and Migration Runner

**Files:**
- Create: `crates/force-sync/migrations/0001_init.sql`
- Create: `crates/force-sync/src/store/mod.rs`
- Create: `crates/force-sync/src/store/pg/mod.rs`
- Create: `crates/force-sync/src/store/pg/migrate.rs`
- Create: `crates/force-sync/tests/support/mod.rs`
- Create: `crates/force-sync/tests/support/postgres.rs`
- Create: `crates/force-sync/tests/pg_migrations.rs`
- Modify: `crates/force-sync/src/lib.rs`

**Step 1: Write the failing integration test**

Create `tests/pg_migrations.rs`:

```rust
#[tokio::test]
async fn applies_initial_schema() {
    let pool = support::postgres::test_pool().await;
    force_sync::store::pg::migrate(&pool).await.unwrap();

    let client = pool.get().await.unwrap();
    let rows = client
        .query("select to_regclass('public.sync_journal')", &[])
        .await
        .unwrap();

    assert_eq!(rows[0].get::<_, Option<String>>(0).as_deref(), Some("sync_journal"));
}
```

**Step 2: Run the test to verify it fails**

Run:

```powershell
cargo test -p force-sync --test pg_migrations -- --nocapture
```

Expected: FAIL because `store::pg::migrate` and the schema do not exist.

**Step 3: Write the initial migration**

Create `migrations/0001_init.sql` with:

- `sync_journal`
- `sync_link`
- `sync_task`
- `sync_checkpoint`
- `sync_conflict`
- `sync_dead_letter`
- `sync_export_watermark`
- `force_sync_schema_migrations`

Also add indexes:

```sql
create unique index sync_journal_source_cursor_uidx on sync_journal(source, source_cursor);
create unique index sync_link_identity_uidx on sync_link(tenant, object_name, external_id);
create index sync_task_ready_idx on sync_task(status, next_attempt_at, lease_until, priority desc);
create index sync_journal_object_observed_idx on sync_journal(object_name, observed_at);
```

**Step 4: Write the embedded migration runner**

Create `src/store/pg/migrate.rs` with a small embedded runner:

```rust
pub async fn migrate(pool: &deadpool_postgres::Pool) -> Result<(), ForceSyncError> {
    // create migration table if needed
    // check applied version
    // apply include_str!("../../migrations/0001_init.sql")
}
```

Do not pull in a migration framework yet.

**Step 5: Add PostgreSQL test support**

Create `tests/support/postgres.rs` with:

- `test_pool() -> deadpool_postgres::Pool`
- `reset_schema(pool)` helper

Use `FORCE_SYNC_TEST_DATABASE_URL`.

**Step 6: Run the migration test to verify it passes**

Run:

```powershell
cargo test -p force-sync --test pg_migrations -- --nocapture
```

Expected: PASS

**Step 7: Commit**

```powershell
git add crates/force-sync/migrations crates/force-sync/src/store crates/force-sync/tests
git commit -m "feat: add force-sync postgres schema and migrator"
```

## Task 4: Add Pool and Store Bootstrap

**Files:**
- Create: `crates/force-sync/src/store/pg/store.rs`
- Modify: `crates/force-sync/src/store/pg/mod.rs`
- Create: `crates/force-sync/tests/pg_store_bootstrap.rs`

**Step 1: Write the failing test**

```rust
#[tokio::test]
async fn pg_store_can_open_transaction() {
    let pool = support::postgres::test_pool().await;
    force_sync::store::pg::migrate(&pool).await.unwrap();
    let store = force_sync::store::pg::PgStore::new(pool.clone());

    let value: i64 = store.with_client(|client| async move {
        client.query_one("select 1", &[]).await.map(|row| row.get(0))
    }).await.unwrap();

    assert_eq!(value, 1);
}
```

**Step 2: Run the test to verify it fails**

Run:

```powershell
cargo test -p force-sync --test pg_store_bootstrap
```

Expected: FAIL because `PgStore` does not exist.

**Step 3: Implement `PgStore`**

Create `src/store/pg/store.rs`:

```rust
#[derive(Clone)]
pub struct PgStore {
    pool: deadpool_postgres::Pool,
}

impl PgStore {
    pub fn new(pool: deadpool_postgres::Pool) -> Self { Self { pool } }
}
```

Add helpers:

- `pool()`
- `with_client(...)`
- `with_transaction(...)`

**Step 4: Run the bootstrap test to verify it passes**

Run:

```powershell
cargo test -p force-sync --test pg_store_bootstrap
```

Expected: PASS

**Step 5: Commit**

```powershell
git add crates/force-sync/src/store/pg/store.rs crates/force-sync/tests/pg_store_bootstrap.rs
git commit -m "feat: add postgres store bootstrap"
```

## Task 5: Implement Journal Writes and Task Leasing

**Files:**
- Create: `crates/force-sync/src/store/pg/journal.rs`
- Create: `crates/force-sync/src/store/pg/task_queue.rs`
- Modify: `crates/force-sync/src/store/pg/mod.rs`
- Create: `crates/force-sync/tests/journal_tasks.rs`

**Step 1: Write the failing tests**

Cover:

- appending a journal entry creates a row
- duplicate `(source, source_cursor)` is deduped
- enqueuing a task in the same transaction works
- leasing a task marks `lease_owner` and `lease_until`

Example:

```rust
#[tokio::test]
async fn lease_ready_task_claims_it_once() {
    // seed one task
    // lease once => one task returned
    // lease again before expiry => zero tasks returned
}
```

**Step 2: Run the tests to verify they fail**

Run:

```powershell
cargo test -p force-sync --test journal_tasks -- --nocapture
```

Expected: FAIL because repository methods do not exist.

**Step 3: Implement journal repository methods**

Add methods:

- `append_journal(&ChangeEnvelope) -> journal_id`
- `append_journal_if_new(&ChangeEnvelope) -> AppendResult`
- `enqueue_apply_task(journal_id, priority)`

Use explicit SQL.

**Step 4: Implement task queue methods**

Add methods:

- `lease_ready_tasks(worker_id, limit, lease_for)`
- `ack_task(task_id)`
- `retry_task(task_id, next_attempt_at, error)`
- `fail_task(task_id, error)`

Use:

```sql
for update skip locked
```

**Step 5: Run the tests to verify they pass**

Run:

```powershell
cargo test -p force-sync --test journal_tasks -- --nocapture
```

Expected: PASS

**Step 6: Commit**

```powershell
git add crates/force-sync/src/store/pg/journal.rs crates/force-sync/src/store/pg/task_queue.rs crates/force-sync/tests/journal_tasks.rs
git commit -m "feat: add journal and task queue repositories"
```

## Task 6: Implement Links, Checkpoints, Conflicts, and Dead Letters

**Files:**
- Create: `crates/force-sync/src/store/pg/link.rs`
- Create: `crates/force-sync/src/store/pg/checkpoint.rs`
- Create: `crates/force-sync/src/store/pg/conflict.rs`
- Create: `crates/force-sync/src/store/pg/dead_letter.rs`
- Modify: `crates/force-sync/src/store/pg/mod.rs`
- Create: `crates/force-sync/tests/control_plane_repos.rs`

**Step 1: Write the failing tests**

Cover:

- upserting `sync_link`
- monotonic checkpoint update
- inserting a conflict row
- writing a dead-letter row

**Step 2: Run the tests to verify they fail**

Run:

```powershell
cargo test -p force-sync --test control_plane_repos -- --nocapture
```

Expected: FAIL because repo methods do not exist.

**Step 3: Implement the repositories**

Add methods:

- `put_link(...)`
- `get_link(...)`
- `advance_checkpoint_if_greater(...)`
- `insert_conflict(...)`
- `insert_dead_letter(...)`

Checkpoint advancement must be monotonic.

**Step 4: Run the tests to verify they pass**

Run:

```powershell
cargo test -p force-sync --test control_plane_repos -- --nocapture
```

Expected: PASS

**Step 5: Commit**

```powershell
git add crates/force-sync/src/store/pg/link.rs crates/force-sync/src/store/pg/checkpoint.rs crates/force-sync/src/store/pg/conflict.rs crates/force-sync/src/store/pg/dead_letter.rs crates/force-sync/tests/control_plane_repos.rs
git commit -m "feat: add link checkpoint conflict and dead-letter repos"
```

## Task 7: Implement Planner and Merge Logic

**Files:**
- Create: `crates/force-sync/src/plan.rs`
- Modify: `crates/force-sync/src/config.rs`
- Modify: `crates/force-sync/src/model.rs`
- Create: `crates/force-sync/tests/planner.rs`

**Step 1: Write the failing tests**

Cover:

- identical hashes become no-op
- Postgres-owned field wins when Salesforce changes it
- conflict-required field opens a conflict
- large batches choose `Bulk`
- dependent records choose `CompositeGraph`
- urgent singleton changes choose `Rest`

Example:

```rust
#[test]
fn planner_chooses_rest_for_singleton_mutation() {
    let decision = plan_change(&context, &envelope).unwrap();
    assert_eq!(decision.lane, ApplyLane::Rest);
}
```

**Step 2: Run the tests to verify they fail**

Run:

```powershell
cargo test -p force-sync --test planner
```

Expected: FAIL because planner types/functions do not exist.

**Step 3: Implement minimal planner types**

Add:

- `ApplyLane`
- `PlannerContext`
- `PlanDecision`
- `plan_change(...)`
- `merge_payload(...)`

Keep this pure. No database or network access inside planner logic.

**Step 4: Add one property test**

Use `proptest` to prove no-op suppression is stable for duplicate envelopes:

```rust
proptest! {
    #[test]
    fn duplicate_envelopes_plan_to_noop(...) { ... }
}
```

**Step 5: Run the tests to verify they pass**

Run:

```powershell
cargo test -p force-sync --test planner
```

Expected: PASS

**Step 6: Commit**

```powershell
git add crates/force-sync/src/plan.rs crates/force-sync/tests/planner.rs
git commit -m "feat: add planner and merge logic"
```

## Task 8: Add PostgreSQL Outbox Capture

**Files:**
- Create: `crates/force-sync/migrations/0002_force_sync_outbox.sql`
- Create: `crates/force-sync/src/capture/mod.rs`
- Create: `crates/force-sync/src/capture/postgres.rs`
- Modify: `crates/force-sync/src/lib.rs`
- Create: `crates/force-sync/tests/postgres_capture.rs`

**Step 1: Write the failing integration test**

Cover:

- an outbox row is captured into `sync_journal`
- an apply task is enqueued
- the outbox row is marked processed

**Step 2: Run the test to verify it fails**

Run:

```powershell
cargo test -p force-sync --test postgres_capture -- --nocapture
```

Expected: FAIL because outbox migration and capture worker do not exist.

**Step 3: Add the outbox table migration**

Create `0002_force_sync_outbox.sql`:

```sql
create table if not exists force_sync_outbox (
    outbox_id bigserial primary key,
    tenant text not null,
    object_name text not null,
    external_id text not null,
    source_cursor text not null,
    op text not null,
    tombstone boolean not null default false,
    payload jsonb not null,
    created_at timestamptz not null default now(),
    processed_at timestamptz
);
```

**Step 4: Implement the capture worker**

Add `capture::postgres::capture_batch(...)`:

- select unprocessed outbox rows
- append journal rows
- enqueue apply tasks
- mark outbox rows processed
- do all of that in one transaction

**Step 5: Run the test to verify it passes**

Run:

```powershell
cargo test -p force-sync --test postgres_capture -- --nocapture
```

Expected: PASS

**Step 6: Commit**

```powershell
git add crates/force-sync/migrations/0002_force_sync_outbox.sql crates/force-sync/src/capture crates/force-sync/tests/postgres_capture.rs
git commit -m "feat: add postgres outbox capture"
```

## Task 9: Implement Salesforce REST Apply Lane

**Files:**
- Create: `crates/force-sync/src/apply/mod.rs`
- Create: `crates/force-sync/src/apply/salesforce.rs`
- Modify: `crates/force-sync/src/lib.rs`
- Create: `crates/force-sync/tests/rest_apply.rs`

**Step 1: Write the failing tests**

Cover:

- external-ID upsert create path
- external-ID upsert update `204` path without requiring returned Salesforce ID
- delete path
- transient failure surfaces as retryable error

Use `wiremock` and `force::test_support::MockAuthenticator` patterns from the existing crate.

**Step 2: Run the tests to verify they fail**

Run:

```powershell
cargo test -p force-sync --test rest_apply -- --nocapture
```

Expected: FAIL because apply code does not exist.

**Step 3: Implement the minimal REST applier**

Add:

- `SalesforceApplier`
- `apply_rest_upsert(...)`
- `apply_rest_delete(...)`

Rules:

- always use external-ID upsert
- do not treat missing returned Salesforce ID on `204` as failure
- return an apply result that can leave `salesforce_id` as `None`

**Step 4: Run the tests to verify they pass**

Run:

```powershell
cargo test -p force-sync --test rest_apply -- --nocapture
```

Expected: PASS

**Step 5: Commit**

```powershell
git add crates/force-sync/src/apply crates/force-sync/tests/rest_apply.rs
git commit -m "feat: add salesforce rest apply lane"
```

## Task 10: Build the One-Object Vertical Slice Runtime

**Files:**
- Create: `crates/force-sync/src/apply/postgres.rs`
- Create: `crates/force-sync/src/runtime.rs`
- Modify: `crates/force-sync/src/lib.rs`
- Modify: `crates/force-sync/src/config.rs`
- Create: `crates/force-sync/tests/end_to_end_postgres_to_salesforce.rs`

**Step 1: Write the failing end-to-end test**

Scenario:

1. insert an outbox row
2. capture it
3. lease the task
4. plan the change
5. apply to Salesforce via mocked REST endpoint
6. update `sync_link`
7. mark task complete

**Step 2: Run the test to verify it fails**

Run:

```powershell
cargo test -p force-sync --test end_to_end_postgres_to_salesforce -- --nocapture
```

Expected: FAIL because runtime orchestration does not exist.

**Step 3: Implement `SyncEngine` and builder**

Add:

- `SyncEngine`
- `SyncEngineBuilder`
- `run_capture_postgres_once()`
- `run_apply_once()`

Keep v0.1 runtime explicit and narrow. Do not add background supervisor magic yet.

**Step 4: Run the end-to-end test to verify it passes**

Run:

```powershell
cargo test -p force-sync --test end_to_end_postgres_to_salesforce -- --nocapture
```

Expected: PASS

**Step 5: Commit**

```powershell
git add crates/force-sync/src/apply/postgres.rs crates/force-sync/src/runtime.rs crates/force-sync/tests/end_to_end_postgres_to_salesforce.rs
git commit -m "feat: add one-object force-sync runtime slice"
```

## Task 11: Add Salesforce CDC Capture and Replay Checkpoints

**Files:**
- Create: `crates/force-sync/src/capture/salesforce.rs`
- Modify: `crates/force-sync/src/capture/mod.rs`
- Create: `crates/force-sync/tests/cdc_capture.rs`

**Step 1: Write the failing tests**

Do not use live gRPC. Test against a stream input of decoded events.

Cover:

- CDC event becomes journal entry + apply task
- latest replay cursor is checkpointed
- duplicate CDC event is deduped by `(source, source_cursor)`
- restart from stored replay cursor uses the checkpoint value

**Step 2: Run the tests to verify they fail**

Run:

```powershell
cargo test -p force-sync --test cdc_capture -- --nocapture
```

Expected: FAIL because Salesforce capture worker does not exist.

**Step 3: Implement stream-driven CDC capture**

Add a function shaped like:

```rust
pub async fn capture_stream<S>(..., stream: S) -> Result<usize>
where
    S: Stream<Item = Result<PubSubEvent<serde_json::Value>, CaptureError>> + Unpin
```

This keeps capture testable without inventing a giant abstraction layer.

**Step 4: Run the tests to verify they pass**

Run:

```powershell
cargo test -p force-sync --test cdc_capture -- --nocapture
```

Expected: PASS

**Step 5: Commit**

```powershell
git add crates/force-sync/src/capture/salesforce.rs crates/force-sync/tests/cdc_capture.rs
git commit -m "feat: add salesforce cdc capture"
```

## Task 12: Add Bulk Apply and Reconcile

**Files:**
- Create: `crates/force-sync/src/reconcile.rs`
- Modify: `crates/force-sync/src/apply/salesforce.rs`
- Modify: `crates/force-sync/src/runtime.rs`
- Create: `crates/force-sync/tests/bulk_reconcile.rs`

**Step 1: Write the failing tests**

Cover:

- planner sends large homogeneous batches to bulk lane
- bulk upsert uses external ID field
- reconcile task detects drift by hash mismatch
- reconcile repair enqueues a new apply task

**Step 2: Run the tests to verify they fail**

Run:

```powershell
cargo test -p force-sync --test bulk_reconcile -- --nocapture
```

Expected: FAIL because bulk and reconcile code do not exist.

**Step 3: Implement bulk lane**

Use `client.bulk().smart_ingest(object, JobOperation::Upsert)` and set:

- external ID field
- batch size from config

**Step 4: Implement reconcile**

Add:

- `detect_drift(...)`
- `enqueue_repair(...)`
- `run_reconcile_once(...)`

Start with hash drift and watermark windows only. Do not add warehouse sink logic.

**Step 5: Run the tests to verify they pass**

Run:

```powershell
cargo test -p force-sync --test bulk_reconcile -- --nocapture
```

Expected: PASS

**Step 6: Commit**

```powershell
git add crates/force-sync/src/reconcile.rs crates/force-sync/src/apply/salesforce.rs crates/force-sync/src/runtime.rs crates/force-sync/tests/bulk_reconcile.rs
git commit -m "feat: add bulk apply and reconcile"
```

## Task 13: Add Docs, Examples, and Final Verification

**Files:**
- Create: `crates/force-sync/README.md`
- Create: `crates/force-sync/examples/postgres_to_salesforce.rs`
- Modify: `README.md`
- Modify: `docs/README.md`

**Step 1: Write the failing doc test or example compile check**

Create a tiny example using:

```rust
let engine = SyncEngine::builder()
    .postgres(pool)
    .salesforce(client, pubsub)
    .object(ObjectSync::new("Account").external_id("External_Id__c"))
    .build()
    .await?;
```

**Step 2: Run the example compile check to verify it fails**

Run:

```powershell
cargo check -p force-sync --example postgres_to_salesforce
```

Expected: FAIL until public API and imports are wired correctly.

**Step 3: Add crate README and root docs**

Document:

- Postgres-first scope
- external-ID identity rule
- required tables and migrations
- outbox contract
- no SQLite / no warehouse sink in v0.1

**Step 4: Run final verification**

Run:

```powershell
cargo fmt --all
cargo clippy -p force-sync --all-targets -- -D warnings
cargo test -p force-sync
cargo check -p force-sync --example postgres_to_salesforce
rg -n "TODO|FIXME|Stub:" crates/force-sync
```

Expected:

- all Cargo commands exit `0`
- `rg` returns no matches

**Step 5: Commit**

```powershell
git add crates/force-sync README.md docs/README.md
git commit -m "docs: add force-sync usage and verification artifacts"
```

## Cross-Cutting Rules

- Use Verus specs for the small deterministic correctness spine:
  - checkpoint monotonicity
  - dedupe behavior
  - lease transition safety
  - merge-policy determinism
- Keep planner logic pure and unit-testable.
- Do not introduce a public storage trait in v0.1.
- Do not add SQLite support during this plan.
- Do not add direct warehouse sinks during this plan.
- Do not depend on Salesforce IDs for steady-state correctness.

## Final Acceptance Checklist

- `crates/force-sync` exists and is added to the workspace
- one-object Postgres-outbox -> Salesforce REST flow passes end to end
- CDC capture writes journal rows and checkpoints replay position
- planner supports `Rest`, `Bulk`, and `CompositeGraph` lanes
- bulk apply path uses external IDs
- reconcile path can detect and repair drift
- journal history is warehouse-friendly via `journal_id` watermarks
- docs and example compile cleanly

Plan complete and saved to `docs/plans/2026-03-25-force-sync-implementation.md`. Two execution options:

**1. Subagent-Driven (this session)** - I dispatch fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** - Open new session with executing-plans, batch execution with checkpoints

**Which approach?**
