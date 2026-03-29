//! Narrow runtime orchestration for the v0.1 vertical slice.

use std::{collections::HashMap, time::Duration};

use chrono::{DateTime, Utc};
use force::{auth::Authenticator, client::ForceClient};
use serde_json::Value;

use crate::{
    apply::{
        postgres::project_sync_link,
        salesforce::{ApplyError, SalesforceApplier},
    },
    capture,
    config::ObjectSync,
    error::ForceSyncError,
    identity::SyncKey,
    model::{ChangeEnvelope, ChangeOperation, SourceCursor, SourceSystem},
    plan::{ApplyLane, PlannerContext, plan_change},
    reconcile,
    store::pg::{LeasedTask, PgStore, SyncConflict},
};

struct ApplyTaskContext {
    _journal_id: i64,
    envelope: ChangeEnvelope,
    current_payload: Option<Value>,
}

/// Runtime sync engine for the explicit Postgres-to-Salesforce vertical slice.
#[derive(Debug, Clone)]
pub struct SyncEngine<A: Authenticator> {
    store: PgStore,
    salesforce: SalesforceApplier<A>,
    objects: HashMap<String, ObjectSync>,
    capture_batch_size: i64,
    capture_priority: i32,
    apply_batch_size: i64,
    reconcile_batch_size: i64,
    lease_for: Duration,
    worker_id: String,
}

/// Builder for [`SyncEngine`].
#[derive(Debug)]
pub struct SyncEngineBuilder<A: Authenticator> {
    salesforce_client: ForceClient<A>,
    postgres: Option<PgStore>,
    objects: Vec<ObjectSync>,
    capture_batch_size: i64,
    capture_priority: i32,
    apply_batch_size: i64,
    reconcile_batch_size: i64,
    lease_for: Duration,
    worker_id: String,
}

impl<A: Authenticator> SyncEngine<A> {
    /// Creates a builder for a runtime using the given Salesforce client.
    #[must_use]
    pub fn builder(salesforce_client: ForceClient<A>) -> SyncEngineBuilder<A> {
        SyncEngineBuilder::new(salesforce_client)
    }

    /// Captures a single configured batch from the Postgres outbox.
    ///
    /// # Errors
    ///
    /// Returns an error if the outbox capture fails.
    pub async fn run_capture_postgres_once(&self) -> Result<usize, ForceSyncError> {
        capture::postgres::capture_batch(
            &self.store,
            self.capture_batch_size,
            self.capture_priority,
        )
        .await
    }

    /// Leases and applies one configured batch of ready tasks.
    ///
    /// # Errors
    ///
    /// Returns an error if task loading or control-plane updates fail.
    pub async fn run_apply_once(&self) -> Result<usize, ForceSyncError> {
        let leased = self
            .store
            .lease_ready_tasks(&self.worker_id, self.apply_batch_size, self.lease_for)
            .await?;
        let batch_size = leased.len().max(1);
        let mut applied = 0usize;

        for task in leased {
            if self.process_leased_task(&task, batch_size).await? {
                applied += 1;
            }
        }

        Ok(applied)
    }

    /// Runs one reconciliation pass and enqueues repair tasks for drift.
    ///
    /// # Errors
    ///
    /// Returns an error if drift detection or task insertion fails.
    pub async fn run_reconcile_once(&self) -> Result<usize, ForceSyncError> {
        reconcile::run_reconcile_once(&self.store, self.reconcile_batch_size.max(1)).await
    }

    async fn process_leased_task(
        &self,
        task: &LeasedTask,
        batch_size: usize,
    ) -> Result<bool, ForceSyncError> {
        let context = self.load_apply_task_context(task.task_id).await?;
        let Some(context) = context else {
            let _ = self
                .store
                .fail_task_for_worker(
                    &self.worker_id,
                    task.task_id,
                    "missing apply task journal row",
                )
                .await?;
            return Ok(false);
        };

        let object = self
            .objects
            .get(context.envelope.sync_key().object_name())
            .ok_or(ForceSyncError::MissingConfiguration {
                field: "object sync",
            })?;

        let existing_link = self
            .store
            .get_link(
                context.envelope.sync_key().tenant(),
                context.envelope.sync_key().object_name(),
                context.envelope.sync_key().external_id(),
            )
            .await?;

        let decision = plan_change(
            &PlannerContext {
                object: object.clone(),
                current_payload: context.current_payload.clone(),
                batch_size,
                urgent: batch_size <= object.lane_thresholds().rest_max_batch_size(),
                has_dependencies: false,
            },
            &context.envelope,
        );

        self.apply_planned_task(
            task,
            &context.envelope,
            existing_link.as_ref(),
            object,
            decision,
        )
        .await
    }

