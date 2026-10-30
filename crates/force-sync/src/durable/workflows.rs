//! The autumn-harvest workflows and activities behind durable sync.
//!
//! Workflows hold no I/O. They only sequence activities and branch on the
//! recorded results, so replay after a crash is deterministic. Activities
//! reach the sync engine through the [`DurableSyncState`] that the harvest
//! builder holds as shared state.

use std::time::Duration;

use autumn_harvest::execution::{StartWorkflowParams, start_or_load_workflow_execution};
use autumn_harvest::prelude::*;
use autumn_harvest::types::{ShardId, WorkflowIdConflictPolicy, WorkflowIdReusePolicy};
use serde::{Deserialize, Serialize};

use super::{DurableSyncState, FORCE_SYNC_QUEUE};
use crate::apply::steps::{ApplyPlan, ApplyResolution, RemoteCall, RemoteOutcome};

/// The workflow name of one apply task.
pub(crate) const APPLY_WORKFLOW: &str = "force_sync_apply";

/// Input of the [`force_sync_apply`] workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ApplyInput {
    /// The `sync_task` row to apply.
    pub task_id: i64,
    /// The number of tasks dispatched together, for lane selection.
    pub batch_size: usize,
    /// The lease owner that acknowledges the task.
    pub lease_owner: String,
    /// The retry policy of the Salesforce call.
    ///
    /// It is part of the input, so it is recorded in history. A config
    /// change then cannot change a run that is already in flight.
    pub remote_retry: RetryPolicy,
}

/// Output of the [`force_sync_apply`] workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ApplyReport {
    /// The task that was settled.
    pub task_id: i64,
    /// Whether the task applied a change.
    pub applied: bool,
}

/// Input of the prepare activity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PrepareInput {
    /// The task to plan.
    pub task_id: i64,
    /// The number of tasks dispatched together.
    pub batch_size: usize,
}

/// Input of the finalize activity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FinalizeInput {
    /// The task to settle.
    pub task_id: i64,
    /// The lease owner that acknowledges the task.
    pub lease_owner: String,
    /// What to settle.
    pub resolution: ApplyResolution,
}

/// Output of the [`force_sync_pump`] workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub(crate) struct PumpReport {
    /// Outbox rows captured into the journal.
    pub captured: usize,
    /// Apply workflows started or attached.
    pub dispatched: usize,
}

/// Applies one sync task durably.
///
/// The three phases run as three activities. Harvest records each result, so
/// a crash after the Salesforce call resumes at finalize and does not call
/// Salesforce again.
///
/// Every path ends in finalize. A task therefore always leaves the `leased`
/// state as `done` or `failed`, and never returns to `ready`. A failed
/// prepare and an exhausted Salesforce retry fail the task with the reason.
#[workflow]
pub(crate) async fn force_sync_apply(
    ctx: &WorkflowContext,
    input: ApplyInput,
) -> Result<ApplyReport, String> {
    let prepared: HarvestResult<ApplyPlan> = ctx
        .execute_activity(
            &force_sync_prepare_info(),
            PrepareInput {
                task_id: input.task_id,
                batch_size: input.batch_size,
            },
        )
        .await;

    let resolution = match prepared {
        Ok(ApplyPlan::Remote(call)) => {
            let mut execute = force_sync_execute_info();
            execute.default_retry_policy = Some(input.remote_retry.clone());
            let outcome = match ctx
                .execute_activity::<_, RemoteOutcome>(&execute, &call)
                .await
            {
                Ok(RemoteOutcome::Retry { message }) => RemoteOutcome::Rejected {
                    message: format!("Salesforce call retries exhausted: {message}"),
                },
                Ok(outcome) => outcome,
                Err(error) => RemoteOutcome::Rejected {
                    message: format!("Salesforce call retries exhausted: {error}"),
                },
            };
            ApplyResolution::Remote { call, outcome }
        }
        Ok(plan) => ApplyResolution::Local { plan },
        Err(error) => ApplyResolution::Local {
            plan: ApplyPlan::Fail {
                reason: format!("prepare failed: {error}"),
            },
        },
    };

    let applied: bool = ctx
        .execute_activity(
            &force_sync_finalize_info(),
            FinalizeInput {
                task_id: input.task_id,
                lease_owner: input.lease_owner,
                resolution,
            },
        )
        .await
        .map_err(|error| error.to_string())?;

    Ok(ApplyReport {
        task_id: input.task_id,
        applied,
    })
}

