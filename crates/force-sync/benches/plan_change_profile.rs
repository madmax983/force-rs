//! One-shot profiling harness for `force_sync::plan_change`
//! (per-record planner/merge decision for the apply pipeline).
//!
//! This is not a criterion benchmark: it is a single realistic run of the
//! public `plan_change` entry point, meant to be executed once under
//! `valgrind --tool=callgrind` (instruction counts) and `valgrind --tool=dhat`
//! (allocation counts) so those tools attribute cost to real call stacks
//! instead of a criterion harness loop.
//!
//! Workload: `SyncEngine::process_leased_task` (crates/force-sync/src/runtime.rs)
//! calls `plan_change` once per leased apply task, with a fresh
//! `PlannerContext` built from the object's config (cloned), the current
//! Postgres-side payload (cloned), and the incoming `ChangeEnvelope` --
//! exactly what an apply-batch worker does for every row in a leased batch.
//! This harness reproduces that call pattern directly: `RECORD_COUNT`
//! Account-shaped change envelopes (12 fields: a mix of Salesforce-owned,
//! Postgres-owned, and shared fields, matching a typical field-ownership
//! split for a bidirectionally-synced object), each with a "current"
//! Postgres-side payload that is either identical (a realistic no-op
//! fraction of any real backlog) or differs by a few fields (a realistic
//! incremental change). Field shapes, ownership, and the identical/changed
//! split are fixed (no RNG) so repeated runs are byte-for-byte
//! deterministic, which is required for callgrind/dhat comparisons to be
//! meaningful.
//!
//! Run directly (`cargo run` doesn't support `--bench`, so use `cargo bench`):
//! ```bash
//! cargo bench -p force-sync --bench plan_change_profile
//! ```
//!
//! Profile (this is a `[[bench]]` target, not a `[[bin]]`: the compiled
//! executable lands under `target/release/deps/`, not `target/release/`):
//! ```bash
//! CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release -p force-sync --bench plan_change_profile
//! BIN=$(find target/release/deps -maxdepth 1 -name 'plan_change_profile-*' -executable -not -name '*.d')
//! valgrind --tool=callgrind --callgrind-out-file=callgrind.out "$BIN"
//! callgrind_annotate callgrind.out
//!
//! valgrind --tool=dhat --dhat-out-file=dhat.out "$BIN"
//! ```
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use chrono::Utc;
use force_sync::{
    ApplyLane, ChangeEnvelope, ChangeOperation, ObjectSync, Owner, PlannerContext, SourceSystem,
    SyncKey, plan_change,
};
use serde_json::{Value, json};

/// Change envelopes planned in one profiling run -- a realistic leased
/// apply-batch backlog size for a busy tenant/object pair.
const RECORD_COUNT: usize = 5_000;

/// A 12-field Account-shaped object config: a mix of Salesforce-owned,
/// Postgres-owned, and shared fields, matching a typical bidirectional
/// field-ownership split (some fields are source-of-truth on one side,
/// some are jointly editable and go through conflict detection).
fn account_object_sync() -> ObjectSync {
    ObjectSync::new("Account")
        .field_owner("Name", Owner::Salesforce)
        .field_owner("Industry", Owner::Salesforce)
        .field_owner("AnnualRevenue", Owner::Salesforce)
        .field_owner("Rating", Owner::Salesforce)
        .field_owner("BillingStreet", Owner::Postgres)
        .field_owner("BillingCity", Owner::Postgres)
        .field_owner("BillingState", Owner::Postgres)
        .field_owner("Description", Owner::Postgres)
        .field_owner("Phone", Owner::Shared)
        .field_owner("Website", Owner::Shared)
}

/// Builds the "current" (already-applied) payload for record `i`.
fn current_payload(i: usize) -> Value {
    json!({
        "Name": format!("Account {i}"),
        "Industry": "Technology",
        "AnnualRevenue": 1_000_000 + i,
        "Rating": "Warm",
        "BillingStreet": format!("{i} Main St"),
        "BillingCity": "Springfield",
        "BillingState": "IL",
        "Description": "Existing customer account",
        "Phone": "555-0100",
        "Website": "https://example.com",
        "NumberOfEmployees": 250,
        "AccountSource": "Web",
    })
}

/// Builds the incoming payload for record `i`. Every 5th record is byte-for-
/// byte identical to `current_payload` (a realistic no-op fraction of any
/// real backlog); the rest change a handful of fields, split between
/// non-conflicting updates and a shared-field conflict (`Phone`) every 7th
/// record, matching a realistic incremental-change distribution.
fn incoming_payload(i: usize) -> Value {
    if i % 5 == 0 {
        return current_payload(i);
    }

    let mut payload = current_payload(i);
    let Value::Object(map) = &mut payload else {
        unreachable!("current_payload always returns an object")
    };
    map.insert("AnnualRevenue".to_string(), json!(1_000_000 + i + 500));
    map.insert("Rating".to_string(), json!("Hot"));

    if i % 7 == 0 {
        map.insert("Phone".to_string(), json!("555-0199"));
    }

    payload
}

const fn source_for(i: usize) -> SourceSystem {
    if i % 2 == 0 {
        SourceSystem::Salesforce
    } else {
        SourceSystem::Postgres
    }
}

fn main() {
    let object = account_object_sync();

    let mut noop = 0usize;
    let mut conflict = 0usize;
    let mut rest = 0usize;
    let mut bulk = 0usize;
    let mut composite_graph = 0usize;

    for i in 0..RECORD_COUNT {
        let sync_key = SyncKey::new("acme", "Account", format!("acc-{i:06}"))
            .expect("non-empty sync key parts");
        let envelope = ChangeEnvelope::new(
            sync_key,
            source_for(i),
            ChangeOperation::Upsert,
            Utc::now(),
            incoming_payload(i),
        );

        let context = PlannerContext {
            object: object.clone(),
            current_payload: Some(current_payload(i)),
            batch_size: RECORD_COUNT,
            urgent: false,
            has_dependencies: false,
        };

        let decision = plan_change(&context, &envelope);
        match decision.lane {
            ApplyLane::Noop => noop += 1,
            ApplyLane::Conflict => conflict += 1,
            ApplyLane::Rest => rest += 1,
            ApplyLane::Bulk => bulk += 1,
            ApplyLane::CompositeGraph => composite_graph += 1,
        }
    }

    // Keep the result observable so the compiler can't fold the whole run away.
    println!(
        "record_count={RECORD_COUNT} noop={noop} conflict={conflict} rest={rest} bulk={bulk} composite_graph={composite_graph}"
    );
}