    async fn apply_planned_task(
        &self,
        task: &LeasedTask,
        envelope: &ChangeEnvelope,
        existing_link: Option<&crate::store::pg::SyncLink>,
        object: &ObjectSync,
        decision: crate::plan::PlanDecision,
    ) -> Result<bool, ForceSyncError> {
        if decision.lane == ApplyLane::Conflict {
            for field_name in &decision.conflicts {
                let conflict = SyncConflict {
                    tenant: envelope.sync_key().tenant().to_owned(),
                    object_name: envelope.sync_key().object_name().to_owned(),
                    external_id: envelope.sync_key().external_id().to_owned(),
                    field_name: field_name.clone(),
                    left_value: Value::Null,
                    right_value: Value::Null,
                    resolution: None,
                };
                self.store.insert_conflict(&conflict).await?;
            }
            let _ = self
                .store
                .fail_task_for_worker(
                    &self.worker_id,
                    task.task_id,
                    format!("planner conflict: {}", decision.conflicts.join(",")),
                )
                .await?;
            return Ok(false);
        }

        if decision.lane == ApplyLane::Noop {
            self.store
                .ack_task_for_worker(&self.worker_id, task.task_id)
                .await?;
            return Ok(false);
        }

        if should_project_locally(envelope.source(), decision.lane) {
            return self.apply_local_task(task, envelope, existing_link).await;
        }

        match decision.lane {
            ApplyLane::Rest => {
                let payload = decision
                    .payload
                    .as_ref()
                    .unwrap_or_else(|| envelope.payload());
                let success = self
                    .apply_rest_task(task, envelope, payload, existing_link, object)
                    .await?;
                Ok(success)
            }
            ApplyLane::Bulk => {
                let payload = decision
                    .payload
                    .as_ref()
                    .unwrap_or_else(|| envelope.payload());
                let success = self
                    .apply_bulk_task(task, envelope, payload, existing_link, object)
                    .await?;
                Ok(success)
            }
            ApplyLane::CompositeGraph => {
                let _ = self
                    .store
                    .fail_task_for_worker(
                        &self.worker_id,
                        task.task_id,
                        format!("unsupported runtime lane: {:?}", decision.lane),
                    )
                    .await?;
                Ok(false)
            }
            ApplyLane::Conflict | ApplyLane::Noop => unreachable!("handled above"),
        }
    }

    async fn apply_local_task(
        &self,
        task: &LeasedTask,
        envelope: &ChangeEnvelope,
        existing_link: Option<&crate::store::pg::SyncLink>,
    ) -> Result<bool, ForceSyncError> {
        let salesforce_id = local_projection_salesforce_id(existing_link, envelope)?;
        let link = project_sync_link(
            existing_link,
            envelope,
            salesforce_id.as_ref(),
            matches!(envelope.operation(), ChangeOperation::Delete),
        );
        self.store.put_link(&link).await?;
        self.store
            .ack_task_for_worker(&self.worker_id, task.task_id)
            .await?;
        Ok(true)
    }

