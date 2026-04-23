//! Pure planner and merge logic for sync envelopes.

use serde_json::{Map, Value};

use crate::config::{ObjectSync, Owner};
use crate::model::{ChangeEnvelope, SourceSystem, payload_hash};

/// Apply lanes chosen by the planner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyLane {
    /// No write is required because the incoming change matches current state.
    Noop,
    /// The change requires operator intervention or later resolution.
    Conflict,
    /// Apply the change through the Salesforce REST API.
    Rest,
    /// Apply the change through Composite Graph for dependent records.
    CompositeGraph,
    /// Apply the change through bulk transport.
    Bulk,
}

/// The pure inputs required to plan a change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannerContext {
    /// Object-level sync configuration.
    pub object: ObjectSync,
    /// The current canonical payload, if one is already known.
    pub current_payload: Option<Value>,
    /// The number of records being planned together.
    pub batch_size: usize,
    /// Whether the record should favor the fast path over batch economics.
    pub urgent: bool,
    /// Whether the change depends on related records being applied together.
    pub has_dependencies: bool,
}

/// The final planner decision after merge and lane selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanDecision {
    /// The selected apply lane.
    pub lane: ApplyLane,
    /// The merged payload to apply, if any.
    pub payload: Option<Value>,
    /// Fields that could not be resolved automatically.
    pub conflicts: Vec<String>,
}

/// The result of merging payloads before lane selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
    /// No material change was detected.
    Noop,
    /// The payload was merged successfully.
    Merged(Value),
    /// The merge could not be resolved automatically.
    Conflict {
        /// The fields that caused the conflict.
        fields: Vec<String>,
    },
}

/// Merges an incoming payload into the current payload using field ownership rules.
#[must_use]
pub fn merge_payload(
    object: &ObjectSync,
    current_payload: Option<&Value>,
    source: SourceSystem,
    incoming_payload: &Value,
) -> MergeOutcome {
    let Some(current_payload) = current_payload else {
        return MergeOutcome::Merged(incoming_payload.clone());
    };

    if payload_hash(current_payload) == payload_hash(incoming_payload) {
        return MergeOutcome::Noop;
    }

    match (current_payload, incoming_payload) {
        (Value::Object(current), Value::Object(incoming)) => {
            merge_object_payload(object, current, source, incoming)
        }
        _ if current_payload == incoming_payload => MergeOutcome::Noop,
        _ => MergeOutcome::Conflict {
            fields: vec!["<root>".to_string()],
        },
    }
}

/// Plans how a change should be applied after merge resolution.
#[must_use]
pub fn plan_change(context: &PlannerContext, envelope: &ChangeEnvelope) -> PlanDecision {
    if context
        .current_payload
        .as_ref()
        .is_some_and(|current| envelope.payload_hash_matches(current))
    {
        return PlanDecision {
            lane: ApplyLane::Noop,
            payload: None,
            conflicts: Vec::new(),
        };
    }

    match merge_payload(
        &context.object,
        context.current_payload.as_ref(),
        envelope.source(),
        envelope.payload(),
    ) {
        MergeOutcome::Noop => PlanDecision {
            lane: ApplyLane::Noop,
            payload: None,
            conflicts: Vec::new(),
        },
        MergeOutcome::Conflict { fields } => PlanDecision {
            lane: ApplyLane::Conflict,
            payload: None,
            conflicts: fields,
        },
        MergeOutcome::Merged(payload) => PlanDecision {
            lane: if context
                .current_payload
                .as_ref()
                .is_some_and(|current| current == &payload)
            {
                ApplyLane::Noop
            } else {
                choose_lane(context)
            },
            payload: if context
                .current_payload
                .as_ref()
                .is_some_and(|current| current == &payload)
            {
                None
            } else {
                Some(payload)
            },
            conflicts: Vec::new(),
        },
    }
}

const fn choose_lane(context: &PlannerContext) -> ApplyLane {
    if context.has_dependencies {
        return ApplyLane::CompositeGraph;
    }

    if context.urgent {
        return ApplyLane::Rest;
    }

    if context.batch_size >= context.object.lane_thresholds().bulk_min_batch_size() {
        ApplyLane::Bulk
    } else {
        ApplyLane::Rest
    }
}

