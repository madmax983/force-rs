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
    envelope: ChangeEnvelope,
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
        let mut applied = 0usize;

        for task in leased {
            if self.process_leased_task(&task).await? {
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

    async fn process_leased_task(&self, task: &LeasedTask) -> Result<bool, ForceSyncError> {
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
                current_payload: None,
                batch_size: 1,
                urgent: true,
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
        match decision.lane {
            ApplyLane::Noop => {
                self.store
                    .ack_task_for_worker(&self.worker_id, task.task_id)
                    .await?;
                Ok(false)
            }
            ApplyLane::Conflict => {
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
                Ok(false)
            }
            ApplyLane::Rest | ApplyLane::Bulk => {
                let payload = decision
                    .payload
                    .as_ref()
                    .unwrap_or_else(|| envelope.payload());
                let success = self
                    .apply_rest_task(task, envelope, payload, existing_link, object)
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
                    j.tenant,
                    j.object_name,
                    j.external_id,
                    j.source,
                    j.source_cursor,
                    j.observed_at,
                    j.operation,
                    j.payload::text as payload_json
                 from sync_task t
                 join sync_journal j
                   on (t.payload->>'journal_id')::bigint = j.journal_id
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
    let tenant: String = row.get("tenant");
    let object_name: String = row.get("object_name");
    let external_id: String = row.get("external_id");
    let source: String = row.get("source");
    let source_cursor: String = row.get("source_cursor");
    let observed_at: DateTime<Utc> = row.get("observed_at");
    let operation: String = row.get("operation");
    let payload_json: String = row.get("payload_json");

    let sync_key = SyncKey::new(tenant, object_name, external_id)?;
    let payload = serde_json::from_str(&payload_json)?;
    let envelope = ChangeEnvelope::new(
        sync_key,
        parse_source_system(&source)?,
        parse_change_operation(&operation)?,
        observed_at,
        payload,
    )
    .with_cursor(parse_source_cursor(&source_cursor)?);

    Ok(ApplyTaskContext { envelope })
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