    async fn apply_bulk_task(
        &self,
        task: &LeasedTask,
        envelope: &ChangeEnvelope,
        payload: &Value,
        existing_link: Option<&crate::store::pg::SyncLink>,
        object: &ObjectSync,
    ) -> Result<bool, ForceSyncError> {
        match envelope.operation() {
            ChangeOperation::Upsert => {
                let Some(existing_salesforce_id) =
                    existing_link.and_then(|link| link.salesforce_id.as_deref())
                else {
                    return self
                        .apply_rest_task(task, envelope, payload, existing_link, object)
                        .await;
                };

                let job_result = self
                    .salesforce
                    .apply_bulk_upsert(
                        envelope.sync_key().object_name(),
                        object
                            .external_id_field()
                            .ok_or(ForceSyncError::MissingConfiguration {
                                field: "external_id_field",
                            })?,
                        1,
                        vec![payload.clone()],
                    )
                    .await;
                if let Err(error) = job_result {
                    return self.handle_apply_error(task, error).await;
                }

                let Some(salesforce_id) =
                    force::types::SalesforceId::new(existing_salesforce_id.to_owned()).ok()
                else {
                    return self
                        .apply_rest_task(task, envelope, payload, existing_link, object)
                        .await;
                };

                let link = project_sync_link(existing_link, envelope, Some(&salesforce_id), false);
                self.store.put_link(&link).await?;
                self.store
                    .ack_task_for_worker(&self.worker_id, task.task_id)
                    .await?;
                Ok(true)
            }
            ChangeOperation::Delete => {
                self.apply_rest_task(task, envelope, payload, existing_link, object)
                    .await
            }
        }
    }

    async fn apply_rest_task(
        &self,
        task: &LeasedTask,
        envelope: &ChangeEnvelope,
        payload: &Value,
        existing_link: Option<&crate::store::pg::SyncLink>,
        object: &ObjectSync,
    ) -> Result<bool, ForceSyncError> {
        match envelope.operation() {
            ChangeOperation::Upsert => {
                let result = self
                    .salesforce
                    .apply_rest_upsert(
                        envelope.sync_key().object_name(),
                        object
                            .external_id_field()
                            .ok_or(ForceSyncError::MissingConfiguration {
                                field: "external_id_field",
                            })?,
                        envelope.sync_key().external_id(),
                        payload,
                    )
                    .await;

                match result {
                    Ok(result) => {
                        let link = project_sync_link(
                            existing_link,
                            envelope,
                            result.salesforce_id.as_ref(),
                            false,
                        );
                        self.store.put_link(&link).await?;
                        self.store
                            .ack_task_for_worker(&self.worker_id, task.task_id)
                            .await?;
                        Ok(true)
                    }
                    Err(error) => self.handle_apply_error(task, error).await,
                }
            }
            ChangeOperation::Delete => {
                let Some(existing_link) = existing_link else {
                    let _ = self
                        .store
                        .fail_task_for_worker(
                            &self.worker_id,
                            task.task_id,
                            "missing Salesforce ID for delete",
                        )
                        .await?;
                    return Ok(false);
                };
                let Some(salesforce_id) = existing_link.salesforce_id.as_deref() else {
                    let _ = self
                        .store
                        .fail_task_for_worker(
                            &self.worker_id,
                            task.task_id,
                            "missing Salesforce ID for delete",
                        )
                        .await?;
                    return Ok(false);
                };
                let salesforce_id = force::types::SalesforceId::new(salesforce_id.to_owned())
                    .map_err(|error| ForceSyncError::InvalidStoredValue {
                        field: "salesforce_id",
                        value: error.to_string(),
                    })?;

                match self
                    .salesforce
                    .apply_rest_delete(envelope.sync_key().object_name(), &salesforce_id)
                    .await
                {
                    Ok(()) => {
                        let link = project_sync_link(
                            existing_link.into(),
                            envelope,
                            Some(&salesforce_id),
                            true,
                        );
                        self.store.put_link(&link).await?;
                        self.store
                            .ack_task_for_worker(&self.worker_id, task.task_id)
                            .await?;
                        Ok(true)
                    }
                    Err(error) => self.handle_apply_error(task, error).await,
                }
            }
        }
    }

    async fn handle_apply_error(
        &self,
        task: &LeasedTask,
        error: ApplyError,
    ) -> Result<bool, ForceSyncError> {
        let error_message = error.to_string();
        match error {
            ApplyError::Retryable(_) => {
                self.store
                    .retry_task_for_worker(
                        &self.worker_id,
                        task.task_id,
                        Utc::now() + chrono::Duration::seconds(30),
                        error_message,
                    )
                    .await?;
            }
            ApplyError::Permanent(_) => {
                self.store
                    .fail_task_for_worker(&self.worker_id, task.task_id, error_message)
                    .await?;
            }
        }

        Ok(false)
    }

