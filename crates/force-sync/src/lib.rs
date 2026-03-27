//! Correctness-first bidirectional Salesforce and Postgres sync engine.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Apply lanes for target systems.
pub mod apply;
/// Capture workers for source systems.
pub mod capture;
/// Object-level sync configuration and conflict policy types.
pub mod config;
/// Error types for force-sync.
pub mod error;
/// Canonical identity types for synced records.
pub mod identity;
/// Change envelope and cursor types for sync events.
pub mod model;
/// Pure planner and merge logic for sync envelopes.
pub mod plan;
/// Reconciliation helpers for drift detection and repair.
pub mod reconcile;
/// Explicit runtime orchestration for the v0.1 vertical slice.
pub mod runtime;
/// Storage backends and migration helpers.
pub mod store;

/// Returns the crate version for smoke tests and runtime diagnostics.
#[must_use]
pub const fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
