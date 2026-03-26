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
    let mut merged = current.clone();
    let mut conflicts = Vec::new();

    for (field, incoming_value) in incoming {
        match merged.get(field) {
            Some(existing_value) if existing_value == incoming_value => {}
            Some(_existing_value) => match object.field_owner_for(field) {
                Some(Owner::Salesforce) if source == SourceSystem::Salesforce => {
                    merged.insert(field.clone(), incoming_value.clone());
                }
                Some(Owner::Postgres) if source == SourceSystem::Postgres => {
                    merged.insert(field.clone(), incoming_value.clone());
                }
                Some(Owner::Salesforce | Owner::Postgres) => {}
                Some(Owner::Shared) | None => conflicts.push(field.clone()),
            },
            None => match object.field_owner_for(field) {
                Some(Owner::Salesforce) if source == SourceSystem::Salesforce => {
                    merged.insert(field.clone(), incoming_value.clone());
                }
                Some(Owner::Postgres) if source == SourceSystem::Postgres => {
                    merged.insert(field.clone(), incoming_value.clone());
                }
                Some(Owner::Salesforce | Owner::Postgres) => {}
                Some(Owner::Shared) | None => {
                    merged.insert(field.clone(), incoming_value.clone());
                }
            },
        }
    }

    if !conflicts.is_empty() {
        return MergeOutcome::Conflict { fields: conflicts };
    }

    MergeOutcome::Merged(Value::Object(merged))
}
