//! Apply steps shared by the explicit runtime and the durable runtime.
//!
//! An apply task runs in three phases:
//!
//! 1. **Prepare** reads the journal row and the sync link, runs the planner,
//!    and returns an [`ApplyPlan`]. It writes nothing.
//! 2. **Execute** performs the one Salesforce call that a
//!    [`ApplyPlan::Remote`] plan names, and returns a [`RemoteOutcome`].
//! 3. **Finalize** writes the sync link and acknowledges, retries, or fails
//!    the task.
//!
//! The explicit runtime runs the three phases in one call. The durable
//! runtime runs each phase as a separate workflow activity, so a crash after
//! the Salesforce call does not repeat the call on recovery. Both types are
//! serializable for that reason.

use force::types::SalesforceId;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    config::ObjectSync,
    error::ForceSyncError,
    model::{ChangeEnvelope, ChangeOperation, SourceSystem},
    plan::{ApplyLane, PlanDecision},
    store::pg::SyncLink,
};

/// Failure reason when an apply task has no journal row.
pub(crate) const MISSING_JOURNAL_ROW: &str = "missing apply task journal row";

/// Failure reason when a delete has no known Salesforce ID.
pub(crate) const MISSING_DELETE_ID: &str = "missing Salesforce ID for delete";

/// The decision that the prepare phase records for one apply task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ApplyPlan {
    /// The task cannot be applied. Fail it with this reason.
    Fail {
        /// The reason stored in `sync_task.last_error`.
        reason: String,
    },
    /// The planner found conflicts. Record them and fail the task.
    Conflict {
        /// The conflicting field names.
        fields: Vec<String>,
    },
    /// No write is required. Acknowledge the task.
    Noop,
    /// The change came from Salesforce. Project the link locally.
    ProjectLocal {
        /// The merged record state to store, or `None` for a delete.
        ///
        /// A plan recorded before this field existed reads as `None`.
        #[serde(default)]
        payload: Option<Value>,
    },
    /// Write the change to Salesforce.
    Remote(RemoteCall),
}

/// The one Salesforce call that a remote plan makes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "call", rename_all = "snake_case")]
pub(crate) enum RemoteCall {
    /// Upsert one record through the REST external-ID endpoint.
    RestUpsert {
        /// The Salesforce object name.
        object_name: String,
        /// The external ID field on the object.
        external_id_field: String,
        /// The external ID value.
        external_id: String,
        /// The merged payload to write.
        payload: Value,
    },
    /// Upsert one record through Bulk API 2.0.
    BulkUpsert {
        /// The Salesforce object name.
        object_name: String,
        /// The external ID field on the object.
        external_id_field: String,
        /// The merged payload to write.
        payload: Value,
        /// The Salesforce ID that the sync link already holds.
        salesforce_id: String,
    },
    /// Delete one record by its Salesforce ID.
    Delete {
        /// The Salesforce object name.
        object_name: String,
        /// The Salesforce ID to delete.
        salesforce_id: String,
    },
}

impl RemoteCall {
    /// The merged record state that an upsert writes.
    pub(crate) const fn payload(&self) -> Option<&Value> {
        match self {
            Self::RestUpsert { payload, .. } | Self::BulkUpsert { payload, .. } => Some(payload),
            Self::Delete { .. } => None,
        }
    }

    /// Whether the call removes the record.
    pub(crate) const fn is_delete(&self) -> bool {
        matches!(self, Self::Delete { .. })
    }

    /// The Salesforce ID that the call names before it runs, if any.
    pub(crate) fn known_salesforce_id(&self) -> Option<&str> {
        match self {
            Self::RestUpsert { .. } => None,
            Self::BulkUpsert { salesforce_id, .. } | Self::Delete { salesforce_id, .. } => {
                Some(salesforce_id)
            }
        }
    }
}