fn merge_object_payload(
    object: &ObjectSync,
    current: &Map<String, Value>,
    source: SourceSystem,
    incoming: &Map<String, Value>,
) -> MergeOutcome {
    // ⚡ Bolt: Delay cloning `current` until an actual mutation is needed.
    // If all incoming fields already match or are ignored, we avoid cloning the entire map.
    let mut merged: Option<Map<String, Value>> = None;
    let mut conflicts = Vec::new();

    for (field, incoming_value) in incoming {
        match current.get(field) {
            Some(existing_value) if existing_value == incoming_value => {}
            Some(_) => match object.field_owner_for(field) {
                Some(Owner::Salesforce) if source == SourceSystem::Salesforce => {
                    merged
                        .get_or_insert_with(|| current.clone())
                        .insert(field.clone(), incoming_value.clone());
                }
                Some(Owner::Postgres) if source == SourceSystem::Postgres => {
                    merged
                        .get_or_insert_with(|| current.clone())
                        .insert(field.clone(), incoming_value.clone());
                }
                Some(Owner::Salesforce | Owner::Postgres) => {}
                Some(Owner::Shared) | None => conflicts.push(field.clone()),
            },
            None => match object.field_owner_for(field) {
                Some(Owner::Salesforce) if source == SourceSystem::Salesforce => {
                    merged
                        .get_or_insert_with(|| current.clone())
                        .insert(field.clone(), incoming_value.clone());
                }
                Some(Owner::Postgres) if source == SourceSystem::Postgres => {
                    merged
                        .get_or_insert_with(|| current.clone())
                        .insert(field.clone(), incoming_value.clone());
                }
                Some(Owner::Salesforce | Owner::Postgres) => {}
                Some(Owner::Shared) | None => {
                    merged
                        .get_or_insert_with(|| current.clone())
                        .insert(field.clone(), incoming_value.clone());
                }
            },
        }
    }

    let merged = merged.unwrap_or_else(|| current.clone());

    if !conflicts.is_empty() {
        return MergeOutcome::Conflict { fields: conflicts };
    }

    MergeOutcome::Merged(Value::Object(merged))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::SyncKey;
    use crate::model::{ChangeEnvelope, ChangeOperation, SourceSystem};
    use chrono::Utc;
    use serde_json::json;

    fn test_object_sync() -> ObjectSync {
        ObjectSync::new("Account")
    }

    fn test_object_sync_with_ownership() -> ObjectSync {
        ObjectSync::new("Account")
            .field_owner("Name", Owner::Salesforce)
            .field_owner("Billing", Owner::Postgres)
            .field_owner("Phone", Owner::Shared)
    }

    fn test_envelope(
        source: SourceSystem,
        operation: ChangeOperation,
        payload: Value,
    ) -> ChangeEnvelope {
        let Ok(sync_key) = SyncKey::new("tenant", "Account", "ext-001") else {
            panic!("failed to create test sync key");
        };
        ChangeEnvelope::new(sync_key, source, operation, Utc::now(), payload)
    }

    fn test_context(object: &ObjectSync, current_payload: Option<Value>) -> PlannerContext {
        PlannerContext {
            object: object.clone(),
            current_payload,
            batch_size: 1,
            urgent: false,
            has_dependencies: false,
        }
    }

    // ── merge_payload ─────────────────────────────────────────────────

    #[test]
    fn merge_payload_no_current_returns_incoming_as_merged() {
        let object = test_object_sync();
        let incoming = json!({"Name": "Acme"});

        let result = merge_payload(&object, None, SourceSystem::Salesforce, &incoming);
        assert_eq!(result, MergeOutcome::Merged(json!({"Name": "Acme"})));
    }

    #[test]
    fn merge_payload_identical_payloads_returns_noop() {
        let object = test_object_sync();
        let current = json!({"Name": "Acme"});
        let incoming = json!({"Name": "Acme"});

        let result = merge_payload(&object, Some(&current), SourceSystem::Salesforce, &incoming);
        assert_eq!(result, MergeOutcome::Noop);
    }

    #[test]
    fn merge_payload_non_object_mismatch_returns_conflict() {
        let object = test_object_sync();
        let current = json!("string_a");
        let incoming = json!("string_b");

        let result = merge_payload(&object, Some(&current), SourceSystem::Salesforce, &incoming);
        assert!(matches!(result, MergeOutcome::Conflict { fields } if fields == vec!["<root>"]));
    }

    #[test]
    fn merge_payload_non_object_identical_returns_noop() {
        let object = test_object_sync();
        let current = json!(42);
        let incoming = json!(42);

        let result = merge_payload(&object, Some(&current), SourceSystem::Salesforce, &incoming);
        assert_eq!(result, MergeOutcome::Noop);
    }

    #[test]
    fn merge_payload_new_field_no_ownership_is_added() {
        let object = test_object_sync();
        let current = json!({"Name": "Acme"});
        let incoming = json!({"Name": "Acme", "Phone": "555-1234"});

        let result = merge_payload(&object, Some(&current), SourceSystem::Salesforce, &incoming);
        assert_eq!(
            result,
            MergeOutcome::Merged(json!({"Name": "Acme", "Phone": "555-1234"}))
        );
    }

    #[test]
    fn merge_payload_salesforce_owned_field_accepted_from_salesforce() {
        let object = test_object_sync_with_ownership();
        let current = json!({"Name": "Old"});
        let incoming = json!({"Name": "New"});

        let result = merge_payload(&object, Some(&current), SourceSystem::Salesforce, &incoming);
        assert_eq!(result, MergeOutcome::Merged(json!({"Name": "New"})));
    }

    #[test]
    fn merge_payload_salesforce_owned_field_rejected_from_postgres() {
        let object = test_object_sync_with_ownership();
        let current = json!({"Name": "Acme"});
        let incoming = json!({"Name": "Changed"});

        let result = merge_payload(&object, Some(&current), SourceSystem::Postgres, &incoming);
        assert_eq!(result, MergeOutcome::Merged(json!({"Name": "Acme"})));
    }

    #[test]
    fn merge_payload_shared_field_conflict_on_change() {
        let object = test_object_sync_with_ownership();
        let current = json!({"Phone": "111"});
        let incoming = json!({"Phone": "222"});

        let result = merge_payload(&object, Some(&current), SourceSystem::Salesforce, &incoming);
        assert!(matches!(result, MergeOutcome::Conflict { fields } if fields == vec!["Phone"]));
    }

    #[test]
    fn merge_payload_null_field_in_incoming_with_ownership() {
        let object = ObjectSync::new("Account")
            .field_owner("Name", Owner::Salesforce)
            .field_owner("Phone", Owner::Salesforce);
        let current = json!({"Name": "Acme", "Phone": "555"});
        let incoming = json!({"Name": "Acme", "Phone": null});

        let result = merge_payload(&object, Some(&current), SourceSystem::Salesforce, &incoming);
        assert_eq!(
            result,
            MergeOutcome::Merged(json!({"Name": "Acme", "Phone": null}))
        );
    }

    #[test]
    fn merge_payload_empty_incoming_object_returns_current() {
        let object = test_object_sync();
        let current = json!({"Name": "Acme"});
        let incoming = json!({});

        let result = merge_payload(&object, Some(&current), SourceSystem::Salesforce, &incoming);
        assert_eq!(result, MergeOutcome::Merged(json!({"Name": "Acme"})));
    }

    #[test]
    fn merge_payload_postgres_owned_field_accepted_from_postgres() {
        let object = test_object_sync_with_ownership();
        let current = json!({"Billing": "Old Address"});
        let incoming = json!({"Billing": "New Address"});

        let result = merge_payload(&object, Some(&current), SourceSystem::Postgres, &incoming);
        assert_eq!(
            result,
            MergeOutcome::Merged(json!({"Billing": "New Address"}))
        );
    }

    #[test]
    fn merge_payload_postgres_owned_field_rejected_from_salesforce() {
        let object = test_object_sync_with_ownership();
        let current = json!({"Billing": "Original"});
        let incoming = json!({"Billing": "Overwritten"});

        let result = merge_payload(&object, Some(&current), SourceSystem::Salesforce, &incoming);
        assert_eq!(result, MergeOutcome::Merged(json!({"Billing": "Original"})));
    }

    #[test]
    fn merge_payload_new_field_salesforce_owned_added_from_salesforce() {
        let object = test_object_sync_with_ownership();
        let current = json!({});
        let incoming = json!({"Name": "Added"});

        let result = merge_payload(&object, Some(&current), SourceSystem::Salesforce, &incoming);
        assert_eq!(result, MergeOutcome::Merged(json!({"Name": "Added"})));
    }

    #[test]
    fn merge_payload_new_field_salesforce_owned_ignored_from_postgres() {
        let object = test_object_sync_with_ownership();
        let current = json!({});
        let incoming = json!({"Name": "ShouldBeIgnored"});

        let result = merge_payload(&object, Some(&current), SourceSystem::Postgres, &incoming);
        assert_eq!(result, MergeOutcome::Merged(json!({})));
    }

    #[test]
    fn merge_payload_new_field_postgres_owned_ignored_from_salesforce() {
        let object = test_object_sync_with_ownership();
        let current = json!({});
        let incoming = json!({"Billing": "ShouldBeIgnored"});

        let result = merge_payload(&object, Some(&current), SourceSystem::Salesforce, &incoming);
        assert_eq!(result, MergeOutcome::Merged(json!({})));
    }

    #[test]
    fn merge_payload_new_shared_field_is_added() {
        let object = test_object_sync_with_ownership();
        let current = json!({});
        let incoming = json!({"Phone": "555-0100"});

        let result = merge_payload(&object, Some(&current), SourceSystem::Salesforce, &incoming);
        assert_eq!(result, MergeOutcome::Merged(json!({"Phone": "555-0100"})));
    }

    #[test]
    fn merge_payload_multiple_conflicts_collected() {
        let object = ObjectSync::new("Account")
            .field_owner("A", Owner::Shared)
            .field_owner("B", Owner::Shared);
        let current = json!({"A": 1, "B": 2});
        let incoming = json!({"A": 10, "B": 20});

        let result = merge_payload(&object, Some(&current), SourceSystem::Salesforce, &incoming);
        match result {
            MergeOutcome::Conflict { fields } => {
                assert!(fields.contains(&"A".to_string()));
                assert!(fields.contains(&"B".to_string()));
                assert_eq!(fields.len(), 2);
            }
            other => panic!("expected Conflict, got {other:?}"),
        }
    }

    #[test]
    fn merge_payload_null_field_in_incoming_no_ownership() {
        let object = test_object_sync();
        let current = json!({"Name": "Acme", "Phone": "555"});
        let incoming = json!({"Name": "Acme", "Phone": null});

        let result = merge_payload(&object, Some(&current), SourceSystem::Salesforce, &incoming);
        assert!(matches!(result, MergeOutcome::Conflict { fields } if fields == vec!["Phone"]));
    }

    // ── plan_change ───────────────────────────────────────────────────

    #[test]
    fn plan_change_delete_no_current_routes_to_rest() {
        let object = test_object_sync();
        let envelope = test_envelope(SourceSystem::Salesforce, ChangeOperation::Delete, json!({}));
        let ctx = test_context(&object, None);

        let decision = plan_change(&ctx, &envelope);
        assert_eq!(decision.lane, ApplyLane::Rest);
    }

    #[test]
    fn plan_change_identical_payload_returns_noop() {
        let object = test_object_sync();
        let payload = json!({"Name": "Acme"});
        let envelope = test_envelope(
            SourceSystem::Salesforce,
            ChangeOperation::Upsert,
            payload.clone(),
        );
        let ctx = test_context(&object, Some(payload));

        let decision = plan_change(&ctx, &envelope);
        assert_eq!(decision.lane, ApplyLane::Noop);
        assert!(decision.payload.is_none());
        assert!(decision.conflicts.is_empty());
    }

    #[test]
    fn plan_change_new_record_routes_to_rest() {
        let object = test_object_sync();
        let envelope = test_envelope(
            SourceSystem::Postgres,
            ChangeOperation::Upsert,
            json!({"Name": "New"}),
        );
        let ctx = test_context(&object, None);

        let decision = plan_change(&ctx, &envelope);
        assert_eq!(decision.lane, ApplyLane::Rest);
        assert_eq!(decision.payload, Some(json!({"Name": "New"})));
    }

    #[test]
    fn plan_change_urgent_routes_to_rest() {
        let object = ObjectSync::new("Account").field_owner("Name", Owner::Postgres);
        let envelope = test_envelope(
            SourceSystem::Postgres,
            ChangeOperation::Upsert,
            json!({"Name": "Updated"}),
        );
        let mut ctx = test_context(&object, Some(json!({"Name": "Old"})));
        ctx.urgent = true;

        let decision = plan_change(&ctx, &envelope);
        assert_eq!(decision.lane, ApplyLane::Rest);
    }

    #[test]
    fn plan_change_with_dependencies_routes_to_composite_graph() {
        let object = ObjectSync::new("Account").field_owner("Name", Owner::Postgres);
        let envelope = test_envelope(
            SourceSystem::Postgres,
            ChangeOperation::Upsert,
            json!({"Name": "Updated"}),
        );
        let mut ctx = test_context(&object, Some(json!({"Name": "Old"})));
        ctx.has_dependencies = true;

        let decision = plan_change(&ctx, &envelope);
        assert_eq!(decision.lane, ApplyLane::CompositeGraph);
    }

    #[test]
    fn plan_change_large_batch_routes_to_bulk() {
        let object = ObjectSync::new("Account").field_owner("Name", Owner::Postgres);
        let envelope = test_envelope(
            SourceSystem::Postgres,
            ChangeOperation::Upsert,
            json!({"Name": "Updated"}),
        );
        let mut ctx = test_context(&object, Some(json!({"Name": "Old"})));
        ctx.batch_size = 1000;

        let decision = plan_change(&ctx, &envelope);
        assert_eq!(decision.lane, ApplyLane::Bulk);
    }

    #[test]
    fn plan_change_conflict_returns_conflict_lane() {
        let object = test_object_sync_with_ownership();
        let envelope = test_envelope(
            SourceSystem::Salesforce,
            ChangeOperation::Upsert,
            json!({"Phone": "new-value"}),
        );
        let ctx = test_context(&object, Some(json!({"Phone": "old-value"})));

        let decision = plan_change(&ctx, &envelope);
        assert_eq!(decision.lane, ApplyLane::Conflict);
        assert!(decision.payload.is_none());
        assert!(!decision.conflicts.is_empty());
    }

    #[test]
    fn plan_change_merged_equals_current_returns_noop() {
        let object = test_object_sync_with_ownership();
        let current = json!({"Name": "Acme", "Billing": "123 Main"});
        let incoming = json!({"Name": "Changed"});
        let envelope = test_envelope(SourceSystem::Postgres, ChangeOperation::Upsert, incoming);
        let ctx = test_context(&object, Some(current));

        let decision = plan_change(&ctx, &envelope);
        assert_eq!(decision.lane, ApplyLane::Noop);
        assert!(decision.payload.is_none());
    }

    #[test]
    fn plan_change_batch_below_bulk_threshold_routes_to_rest() {
        let object = ObjectSync::new("Account").field_owner("Name", Owner::Postgres);
        let envelope = test_envelope(
            SourceSystem::Postgres,
            ChangeOperation::Upsert,
            json!({"Name": "Updated"}),
        );
        let mut ctx = test_context(&object, Some(json!({"Name": "Old"})));
        ctx.batch_size = 499;

        let decision = plan_change(&ctx, &envelope);
        assert_eq!(decision.lane, ApplyLane::Rest);
    }

    #[test]
    fn plan_change_batch_at_bulk_threshold_routes_to_bulk() {
        let object = ObjectSync::new("Account").field_owner("Name", Owner::Postgres);
        let envelope = test_envelope(
            SourceSystem::Postgres,
            ChangeOperation::Upsert,
            json!({"Name": "Updated"}),
        );
        let mut ctx = test_context(&object, Some(json!({"Name": "Old"})));
        ctx.batch_size = 500;

        let decision = plan_change(&ctx, &envelope);
        assert_eq!(decision.lane, ApplyLane::Bulk);
    }

    #[test]
    fn plan_change_dependencies_take_priority_over_bulk() {
        let object = ObjectSync::new("Account").field_owner("Name", Owner::Postgres);
        let envelope = test_envelope(
            SourceSystem::Postgres,
            ChangeOperation::Upsert,
            json!({"Name": "Updated"}),
        );
        let mut ctx = test_context(&object, Some(json!({"Name": "Old"})));
        ctx.batch_size = 1000;
        ctx.has_dependencies = true;

        let decision = plan_change(&ctx, &envelope);
        assert_eq!(decision.lane, ApplyLane::CompositeGraph);
    }

    #[test]
    fn plan_change_dependencies_take_priority_over_urgent() {
        let object = ObjectSync::new("Account").field_owner("Name", Owner::Postgres);
        let envelope = test_envelope(
            SourceSystem::Postgres,
            ChangeOperation::Upsert,
            json!({"Name": "Updated"}),
        );
        let mut ctx = test_context(&object, Some(json!({"Name": "Old"})));
        ctx.urgent = true;
        ctx.has_dependencies = true;

        let decision = plan_change(&ctx, &envelope);
        assert_eq!(decision.lane, ApplyLane::CompositeGraph);
    }

    #[test]
    fn plan_change_urgent_takes_priority_over_bulk() {
        let object = ObjectSync::new("Account").field_owner("Name", Owner::Postgres);
        let envelope = test_envelope(
            SourceSystem::Postgres,
            ChangeOperation::Upsert,
            json!({"Name": "Updated"}),
        );
        let mut ctx = test_context(&object, Some(json!({"Name": "Old"})));
        ctx.batch_size = 1000;
        ctx.urgent = true;

        let decision = plan_change(&ctx, &envelope);
        assert_eq!(decision.lane, ApplyLane::Rest);
    }

    #[test]
    fn plan_change_semantically_equal_payload_returns_noop() {
        let object = test_object_sync();
        let current = json!({"A": 1, "B": 2});
        let incoming = json!({"B": 2, "A": 1});
        let envelope = test_envelope(SourceSystem::Postgres, ChangeOperation::Upsert, incoming);
        let ctx = test_context(&object, Some(current));

        let decision = plan_change(&ctx, &envelope);
        assert_eq!(decision.lane, ApplyLane::Noop);
    }
}