/// Captures outbox rows, then dispatches ready tasks as apply workflows.
///
/// A schedule fires it at the configured pump interval.
#[workflow]
pub(crate) async fn force_sync_pump(
    ctx: &WorkflowContext,
    input: (),
) -> Result<PumpReport, String> {
    let () = input;
    let captured: usize = ctx
        .execute_activity(&force_sync_capture_info(), ())
        .await
        .map_err(|error| error.to_string())?;
    let dispatched: usize = ctx
        .execute_activity(&force_sync_dispatch_info(), ())
        .await
        .map_err(|error| error.to_string())?;
    Ok(PumpReport {
        captured,
        dispatched,
    })
}

/// Runs one reconciliation pass. A schedule fires it.
#[workflow]
pub(crate) async fn force_sync_reconcile(
    ctx: &WorkflowContext,
    input: (),
) -> Result<usize, String> {
    let () = input;
    ctx.execute_activity(&force_sync_reconcile_pass_info(), ())
        .await
        .map_err(|error| error.to_string())
}

fn sync_state(ctx: &ActivityContext) -> Result<&DurableSyncState, String> {
    ctx.state::<DurableSyncState>()
        .ok_or_else(|| "force-sync durable state is not registered on this worker".to_owned())
}

/// Prepare phase. It reads only, so a retry is always safe.
#[activity(
    start_to_close = "30s",
    retry = RetryPolicy::exponential(5, Duration::from_secs(1)),
    queue = "force-sync"
)]
pub(crate) async fn force_sync_prepare(
    ctx: &ActivityContext,
    input: PrepareInput,
) -> Result<ApplyPlan, String> {
    sync_state(ctx)?
        .ops
        .prepare_apply(input.task_id, input.batch_size)
        .await
        .map_err(|error| error.to_string())
}

/// Execute phase: the one Salesforce call.
///
/// A transient failure fails the attempt, so harvest retries it with the
/// policy that the workflow input carries. On the last attempt the failure is
/// returned as a rejection, so the workflow records why the task failed.
/// Upserts use external IDs and deletes accept 404, so a repeated call after
/// a lost result writes the same state.
#[activity(
    start_to_close = "300s",
    retry = RetryPolicy::exponential(8, Duration::from_secs(2)),
    queue = "force-sync"
)]
pub(crate) async fn force_sync_execute(
    ctx: &ActivityContext,
    call: RemoteCall,
) -> Result<RemoteOutcome, String> {
    let outcome = sync_state(ctx)?.ops.execute_remote(&call).await;
    settle_attempt(outcome, ctx.is_last_attempt())
}

/// Maps a transient failure to an attempt failure, except on the last attempt.
pub(crate) fn settle_attempt(
    outcome: RemoteOutcome,
    last_attempt: bool,
) -> Result<RemoteOutcome, String> {
    match outcome {
        RemoteOutcome::Retry { message } if last_attempt => Ok(RemoteOutcome::Rejected {
            message: format!("Salesforce call retries exhausted: {message}"),
        }),
        RemoteOutcome::Retry { message } => Err(message),
        outcome => Ok(outcome),
    }
}

/// Finalize phase. Link writes are upserts and the task update checks the
/// lease owner, so a retry is safe.
#[activity(
    start_to_close = "30s",
    retry = RetryPolicy::exponential(10, Duration::from_secs(1)),
    queue = "force-sync"
)]
pub(crate) async fn force_sync_finalize(
    ctx: &ActivityContext,
    input: FinalizeInput,
) -> Result<bool, String> {
    sync_state(ctx)?
        .ops
        .finalize_apply(&input.lease_owner, input.task_id, &input.resolution)
        .await
        .map_err(|error| error.to_string())
}

/// Captures up to the configured number of outbox batches.
///
/// Each batch commits the journal rows, the apply tasks and the outbox marks
/// in one transaction, so a retry never captures a row twice.
#[activity(
    start_to_close = "120s",
    retry = RetryPolicy::exponential(5, Duration::from_secs(1)),
    queue = "force-sync"
)]
pub(crate) async fn force_sync_capture(ctx: &ActivityContext, input: ()) -> Result<usize, String> {
    let () = input;
    let state = sync_state(ctx)?;
    let batch_size = usize::try_from(state.ops.capture_batch_size()).unwrap_or(0);
    let mut captured = 0;
    for _ in 0..state.config.capture_batches_per_pump {
        let count = state
            .ops
            .capture_postgres_once()
            .await
            .map_err(|error| error.to_string())?;
        captured += count;
        if count < batch_size || count == 0 {
            break;
        }
    }
    Ok(captured)
}