/// The result of one Salesforce call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub(crate) enum RemoteOutcome {
    /// Salesforce accepted the write.
    Applied {
        /// The Salesforce ID that the call returned, if any.
        salesforce_id: Option<String>,
    },
    /// Salesforce rejected the write. A retry cannot succeed.
    Rejected {
        /// The error message stored on the task.
        message: String,
    },
    /// The call failed with a transient error. A retry can succeed.
    Retry {
        /// The error message stored on the task.
        message: String,
    },
}

/// What the finalize phase settles: a local plan, or a remote call and its
/// outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "settled", rename_all = "snake_case")]
pub(crate) enum ApplyResolution {
    /// A plan that makes no Salesforce call.
    Local {
        /// The plan. It is never [`ApplyPlan::Remote`].
        plan: ApplyPlan,
    },
    /// A Salesforce call and its outcome.
    Remote {
        /// The call that ran.
        call: RemoteCall,
        /// The outcome of the call.
        outcome: RemoteOutcome,
    },
}

/// Turns a planner decision into the plan for one apply task.
///
/// This function is pure. It reads only its arguments.
///
/// # Errors
///
/// Returns [`ForceSyncError::MissingConfiguration`] when a Salesforce write
/// needs an external ID field that the object does not configure. Returns
/// [`ForceSyncError::InvalidStoredValue`] when a delete names a stored
/// Salesforce ID that is not valid.
pub(crate) fn plan_apply(
    decision: PlanDecision,
    envelope: &ChangeEnvelope,
    existing_link: Option<&SyncLink>,
    object: &ObjectSync,
) -> Result<ApplyPlan, ForceSyncError> {
    match decision.lane {
        ApplyLane::Conflict => {
            return Ok(ApplyPlan::Conflict {
                fields: decision.conflicts,
            });
        }
        ApplyLane::Noop => return Ok(ApplyPlan::Noop),
        ApplyLane::CompositeGraph => {
            return Ok(ApplyPlan::Fail {
                reason: format!("unsupported runtime lane: {:?}", decision.lane),
            });
        }
        ApplyLane::Rest | ApplyLane::Bulk => {}
    }

    if envelope.source() == SourceSystem::Salesforce {
        let payload = match envelope.operation() {
            ChangeOperation::Upsert => decision.payload,
            ChangeOperation::Delete => None,
        };
        return Ok(ApplyPlan::ProjectLocal { payload });
    }

    let object_name = envelope.sync_key().object_name().to_owned();
    if envelope.operation() == ChangeOperation::Delete {
        let Some(salesforce_id) = existing_link.and_then(|link| link.salesforce_id.as_deref())
        else {
            return Ok(ApplyPlan::Fail {
                reason: MISSING_DELETE_ID.to_owned(),
            });
        };
        let salesforce_id = parse_salesforce_id("salesforce_id", salesforce_id)?;
        return Ok(ApplyPlan::Remote(RemoteCall::Delete {
            object_name,
            salesforce_id: salesforce_id.as_str().to_owned(),
        }));
    }

    let external_id_field = object
        .external_id_field()
        .ok_or(ForceSyncError::MissingConfiguration {
            field: "external_id_field",
        })?
        .to_owned();
    let payload = decision
        .payload
        .unwrap_or_else(|| envelope.payload().clone());

    // The bulk lane needs a valid Salesforce ID to project the link, because
    // a bulk job returns no per-record ID. Without one, the REST lane runs.
    let bulk_salesforce_id = (decision.lane == ApplyLane::Bulk)
        .then(|| existing_link.and_then(|link| link.salesforce_id.as_deref()))
        .flatten()
        .and_then(|id| SalesforceId::new(id.to_owned()).ok());

    Ok(ApplyPlan::Remote(match bulk_salesforce_id {
        Some(salesforce_id) => RemoteCall::BulkUpsert {
            object_name,
            external_id_field,
            payload,
            salesforce_id: salesforce_id.as_str().to_owned(),
        },
        None => RemoteCall::RestUpsert {
            object_name,
            external_id_field,
            external_id: envelope.sync_key().external_id().to_owned(),
            payload,
        },
    }))
}

