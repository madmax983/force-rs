//! Narrow runtime orchestration for the v0.1 vertical slice.

use std::{collections::HashMap, time::Duration};

use chrono::{DateTime, Utc};
use force::{auth::Authenticator, client::ForceClient};
use serde_json::Value;

use crate::{
    ObjectSync, PlannerContext,
    apply::{
        postgres::project_sync_link,
        salesforce::{ApplyError, SalesforceApplier},
        steps::{
            ApplyPlan, ApplyResolution, MISSING_JOURNAL_ROW, RemoteCall, RemoteOutcome,
            parse_salesforce_id, plan_apply,
        },
    },
    capture::{
        self,
        salesforce::{CdcSource, CdcStream, CdcWindow, CdcWindowReport},
    },
    error::ForceSyncError,
    identity::SyncKey,
    model::{ChangeEnvelope, ChangeOperation, SourceCursor, SourceSystem},
    plan_change, reconcile,
    store::pg::{LeasedTask, PgStore, SyncConflict, SyncLink},
};

struct ApplyTaskContext {
    _journal_id: i64,
    envelope: ChangeEnvelope,
    current_payload: Option<Value>,
}

/// The journal row, current payload and sync link behind one apply task.
struct ApplySubject {
    envelope: ChangeEnvelope,
    current_payload: Option<Value>,
    existing_link: Option<SyncLink>,
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

    /// The control-plane store.
    #[cfg(feature = "durable")]
    pub(crate) const fn store(&self) -> &PgStore {
        &self.store
    }

    /// The sync config of `object_name`, if the engine syncs it.
    #[cfg(feature = "durable")]
    pub(crate) fn object(&self, object_name: &str) -> Option<&ObjectSync> {
        self.objects.get(object_name)
    }