/// Leases ready tasks and starts one apply workflow per task.
///
/// The workflow ID is `force-sync-apply-{task_id}`. A task that is dispatched
/// again attaches to its running workflow, and replaces a failed one. Each
/// workflow carries its sync key as a concurrency key with limit 1, so two
/// changes to one record never apply at the same time.
#[activity(
    start_to_close = "120s",
    retry = RetryPolicy::exponential(5, Duration::from_secs(1)),
    queue = "force-sync"
)]
pub(crate) async fn force_sync_dispatch(ctx: &ActivityContext, input: ()) -> Result<usize, String> {
    let () = input;
    let state = sync_state(ctx)?;
    let config = &state.config;
    let leased = state
        .ops
        .lease_apply_tasks(
            &config.lease_owner,
            config.dispatch_batch_size,
            config.dispatch_lease,
        )
        .await
        .map_err(|error| error.to_string())?;
    if leased.is_empty() {
        return Ok(0);
    }

    let batch_size = leased.len();
    let mut started = 0;
    for (index, (task_id, key)) in leased.iter().enumerate() {
        let input = ApplyInput {
            task_id: *task_id,
            batch_size,
            lease_owner: config.lease_owner.clone(),
            remote_retry: config.remote_retry.clone(),
        };
        let started_run = start_apply(state, &input, key.as_deref()).await;
        if started_run == Ok(StartResult::AlreadyCompleted) {
            // The run finished, so its finalize already settled this task
            // once. A new run cannot start under the reuse policy, so fail
            // the task for an operator instead of leasing it forever.
            state
                .ops
                .fail_task(
                    &config.lease_owner,
                    *task_id,
                    &format!(
                        "apply workflow {} already completed; inspect its history",
                        apply_workflow_id(*task_id)
                    ),
                )
                .await
                .map_err(|error| error.to_string())?;
            continue;
        }
        if let Err(error) = started_run {
            // Return the tasks that did not start, so the retry of this
            // activity leases them again without waiting for the lease.
            let unstarted: Vec<i64> = leased[index..].iter().map(|(id, _)| *id).collect();
            state
                .ops
                .release_tasks(&config.lease_owner, &unstarted, &error)
                .await
                .map_err(|release| format!("{error}; release also failed: {release}"))?;
            return Err(error);
        }
        started += 1;
    }
    Ok(started)
}

/// The workflow ID of the apply workflow for `task_id`.
pub(crate) fn apply_workflow_id(task_id: i64) -> String {
    format!("force-sync-apply-{task_id}")
}

/// What a dispatch start found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartResult {
    /// A run started, or the task attached to its running workflow.
    Running,
    /// The task attached to a workflow that already completed.
    AlreadyCompleted,
}

async fn start_apply(
    state: &DurableSyncState,
    input: &ApplyInput,
    concurrency_key: Option<&str>,
) -> Result<StartResult, String> {
    let workflow_id = apply_workflow_id(input.task_id);
    let payload = serde_json::to_value(input).map_err(|error| error.to_string())?;
    let mut conn = state
        .harvest_pool
        .get()
        .await
        .map_err(|error| format!("harvest pool: {error}"))?;
    let params = StartWorkflowParams {
        reuse_policy: WorkflowIdReusePolicy::AllowDuplicateFailedOnly,
        conflict_policy: WorkflowIdConflictPolicy::UseExisting,
        concurrency_key: concurrency_key.map(str::to_owned),
        concurrency_limit: concurrency_key.map(|_| 1),
        ..StartWorkflowParams::new(
            APPLY_WORKFLOW,
            &workflow_id,
            ExecutionId::new_for_shard(ShardId::new(0)),
            payload,
            FORCE_SYNC_QUEUE,
        )
    };
    let started = start_or_load_workflow_execution(&mut conn, params, None)
        .await
        .map_err(|error| format!("start {workflow_id}: {error}"))?;
    Ok(
        if !started.created && started.state.eq_ignore_ascii_case("completed") {
            StartResult::AlreadyCompleted
        } else {
            StartResult::Running
        },
    )
}

/// Runs one reconciliation pass and enqueues repair tasks.
#[activity(
    start_to_close = "300s",
    retry = RetryPolicy::exponential(5, Duration::from_secs(1)),
    queue = "force-sync"
)]
pub(crate) async fn force_sync_reconcile_pass(
    ctx: &ActivityContext,
    input: (),
) -> Result<usize, String> {
    let () = input;
    sync_state(ctx)?
        .ops
        .reconcile_once()
        .await
        .map_err(|error| error.to_string())
}

/// Every workflow of durable sync.
pub(crate) fn all_workflows() -> Vec<WorkflowInfo> {
    workflows![force_sync_apply, force_sync_pump, force_sync_reconcile]
}

/// Every activity of durable sync.
pub(crate) fn all_activities() -> Vec<ActivityInfo> {
    activities![
        force_sync_prepare,
        force_sync_execute,
        force_sync_finalize,
        force_sync_capture,
        force_sync_dispatch,
        force_sync_reconcile_pass,
    ]
}