    async fn load_apply_task_context(
        &self,
        task_id: i64,
    ) -> Result<Option<ApplyTaskContext>, ForceSyncError> {
        let client = self.store.pool().get().await?;
        let row = client
            .query_opt(
                "select
                    j.journal_id,
                    j.tenant,
                    j.object_name,
                    j.external_id,
                    j.source,
                    j.source_cursor,
                    j.observed_at,
                    j.operation,
                    j.payload::text as payload_json,
                    applied.payload::text as current_payload_json
                 from sync_task t
                 join sync_journal j
                   on (t.payload->>'journal_id')::bigint = j.journal_id
                 left join sync_link link
                   on link.tenant = j.tenant
                  and link.object_name = j.object_name
                  and link.external_id = j.external_id
                 left join lateral (
                    select payload
                    from sync_journal applied
                    where applied.tenant = j.tenant
                      and applied.object_name = j.object_name
                      and applied.external_id = j.external_id
                      and link.last_payload_hash is not null
                      and applied.payload_hash = link.last_payload_hash
                    order by applied.journal_id desc
                    limit 1
                 ) applied on true
                 where t.task_id = $1",
                &[&task_id],
            )
            .await?;

        row.as_ref().map(build_apply_task_context).transpose()
    }
}

impl<A: Authenticator> SyncEngineBuilder<A> {
    fn new(salesforce_client: ForceClient<A>) -> Self {
        Self {
            salesforce_client,
            postgres: None,
            objects: Vec::new(),
            capture_batch_size: 100,
            capture_priority: 0,
            apply_batch_size: 1,
            reconcile_batch_size: 100,
            lease_for: Duration::from_secs(30),
            worker_id: "force-sync-apply".to_owned(),
        }
    }

    /// Sets the Postgres control-plane store.
    #[must_use]
    pub fn postgres(mut self, postgres: PgStore) -> Self {
        self.postgres = Some(postgres);
        self
    }

    /// Adds an object sync definition.
    #[must_use]
    pub fn object(mut self, object: ObjectSync) -> Self {
        self.objects.push(object);
        self
    }

    /// Sets the number of records scanned per reconciliation pass.
    #[must_use]
    pub const fn reconcile_batch_size(mut self, reconcile_batch_size: i64) -> Self {
        self.reconcile_batch_size = reconcile_batch_size;
        self
    }

    /// Sets the number of tasks leased for each apply pass.
    #[must_use]
    pub const fn apply_batch_size(mut self, apply_batch_size: i64) -> Self {
        self.apply_batch_size = apply_batch_size;
        self
    }

    /// Builds the runtime engine.
    ///
    /// # Errors
    ///
    /// Returns an error if required runtime configuration is missing.
    pub fn build(self) -> Result<SyncEngine<A>, ForceSyncError> {
        let store = self
            .postgres
            .ok_or(ForceSyncError::MissingConfiguration { field: "postgres" })?;
        if self.objects.is_empty() {
            return Err(ForceSyncError::MissingConfiguration {
                field: "object sync",
            });
        }

        Ok(SyncEngine {
            store,
            salesforce: SalesforceApplier::new(self.salesforce_client),
            objects: self
                .objects
                .into_iter()
                .map(|object| (object.object_name().to_owned(), object))
                .collect(),
            capture_batch_size: self.capture_batch_size,
            capture_priority: self.capture_priority,
            apply_batch_size: self.apply_batch_size,
            reconcile_batch_size: self.reconcile_batch_size,
            lease_for: self.lease_for,
            worker_id: self.worker_id,
        })
    }
}