/// Parses a stored Salesforce ID, naming `field` in the error.
pub(crate) fn parse_salesforce_id(
    field: &'static str,
    value: &str,
) -> Result<SalesforceId, ForceSyncError> {
    SalesforceId::new(value.to_owned()).map_err(|error| ForceSyncError::InvalidStoredValue {
        field,
        value: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;

    use super::*;
    use crate::identity::SyncKey;

    const SF_ID: &str = "001000000000001AAA";

    fn envelope(source: SourceSystem, operation: ChangeOperation) -> ChangeEnvelope {
        ChangeEnvelope::new(
            SyncKey::new("tenant", "Account", "ext-1")
                .unwrap_or_else(|error| panic!("unexpected sync key error: {error}")),
            source,
            operation,
            Utc::now(),
            json!({"Name": "Acme"}),
        )
    }

    fn decision(lane: ApplyLane) -> PlanDecision {
        PlanDecision {
            lane,
            payload: Some(json!({"Name": "Merged"})),
            conflicts: Vec::new(),
        }
    }

    fn link(salesforce_id: Option<&str>) -> SyncLink {
        SyncLink {
            tenant: "tenant".to_owned(),
            object_name: "Account".to_owned(),
            external_id: "ext-1".to_owned(),
            salesforce_id: salesforce_id.map(str::to_owned),
            postgres_id: None,
            last_source: None,
            last_source_cursor: None,
            last_payload_hash: None,
            tombstone: false,
        }
    }

    fn object() -> ObjectSync {
        ObjectSync::new("Account").external_id("ExternalId__c")
    }

    fn plan(
        lane: ApplyLane,
        source: SourceSystem,
        operation: ChangeOperation,
        existing: Option<&SyncLink>,
    ) -> ApplyPlan {
        plan_apply(
            decision(lane),
            &envelope(source, operation),
            existing,
            &object(),
        )
        .unwrap_or_else(|error| panic!("unexpected plan error: {error}"))
    }

    #[test]
    fn conflict_lane_carries_the_conflicting_fields() {
        let decision = PlanDecision {
            lane: ApplyLane::Conflict,
            payload: None,
            conflicts: vec!["Name".to_owned(), "Phone".to_owned()],
        };
        let plan = plan_apply(
            decision,
            &envelope(SourceSystem::Postgres, ChangeOperation::Upsert),
            None,
            &object(),
        )
        .unwrap_or_else(|error| panic!("unexpected plan error: {error}"));
        assert_eq!(
            plan,
            ApplyPlan::Conflict {
                fields: vec!["Name".to_owned(), "Phone".to_owned()]
            }
        );
    }

    #[test]
    fn noop_lane_is_noop_for_either_source() {
        for source in [SourceSystem::Postgres, SourceSystem::Salesforce] {
            assert_eq!(
                plan(ApplyLane::Noop, source, ChangeOperation::Upsert, None),
                ApplyPlan::Noop
            );
        }
    }

    #[test]
    fn composite_graph_lane_fails_as_unsupported() {
        assert_eq!(
            plan(
                ApplyLane::CompositeGraph,
                SourceSystem::Postgres,
                ChangeOperation::Upsert,
                None
            ),
            ApplyPlan::Fail {
                reason: "unsupported runtime lane: CompositeGraph".to_owned()
            }
        );
    }

    #[test]
    fn salesforce_origin_projects_locally_on_write_lanes() {
        for lane in [ApplyLane::Rest, ApplyLane::Bulk] {
            assert_eq!(
                plan(
                    lane,
                    SourceSystem::Salesforce,
                    ChangeOperation::Upsert,
                    None
                ),
                ApplyPlan::ProjectLocal {
                    payload: Some(json!({"Name": "Merged"}))
                }
            );
        }
    }

    #[test]
    fn a_local_projection_carries_the_merged_state_except_for_a_delete() {
        let merged = json!({"Name": "Acme", "Phone": "555-0100"});
        let decision = |payload: Option<Value>| PlanDecision {
            lane: ApplyLane::Rest,
            payload,
            conflicts: Vec::new(),
        };
        let upsert = envelope(SourceSystem::Salesforce, ChangeOperation::Upsert);
        let delete = envelope(SourceSystem::Salesforce, ChangeOperation::Delete);
        let object = object();

        let planned = plan_apply(decision(Some(merged.clone())), &upsert, None, &object);
        assert!(
            matches!(&planned, Ok(ApplyPlan::ProjectLocal { payload: Some(p) }) if *p == merged),
            "{planned:?}"
        );
        let planned = plan_apply(decision(Some(merged)), &delete, None, &object);
        assert!(
            matches!(planned, Ok(ApplyPlan::ProjectLocal { payload: None })),
            "{planned:?}"
        );
    }

    #[test]
    fn a_plan_recorded_without_a_payload_still_reads() {
        let plan: ApplyPlan = serde_json::from_value(json!({"kind": "project_local"}))
            .unwrap_or_else(|error| panic!("old plan: {error}"));
        assert_eq!(plan, ApplyPlan::ProjectLocal { payload: None });
    }

    #[test]
    fn rest_upsert_uses_the_merged_payload_and_external_id() {
        assert_eq!(
            plan(
                ApplyLane::Rest,
                SourceSystem::Postgres,
                ChangeOperation::Upsert,
                None
            ),
            ApplyPlan::Remote(RemoteCall::RestUpsert {
                object_name: "Account".to_owned(),
                external_id_field: "ExternalId__c".to_owned(),
                external_id: "ext-1".to_owned(),
                payload: json!({"Name": "Merged"}),
            })
        );
    }

    #[test]
    fn upsert_without_merged_payload_uses_the_envelope_payload() {
        let decision = PlanDecision {
            lane: ApplyLane::Rest,
            payload: None,
            conflicts: Vec::new(),
        };
        let plan = plan_apply(
            decision,
            &envelope(SourceSystem::Postgres, ChangeOperation::Upsert),
            None,
            &object(),
        )
        .unwrap_or_else(|error| panic!("unexpected plan error: {error}"));
        let ApplyPlan::Remote(RemoteCall::RestUpsert { payload, .. }) = plan else {
            panic!("expected a REST upsert, got {plan:?}");
        };
        assert_eq!(payload, json!({"Name": "Acme"}));
    }

    #[test]
    fn upsert_without_external_id_field_is_a_configuration_error() {
        let result = plan_apply(
            decision(ApplyLane::Rest),
            &envelope(SourceSystem::Postgres, ChangeOperation::Upsert),
            None,
            &ObjectSync::new("Account"),
        );
        assert!(matches!(
            result,
            Err(ForceSyncError::MissingConfiguration {
                field: "external_id_field"
            })
        ));
    }

    #[test]
    fn bulk_upsert_needs_a_valid_linked_salesforce_id() {
        let linked = link(Some(SF_ID));
        assert_eq!(
            plan(
                ApplyLane::Bulk,
                SourceSystem::Postgres,
                ChangeOperation::Upsert,
                Some(&linked)
            ),
            ApplyPlan::Remote(RemoteCall::BulkUpsert {
                object_name: "Account".to_owned(),
                external_id_field: "ExternalId__c".to_owned(),
                payload: json!({"Name": "Merged"}),
                salesforce_id: SF_ID.to_owned(),
            })
        );
    }

    #[test]
    fn bulk_upsert_falls_back_to_rest_without_a_usable_id() {
        let unlinked = link(None);
        let invalid = link(Some("not-an-id"));
        for existing in [None, Some(&unlinked), Some(&invalid)] {
            let plan = plan(
                ApplyLane::Bulk,
                SourceSystem::Postgres,
                ChangeOperation::Upsert,
                existing,
            );
            assert!(
                matches!(plan, ApplyPlan::Remote(RemoteCall::RestUpsert { .. })),
                "expected REST fallback, got {plan:?}"
            );
        }
    }

    #[test]
    fn delete_uses_the_linked_salesforce_id_on_either_lane() {
        let linked = link(Some(SF_ID));
        for lane in [ApplyLane::Rest, ApplyLane::Bulk] {
            assert_eq!(
                plan(
                    lane,
                    SourceSystem::Postgres,
                    ChangeOperation::Delete,
                    Some(&linked)
                ),
                ApplyPlan::Remote(RemoteCall::Delete {
                    object_name: "Account".to_owned(),
                    salesforce_id: SF_ID.to_owned(),
                })
            );
        }
    }

    #[test]
    fn delete_without_linked_id_fails_the_task() {
        let unlinked = link(None);
        for existing in [None, Some(&unlinked)] {
            assert_eq!(
                plan(
                    ApplyLane::Rest,
                    SourceSystem::Postgres,
                    ChangeOperation::Delete,
                    existing
                ),
                ApplyPlan::Fail {
                    reason: MISSING_DELETE_ID.to_owned()
                }
            );
        }
    }

    #[test]
    fn delete_with_invalid_linked_id_is_a_stored_value_error() {
        let invalid = link(Some("bogus"));
        let result = plan_apply(
            decision(ApplyLane::Rest),
            &envelope(SourceSystem::Postgres, ChangeOperation::Delete),
            Some(&invalid),
            &object(),
        );
        assert!(matches!(
            result,
            Err(ForceSyncError::InvalidStoredValue {
                field: "salesforce_id",
                ..
            })
        ));
    }

    #[test]
    fn remote_call_helpers_report_delete_and_known_id() {
        let rest = RemoteCall::RestUpsert {
            object_name: "Account".to_owned(),
            external_id_field: "ExternalId__c".to_owned(),
            external_id: "ext-1".to_owned(),
            payload: json!({}),
        };
        let delete = RemoteCall::Delete {
            object_name: "Account".to_owned(),
            salesforce_id: SF_ID.to_owned(),
        };
        assert!(!rest.is_delete());
        assert_eq!(rest.known_salesforce_id(), None);
        assert!(delete.is_delete());
        assert_eq!(delete.known_salesforce_id(), Some(SF_ID));
    }

    #[test]
    fn plans_and_resolutions_round_trip_through_json() {
        let resolutions = [
            ApplyResolution::Local {
                plan: ApplyPlan::Fail {
                    reason: "x".to_owned(),
                },
            },
            ApplyResolution::Local {
                plan: ApplyPlan::Conflict {
                    fields: vec!["Name".to_owned()],
                },
            },
            ApplyResolution::Local {
                plan: ApplyPlan::Noop,
            },
            ApplyResolution::Local {
                plan: ApplyPlan::ProjectLocal {
                    payload: Some(json!({"Name": "Acme"})),
                },
            },
            ApplyResolution::Remote {
                call: RemoteCall::BulkUpsert {
                    object_name: "Account".to_owned(),
                    external_id_field: "ExternalId__c".to_owned(),
                    payload: json!({"Name": "Acme"}),
                    salesforce_id: SF_ID.to_owned(),
                },
                outcome: RemoteOutcome::Applied {
                    salesforce_id: Some(SF_ID.to_owned()),
                },
            },
            ApplyResolution::Remote {
                call: RemoteCall::Delete {
                    object_name: "Account".to_owned(),
                    salesforce_id: SF_ID.to_owned(),
                },
                outcome: RemoteOutcome::Rejected {
                    message: "no".to_owned(),
                },
            },
        ];
        for resolution in resolutions {
            let json = serde_json::to_value(&resolution)
                .unwrap_or_else(|error| panic!("serialize failed: {error}"));
            let back: ApplyResolution = serde_json::from_value(json)
                .unwrap_or_else(|error| panic!("deserialize failed: {error}"));
            assert_eq!(back, resolution);
        }
    }
}