    /// The number of outbox rows captured per batch.
    #[cfg(feature = "durable")]
    pub(crate) const fn capture_batch_size_value(&self) -> i64 {
        self.capture_batch_size
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

    /// Captures one bounded window of a Salesforce CDC stream.
    ///
    /// The stream object must be one of the configured objects. See
    /// [`crate::capture_cdc_window`] for the window semantics.
    ///
    /// # Errors
    ///
    /// Returns [`ForceSyncError::MissingConfiguration`] when the stream object
    /// is not configured, and any error of [`crate::capture_cdc_window`].
    pub async fn run_cdc_window(
        &self,
        source: &dyn CdcSource,
        stream: &CdcStream,
        window: CdcWindow,
    ) -> Result<CdcWindowReport, ForceSyncError> {
        let object =
            self.objects
                .get(stream.object_name())
                .ok_or(ForceSyncError::MissingConfiguration {
                    field: "object sync",
                })?;
        capture::salesforce::capture_cdc_window(&self.store, source, stream, object, window).await
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
        let subject = self.load_apply_subject(task.task_id).await?;
        let plan = self.plan_subject(subject.as_ref(), batch_size)?;
        let resolution = match plan {
            ApplyPlan::Remote(call) => {
                let outcome = self.execute_remote(&call).await;
                ApplyResolution::Remote { call, outcome }
            }
            plan => ApplyResolution::Local { plan },
        };
        self.finalize_subject(&self.worker_id, task.task_id, subject.as_ref(), &resolution)
            .await
    }

    /// Prepare phase: plans one apply task without writing anything.
    ///
    /// `batch_size` is the number of tasks dispatched together. The planner
    /// uses it to choose between the REST and bulk lanes.
    #[cfg(feature = "durable")]
    pub(crate) async fn prepare_apply(
        &self,
        task_id: i64,
        batch_size: usize,
    ) -> Result<ApplyPlan, ForceSyncError> {
        let subject = self.load_apply_subject(task_id).await?;
        self.plan_subject(subject.as_ref(), batch_size)
    }

    /// Finalize phase: writes the link and settles the task for `lease_owner`.
    ///
    /// It reloads the journal row and the link, so it projects onto the
    /// current link even when it runs long after the prepare phase. Returns
    /// whether the task applied a change.
    #[cfg(feature = "durable")]
    pub(crate) async fn finalize_apply(
        &self,
        lease_owner: &str,
        task_id: i64,
        resolution: &ApplyResolution,
    ) -> Result<bool, ForceSyncError> {
        let subject = self.load_apply_subject(task_id).await?;
        self.finalize_subject(lease_owner, task_id, subject.as_ref(), resolution)
            .await
    }

    /// Execute phase: makes the one Salesforce call that `call` names.
    pub(crate) async fn execute_remote(&self, call: &RemoteCall) -> RemoteOutcome {
        let result = match call {
            RemoteCall::RestUpsert {
                object_name,
                external_id_field,
                external_id,
                payload,
            } => self
                .salesforce
                .apply_rest_upsert(object_name, external_id_field, external_id, payload)
                .await
                .map(|result| result.salesforce_id.map(|id| id.as_str().to_owned())),
            RemoteCall::BulkUpsert {
                object_name,
                external_id_field,
                payload,
                salesforce_id,
            } => self
                .salesforce
                .apply_bulk_upsert(object_name, external_id_field, 1, vec![payload.clone()])
                .await
                .map(|_| Some(salesforce_id.clone())),
            RemoteCall::Delete {
                object_name,
                salesforce_id,
            } => match parse_salesforce_id("salesforce_id", salesforce_id) {
                Ok(id) => self
                    .salesforce
                    .apply_rest_delete(object_name, &id)
                    .await
                    .map(|()| Some(salesforce_id.clone())),
                Err(error) => {
                    return RemoteOutcome::Rejected {
                        message: error.to_string(),
                    };
                }
            },
        };

        match result {
            Ok(salesforce_id) => RemoteOutcome::Applied { salesforce_id },
            Err(error @ ApplyError::Retryable(_)) => RemoteOutcome::Retry {
                message: error.to_string(),
            },
            Err(error @ ApplyError::Permanent(_)) => RemoteOutcome::Rejected {
                message: error.to_string(),
            },
        }
    }

    fn plan_subject(
        &self,
        subject: Option<&ApplySubject>,
        batch_size: usize,
    ) -> Result<ApplyPlan, ForceSyncError> {
        let Some(subject) = subject else {
            return Ok(ApplyPlan::Fail {
                reason: MISSING_JOURNAL_ROW.to_owned(),
            });
        };

        let object = self
            .objects
            .get(subject.envelope.sync_key().object_name())
            .ok_or(ForceSyncError::MissingConfiguration {
                field: "object sync",
            })?;

        let decision = plan_change(
            &PlannerContext {
                object: object.clone(),
                current_payload: subject.current_payload.clone(),
                batch_size,
                urgent: batch_size <= object.lane_thresholds().rest_max_batch_size(),
                has_dependencies: false,
            },
            &subject.envelope,
        );

        plan_apply(
            decision,
            &subject.envelope,
            subject.existing_link.as_ref(),
            object,
        )
    }

    async fn finalize_subject(
        &self,
        lease_owner: &str,
        task_id: i64,
        subject: Option<&ApplySubject>,
        resolution: &ApplyResolution,
    ) -> Result<bool, ForceSyncError> {
        let plan = match resolution {
            ApplyResolution::Local { plan } => plan,
            ApplyResolution::Remote { call, outcome } => {
                return self
                    .finalize_remote(lease_owner, task_id, subject, call, outcome)
                    .await;
            }
        };

        match plan {
            ApplyPlan::Fail { reason } => {
                self.store
                    .fail_task_for_worker(lease_owner, task_id, reason)
                    .await?;
                Ok(false)
            }
            ApplyPlan::Noop => {
                self.store.ack_task_for_worker(lease_owner, task_id).await?;
                Ok(false)
            }
            ApplyPlan::Conflict { fields } => {
                let Some(subject) = subject else {
                    return self.fail_missing_journal(lease_owner, task_id).await;
                };
                let key = subject.envelope.sync_key();
                for field_name in fields {
                    self.store
                        .insert_conflict(&SyncConflict {
                            tenant: key.tenant().to_owned(),
                            object_name: key.object_name().to_owned(),
                            external_id: key.external_id().to_owned(),
                            field_name: field_name.clone(),
                            left_value: Value::Null,
                            right_value: Value::Null,
                            resolution: None,
                        })
                        .await?;
                }
                self.store
                    .fail_task_for_worker(
                        lease_owner,
                        task_id,
                        format!("planner conflict: {}", fields.join(",")),
                    )
                    .await?;
                Ok(false)
            }
            ApplyPlan::ProjectLocal => {
                let Some(subject) = subject else {
                    return self.fail_missing_journal(lease_owner, task_id).await;
                };
                let existing_link = subject.existing_link.as_ref();
                let salesforce_id =
                    local_projection_salesforce_id(existing_link, &subject.envelope)?;
                let link = project_sync_link(
                    existing_link,
                    &subject.envelope,
                    salesforce_id.as_ref(),
                    matches!(subject.envelope.operation(), ChangeOperation::Delete),
                );
                self.store.put_link(&link).await?;
                self.store.ack_task_for_worker(lease_owner, task_id).await?;
                Ok(true)
            }
            ApplyPlan::Remote(call) => Err(ForceSyncError::InvalidStoredValue {
                field: "apply resolution",
                value: format!("remote call without an outcome: {call:?}"),
            }),
        }
    }

    async fn finalize_remote(
        &self,
        lease_owner: &str,
        task_id: i64,
        subject: Option<&ApplySubject>,
        call: &RemoteCall,
        outcome: &RemoteOutcome,
    ) -> Result<bool, ForceSyncError> {
        match outcome {
            RemoteOutcome::Applied { salesforce_id } => {
                let Some(subject) = subject else {
                    return self.fail_missing_journal(lease_owner, task_id).await;
                };
                let salesforce_id = salesforce_id
                    .as_deref()
                    .or_else(|| call.known_salesforce_id())
                    .map(|id| parse_salesforce_id("salesforce_id", id))
                    .transpose()?;
                let link = project_sync_link(
                    subject.existing_link.as_ref(),
                    &subject.envelope,
                    salesforce_id.as_ref(),
                    call.is_delete(),
                );
                self.store.put_link(&link).await?;
                self.store.ack_task_for_worker(lease_owner, task_id).await?;
                Ok(true)
            }
            RemoteOutcome::Rejected { message } => {
                self.store
                    .fail_task_for_worker(lease_owner, task_id, message)
                    .await?;
                Ok(false)
            }
            RemoteOutcome::Retry { message } => {
                self.store
                    .retry_task_for_worker(
                        lease_owner,
                        task_id,
                        Utc::now() + chrono::Duration::seconds(30),
                        message,
                    )
                    .await?;
                Ok(false)
            }
        }
    }

    async fn fail_missing_journal(
        &self,
        lease_owner: &str,
        task_id: i64,
    ) -> Result<bool, ForceSyncError> {
        self.store
            .fail_task_for_worker(lease_owner, task_id, MISSING_JOURNAL_ROW)
            .await?;
        Ok(false)
    }

    async fn load_apply_subject(
        &self,
        task_id: i64,
    ) -> Result<Option<ApplySubject>, ForceSyncError> {
        let Some(context) = self.load_apply_task_context(task_id).await? else {
            return Ok(None);
        };
        let key = context.envelope.sync_key();
        let existing_link = self
            .store
            .get_link(key.tenant(), key.object_name(), key.external_id())
            .await?;
        Ok(Some(ApplySubject {
            envelope: context.envelope,
            current_payload: context.current_payload,
            existing_link,
        }))
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
        parse_source_system,
    };
    use crate::{
        identity::SyncKey,
        model::{ChangeEnvelope, ChangeOperation, SourceCursor, SourceSystem},
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
        let Err(err) = parse_source_system("oracle") else {
            panic!("expected error for unknown source system");
        };
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
        let Err(err) = parse_change_operation("insert") else {
            panic!("expected error for unknown change operation");
        };
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
        let Err(err) = parse_source_cursor("salesforce-replay-id:not-a-number") else {
            panic!("expected error for invalid replay ID");
        };
        assert!(err.to_string().contains("source_cursor"));
    }

    #[test]
    fn parse_source_cursor_unknown_prefix_returns_error() {
        let Err(err) = parse_source_cursor("kafka-offset:99") else {
            panic!("expected error for unknown cursor prefix");
        };
        assert!(err.to_string().contains("kafka-offset:99"));
    }

    // -- SyncEngineBuilder validation tests --

    mod builder_tests {
        use async_trait::async_trait;
        use force::{
            auth::{AccessToken, Authenticator, TokenResponse},
            client::builder,
            error::Result as ForceResult,
        };

        use crate::{ObjectSync, SyncEngine, error::ForceSyncError};

        #[derive(Debug, Clone)]
        struct StubAuth;

        #[async_trait]
        impl Authenticator for StubAuth {
            async fn authenticate(&self) -> ForceResult<AccessToken> {
                Ok(AccessToken::from_response(TokenResponse {
                    access_token: secrecy::SecretString::new("stub".to_owned().into()),
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

            let Err(err) = result else {
                panic!("expected error for missing postgres");
            };
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
            let store = crate::PgStore::new(pool);

            let result = SyncEngine::builder(client).postgres(store).build();

            let Err(err) = result else {
                panic!("expected error for empty objects");
            };
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