fn build_apply_task_context(row: &tokio_postgres::Row) -> Result<ApplyTaskContext, ForceSyncError> {
    let journal_id: i64 = row.get("journal_id");
    let tenant: String = row.get("tenant");
    let object_name: String = row.get("object_name");
    let external_id: String = row.get("external_id");
    let source: String = row.get("source");
    let source_cursor: String = row.get("source_cursor");
    let observed_at: DateTime<Utc> = row.get("observed_at");
    let operation: String = row.get("operation");
    let payload_json: String = row.get("payload_json");
    let current_payload_json: Option<String> = row.get("current_payload_json");

    let sync_key = SyncKey::new(tenant, object_name, external_id)?;
    let payload = serde_json::from_str(&payload_json)?;
    let current_payload = match current_payload_json {
        Some(current_payload_json) => Some(serde_json::from_str(&current_payload_json)?),
        None => None,
    };
    let envelope = ChangeEnvelope::new(
        sync_key,
        parse_source_system(&source)?,
        parse_change_operation(&operation)?,
        observed_at,
        payload,
    )
    .with_cursor(parse_source_cursor(&source_cursor)?);

    Ok(ApplyTaskContext {
        _journal_id: journal_id,
        envelope,
        current_payload,
    })
}

fn should_project_locally(source: SourceSystem, lane: ApplyLane) -> bool {
    source == SourceSystem::Salesforce && !matches!(lane, ApplyLane::Noop | ApplyLane::Conflict)
}

fn local_projection_salesforce_id(
    existing_link: Option<&crate::store::pg::SyncLink>,
    envelope: &ChangeEnvelope,
) -> Result<Option<force::types::SalesforceId>, ForceSyncError> {
    if let Some(existing_salesforce_id) =
        existing_link.and_then(|link| link.salesforce_id.as_deref())
    {
        return force::types::SalesforceId::new(existing_salesforce_id.to_owned())
            .map(Some)
            .map_err(|error| ForceSyncError::InvalidStoredValue {
                field: "salesforce_id",
                value: error.to_string(),
            });
    }

    let Some(payload_salesforce_id) = envelope.payload().get("Id").and_then(Value::as_str) else {
        return Ok(None);
    };

    force::types::SalesforceId::new(payload_salesforce_id.to_owned())
        .map(Some)
        .map_err(|error| ForceSyncError::InvalidStoredValue {
            field: "payload.Id",
            value: error.to_string(),
        })
}

fn parse_source_system(value: &str) -> Result<SourceSystem, ForceSyncError> {
    match value {
        "salesforce" => Ok(SourceSystem::Salesforce),
        "postgres" => Ok(SourceSystem::Postgres),
        _ => Err(ForceSyncError::InvalidStoredValue {
            field: "source",
            value: value.to_owned(),
        }),
    }
}

fn parse_change_operation(value: &str) -> Result<ChangeOperation, ForceSyncError> {
    match value {
        "upsert" => Ok(ChangeOperation::Upsert),
        "delete" => Ok(ChangeOperation::Delete),
        _ => Err(ForceSyncError::InvalidStoredValue {
            field: "operation",
            value: value.to_owned(),
        }),
    }
}

