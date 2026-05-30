//! Planner and merge logic tests.

use chrono::Utc;
use proptest::prelude::*;
use serde_json::{Value, json};

use force_sync::SyncKey;
use force_sync::{ApplyLane, MergeOutcome, PlannerContext, merge_payload, plan_change};
use force_sync::{ChangeEnvelope, ChangeOperation, SourceSystem};
use force_sync::{ObjectSync, Owner};

fn sync_key() -> SyncKey {
    match SyncKey::new("tenant", "Account", "abc") {
        Ok(sync_key) => sync_key,
        Err(error) => panic!("unexpected sync key construction error: {error}"),
    }
}

fn envelope(payload: Value) -> ChangeEnvelope {
    ChangeEnvelope::new(
        sync_key(),
        SourceSystem::Salesforce,
        ChangeOperation::Upsert,
        Utc::now(),
        payload,
    )
}

const fn base_context(object: &ObjectSync) -> PlannerContext<'_> {
    PlannerContext {
        object,
        current_payload: None,
        batch_size: 1,
        urgent: true,
        has_dependencies: false,
    }
}

#[test]
fn identical_hashes_become_noop() {
    let object = ObjectSync::new("Account").external_id("External_Id__c");
    let payload = json!({
        "Description": "Keep",
        "Name": "Acme"
    });
    let context = PlannerContext {
        current_payload: Some(&json!({
            "Name": "Acme",
            "Description": "Keep"
        })),
        ..base_context(&object)
    };

    let decision = plan_change(&context, &envelope(payload));

    assert_eq!(decision.lane, ApplyLane::Noop);
    assert!(decision.payload.is_none());
    assert!(decision.conflicts.is_empty());
}

#[test]
fn postgres_owned_field_wins_when_salesforce_changes_it() {
    let object = ObjectSync::new("Account")
        .external_id("External_Id__c")
        .field_owner("Name", Owner::Postgres);
    let current_payload = json!({
        "Description": "Keep",
        "Name": "Old"
    });
    let context = PlannerContext {
        current_payload: Some(&json!({
            "Description": "Keep",
            "Name": "Old"
        })),
        ..base_context(&object)
    };

    let decision = plan_change(
        &context,
        &envelope(json!({
            "Description": "Keep",
            "Name": "New"
        })),
    );

    assert_eq!(
        merge_payload(
            context.object,
            context.current_payload,
            SourceSystem::Salesforce,
            &json!({
                "Description": "Keep",
                "Name": "New"
            }),
        ),
        MergeOutcome::Merged(current_payload)
    );
    assert_eq!(decision.lane, ApplyLane::Noop);
    assert!(decision.payload.is_none());
    assert!(decision.conflicts.is_empty());
}

#[test]
fn conflict_required_field_opens_a_conflict() {
    let object = ObjectSync::new("Account")
        .external_id("External_Id__c")
        .field_owner("Name", Owner::Shared);
    let context = PlannerContext {
        current_payload: Some(&json!({
            "Description": "Keep",
            "Name": "Old"
        })),
        ..base_context(&object)
    };

    let decision = plan_change(
        &context,
        &envelope(json!({
            "Description": "Keep",
            "Name": "New"
        })),
    );

    assert_eq!(decision.lane, ApplyLane::Conflict);
    assert!(decision.payload.is_none());
    assert!(decision.conflicts.iter().any(|field| field == "Name"));
}

#[test]
fn large_batches_choose_bulk() {
    let object = ObjectSync::new("Account").external_id("External_Id__c");
    let context = PlannerContext {
        batch_size: 500,
        urgent: false,
        ..base_context(&object)
    };

    let decision = plan_change(&context, &envelope(json!({"Name": "Acme"})));

    assert_eq!(decision.lane, ApplyLane::Bulk);
}

#[test]
fn dependent_records_choose_composite_graph() {
    let object = ObjectSync::new("Account").external_id("External_Id__c");
    let context = PlannerContext {
        urgent: true,
        has_dependencies: true,
        ..base_context(&object)
    };

    let decision = plan_change(&context, &envelope(json!({"Name": "Acme"})));

    assert_eq!(decision.lane, ApplyLane::CompositeGraph);
}

#[test]
fn urgent_singleton_changes_choose_rest() {
    let object = ObjectSync::new("Account").external_id("External_Id__c");
    let context = PlannerContext {
        urgent: true,
        batch_size: 1,
        ..base_context(&object)
    };

    let decision = plan_change(&context, &envelope(json!({"Name": "Acme"})));

    assert_eq!(decision.lane, ApplyLane::Rest);
}

proptest! {
    #[test]
    fn duplicate_envelopes_plan_to_noop(fields in prop::collection::btree_map(
        "[a-z]{1,8}",
        "[A-Za-z0-9]{0,8}",
        0..4,
    )) {
        let object = ObjectSync::new("Account").external_id("External_Id__c");
        let payload = match serde_json::to_value(fields) {
            Ok(payload) => payload,
            Err(error) => panic!("unexpected json conversion error: {error}"),
        };
        let context = PlannerContext {
            current_payload: Some(&payload),
            ..base_context(&object)
        };

        let decision = plan_change(&context, &envelope(payload.clone()));

        prop_assert_eq!(decision.lane, ApplyLane::Noop);
        prop_assert!(decision.payload.is_none());
        prop_assert!(decision.conflicts.is_empty());
    }
}
