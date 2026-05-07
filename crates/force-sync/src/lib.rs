//! Correctness-first bidirectional Salesforce and Postgres sync engine.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Apply lanes for target systems.
pub(crate) mod apply;
/// Capture workers for source systems.
pub(crate) mod capture;
/// Object-level sync configuration and conflict policy types.
pub(crate) mod config;
/// Error types for force-sync.
pub mod error;
/// Canonical identity types for synced records.
pub(crate) mod identity;
/// Change envelope and cursor types for sync events.
pub(crate) mod model;
/// Pure planner and merge logic for sync envelopes.
pub(crate) mod plan;
/// Reconciliation helpers for drift detection and repair.
pub(crate) mod reconcile;
/// Explicit runtime orchestration for the v0.1 vertical slice.
pub(crate) mod runtime;
/// Storage backends and migration helpers.
pub(crate) mod store;

/// Returns the crate version for smoke tests and runtime diagnostics.
#[must_use]
pub const fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub use apply::{ApplyError, RestApplyResult, SalesforceApplier, project_sync_link};
pub use capture::{capture_batch, capture_stream, load_replay_id};
pub use config::{ConflictPolicy, LaneThresholds, ObjectSync, Owner};
pub use error::ForceSyncError;
pub use identity::SyncKey;
pub use model::{ChangeEnvelope, ChangeOperation, SourceCursor, SourceSystem};
pub use plan::{ApplyLane, MergeOutcome, PlanDecision, PlannerContext, merge_payload, plan_change};
pub use reconcile::DriftItem;
pub use reconcile::{detect_drift, enqueue_repair, run_reconcile_once};
pub use runtime::{SyncEngine, SyncEngineBuilder};
pub use store::pg::{
    AppendResult, CheckpointState, DeadLetter, LeasedTask, PgStore, SyncConflict, SyncLink, migrate,
};