fn parse_source_cursor(value: &str) -> Result<SourceCursor, ForceSyncError> {
    if let Some(replay_id) = value.strip_prefix("salesforce-replay-id:") {
        let replay_id =
            replay_id
                .parse::<i64>()
                .map_err(|_| ForceSyncError::InvalidStoredValue {
                    field: "source_cursor",
                    value: value.to_owned(),
                })?;
        return Ok(SourceCursor::SalesforceReplayId(replay_id));
    }

    if let Some(lsn) = value.strip_prefix("postgres-lsn:") {
        return Ok(SourceCursor::PostgresLsn(lsn.to_owned()));
    }

    if let Some(snapshot) = value.strip_prefix("snapshot:") {
        return Ok(SourceCursor::Snapshot(snapshot.to_owned()));
    }

    Err(ForceSyncError::InvalidStoredValue {
        field: "source_cursor",
        value: value.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;

    use super::{
        local_projection_salesforce_id, parse_change_operation, parse_source_cursor,
        parse_source_system, should_project_locally,
    };
    use crate::{
        identity::SyncKey,
        model::{ChangeEnvelope, ChangeOperation, SourceCursor, SourceSystem},
        plan::ApplyLane,
        store::pg::SyncLink,
    };

    fn envelope(payload: serde_json::Value) -> ChangeEnvelope {
        ChangeEnvelope::new(
            SyncKey::new("tenant", "Account", "external-1")
                .unwrap_or_else(|error| panic!("unexpected sync key construction error: {error}")),
            SourceSystem::Salesforce,
            ChangeOperation::Upsert,
            Utc::now(),
            payload,
        )
    }

    #[test]
    fn should_not_project_local_noops_back_to_salesforce() {
        assert!(!should_project_locally(
            SourceSystem::Salesforce,
            ApplyLane::Noop
        ));
        assert!(!should_project_locally(
            SourceSystem::Postgres,
            ApplyLane::Rest
        ));
        assert!(should_project_locally(
            SourceSystem::Salesforce,
            ApplyLane::Rest
        ));
    }

    #[test]
    fn local_projection_salesforce_id_uses_payload_id_when_link_is_missing() {
        let envelope = envelope(json!({
            "Id": "001000000000009AAA",
            "Name": "Incoming Salesforce"
        }));

        let salesforce_id = local_projection_salesforce_id(None, &envelope)
            .unwrap_or_else(|error| panic!("unexpected projection error: {error}"));

        assert_eq!(
            salesforce_id
                .as_ref()
                .map(force::types::SalesforceId::as_str),
            Some("001000000000009AAA")
        );
    }

    #[test]
    fn local_projection_salesforce_id_prefers_existing_link() {
        let envelope = envelope(json!({
            "Id": "001000000000009AAA",
            "Name": "Incoming Salesforce"
        }));
        let existing_link = SyncLink {
            tenant: "tenant".to_owned(),
            object_name: "Account".to_owned(),
            external_id: "external-1".to_owned(),
            salesforce_id: Some("001000000000001AAA".to_owned()),
            postgres_id: None,
            last_source: Some("postgres".to_owned()),
            last_source_cursor: Some("postgres-lsn:1".to_owned()),
            last_payload_hash: None,
            tombstone: false,
        };

        let salesforce_id = local_projection_salesforce_id(Some(&existing_link), &envelope)
            .unwrap_or_else(|error| panic!("unexpected projection error: {error}"));

        assert_eq!(
            salesforce_id
                .as_ref()
                .map(force::types::SalesforceId::as_str),
            Some("001000000000001AAA")
        );
    }

    #[test]
    fn local_projection_returns_none_when_no_link_and_no_payload_id() {
        let envelope = envelope(json!({"Name": "No Id Field"}));
        let salesforce_id = local_projection_salesforce_id(None, &envelope)
            .unwrap_or_else(|error| panic!("unexpected projection error: {error}"));
        assert!(salesforce_id.is_none());
    }

    // -- parse_source_system tests --

    #[test]
    fn parse_source_system_salesforce() {
        assert_eq!(
            parse_source_system("salesforce").unwrap_or_else(|e| panic!("unexpected error: {e}")),
            SourceSystem::Salesforce,
        );
    }

    #[test]
    fn parse_source_system_postgres() {
        assert_eq!(
            parse_source_system("postgres").unwrap_or_else(|e| panic!("unexpected error: {e}")),
            SourceSystem::Postgres,
        );
    }

    #[test]
    fn parse_source_system_unknown_returns_error() {
        let result = parse_source_system("oracle");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("oracle"));
    }

    // -- parse_change_operation tests --

    #[test]
    fn parse_change_operation_upsert() {
        assert_eq!(
            parse_change_operation("upsert").unwrap_or_else(|e| panic!("unexpected error: {e}")),
            ChangeOperation::Upsert,
        );
    }

    #[test]
    fn parse_change_operation_delete() {
        assert_eq!(
            parse_change_operation("delete").unwrap_or_else(|e| panic!("unexpected error: {e}")),
            ChangeOperation::Delete,
        );
    }

    #[test]
    fn parse_change_operation_unknown_returns_error() {
        let result = parse_change_operation("insert");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("insert"));
    }

    // -- parse_source_cursor tests --

    #[test]
    fn parse_source_cursor_salesforce_replay_id() {
        let cursor = parse_source_cursor("salesforce-replay-id:42")
            .unwrap_or_else(|e| panic!("unexpected error: {e}"));
        assert_eq!(cursor, SourceCursor::SalesforceReplayId(42));
    }

    #[test]
    fn parse_source_cursor_postgres_lsn() {
        let cursor = parse_source_cursor("postgres-lsn:0/16B3748")
            .unwrap_or_else(|e| panic!("unexpected error: {e}"));
        assert_eq!(cursor, SourceCursor::PostgresLsn("0/16B3748".to_owned()));
    }

    #[test]
    fn parse_source_cursor_snapshot() {
        let cursor = parse_source_cursor("snapshot:2024-01-01T00:00:00Z")
            .unwrap_or_else(|e| panic!("unexpected error: {e}"));
        assert_eq!(
            cursor,
            SourceCursor::Snapshot("2024-01-01T00:00:00Z".to_owned())
        );
    }

    #[test]
    fn parse_source_cursor_invalid_replay_id_returns_error() {
        let result = parse_source_cursor("salesforce-replay-id:not-a-number");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("source_cursor"));
    }

    #[test]
    fn parse_source_cursor_unknown_prefix_returns_error() {
        let result = parse_source_cursor("kafka-offset:99");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("kafka-offset:99"));
    }

    // -- should_project_locally additional coverage --

    #[test]
    fn should_project_locally_salesforce_bulk() {
        assert!(should_project_locally(
            SourceSystem::Salesforce,
            ApplyLane::Bulk
        ));
    }

    #[test]
    fn should_not_project_locally_salesforce_conflict() {
        assert!(!should_project_locally(
            SourceSystem::Salesforce,
            ApplyLane::Conflict
        ));
    }

    #[test]
    fn should_not_project_locally_postgres_bulk() {
        assert!(!should_project_locally(
            SourceSystem::Postgres,
            ApplyLane::Bulk
        ));
    }

    // -- SyncEngineBuilder validation tests --

    mod builder_tests {
        use async_trait::async_trait;
        use force::{
            auth::{AccessToken, Authenticator, TokenResponse},
            client::builder,
            error::Result as ForceResult,
        };

        use crate::{config::ObjectSync, error::ForceSyncError, runtime::SyncEngine};

        #[derive(Debug, Clone)]
        struct StubAuth;

        #[async_trait]
        impl Authenticator for StubAuth {
            async fn authenticate(&self) -> ForceResult<AccessToken> {
                Ok(AccessToken::from_response(TokenResponse {
                    access_token: "stub".to_owned(),
                    instance_url: "https://stub.salesforce.com".to_owned(),
                    token_type: "Bearer".to_owned(),
                    issued_at: "1704067200000".to_owned(),
                    signature: "stub".to_owned(),
                    expires_in: Some(7200),
                    refresh_token: None,
                }))
            }

            async fn refresh(&self) -> ForceResult<AccessToken> {
                self.authenticate().await
            }
        }

        #[tokio::test]
        async fn build_fails_without_postgres() {
            let client = builder()
                .authenticate(StubAuth)
                .build()
                .await
                .unwrap_or_else(|e| panic!("unexpected client build error: {e}"));

            let result = SyncEngine::builder(client)
                .object(ObjectSync::new("Account").external_id("ExternalId__c"))
                .build();

            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(
                matches!(
                    err,
                    ForceSyncError::MissingConfiguration { field: "postgres" }
                ),
                "expected MissingConfiguration for postgres, got: {err}"
            );
        }

        #[tokio::test]
        async fn build_fails_with_empty_objects() {
            let client = builder()
                .authenticate(StubAuth)
                .build()
                .await
                .unwrap_or_else(|e| panic!("unexpected client build error: {e}"));

            // Create a dummy PgStore -- it won't be used for queries, only for builder validation.
            let mut config = deadpool_postgres::Config::new();
            config.url = Some("postgresql://unused:unused@localhost:5432/unused".to_owned());
            let pool = config
                .create_pool(
                    Some(deadpool_postgres::Runtime::Tokio1),
                    tokio_postgres::NoTls,
                )
                .unwrap_or_else(|e| panic!("unexpected pool error: {e}"));
            let store = crate::store::pg::PgStore::new(pool);

            let result = SyncEngine::builder(client).postgres(store).build();

            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(
                matches!(
                    err,
                    ForceSyncError::MissingConfiguration {
                        field: "object sync"
                    }
                ),
                "expected MissingConfiguration for object sync, got: {err}"
            );
        }
    }
}
