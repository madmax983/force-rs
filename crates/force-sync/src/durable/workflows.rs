//! The autumn-harvest workflows and activities behind durable sync.
//!
//! Workflows hold no I/O. They only sequence activities and branch on the
//! recorded results, so replay after a crash is deterministic. Activities
//! reach the sync engine through the [`DurableSyncState`] that the harvest
//! builder holds as shared state.

use std::time::Duration;

use autumn_harvest::execution::{StartWorkflowParams, start_or_load_workflow_execution};
use autumn_harvest::prelude::*;
use autumn_harvest::types::{WorkflowIdConflictPolicy, WorkflowIdReusePolicy};
use serde::{Deserialize, Serialize};

use super::{DurableSyncState, FORCE_SYNC_QUEUE};
use crate::apply::steps::{ApplyPlan, ApplyResolution, RemoteCall, RemoteOutcome};
use crate::capture::salesforce::CdcWindowReport;

/// The workflow name of one CDC stream.
pub(crate) const CDC_STREAM_WORKFLOW: &str = "force_sync_cdc_stream";

/// How often a CDC window activity heartbeats.
const CDC_HEARTBEAT_EVERY: Duration = Duration::from_secs(10);

/// How often a CDC window activity checks for cancellation.
const CDC_CANCEL_POLL: Duration = Duration::from_secs(1);

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

/// What places the apply workflow of one task on a shard.
///
/// Harvest enforces a concurrency key within one shard only. So every task
/// for one record must land on one shard, or two of them could apply at the
/// same time on two shards. The record key places the workflow. The task's
/// journal row never changes, so a task dispatched again gets the same key,
/// reaches the same shard, and attaches to its run there. A task with no
/// record key places by its workflow ID.
pub(crate) fn apply_placement_key<'a>(
    workflow_id: &'a str,
    concurrency_key: Option<&'a str>,
) -> &'a str {
    concurrency_key.unwrap_or(workflow_id)
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
    let placement = apply_placement_key(&workflow_id, concurrency_key);
    let (shard, exec_id) = state.harvest.place(APPLY_WORKFLOW, placement);
    let mut conn = state
        .harvest
        .pool
        .pool_for(shard)
        .get()
        .await
        .map_err(|error| format!("harvest pool for shard {shard}: {error}"))?;
    let params = StartWorkflowParams {
        reuse_policy: WorkflowIdReusePolicy::AllowDuplicateFailedOnly,
        conflict_policy: WorkflowIdConflictPolicy::UseExisting,
        concurrency_key: concurrency_key.map(str::to_owned),
        concurrency_limit: concurrency_key.map(|_| 1),
        ..StartWorkflowParams::new(
            APPLY_WORKFLOW,
            &workflow_id,
            exec_id,
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

/// Input of the [`force_sync_cdc_stream`] workflow.
///
/// The supervisor builds it from the config, and each run reads it again
/// before it continues as new. It is recorded in history, so a config
/// change reaches a stream at its next run and never changes a run in
/// flight.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CdcStreamInput {
    /// The [`crate::CdcStream`] name.
    pub stream: String,
    /// Windows before the run continues as new.
    pub windows_per_run: u32,
    /// Seconds to wait after a failed window.
    pub error_backoff_secs: u64,
}

/// Output of the CDC window activity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub(crate) enum CdcWindowResult {
    /// The window ran.
    Window(CdcWindowReport),
    /// The stream is no longer configured. Its workflow ends.
    Unconfigured,
}

/// The stream workflow input that the current config gives `stream`.
fn cdc_stream_input(config: &super::DurableSyncConfig, stream: &str) -> CdcStreamInput {
    CdcStreamInput {
        stream: stream.to_owned(),
        windows_per_run: config.cdc_windows_per_run,
        error_backoff_secs: config.cdc_error_backoff.as_secs(),
    }
}

/// The workflow ID of the stream workflow for `stream`.
pub(crate) fn cdc_workflow_id(stream: &str) -> String {
    format!("force-sync-cdc-{stream}")
}

/// Follows one CDC stream, one bounded window at a time.
///
/// Each window resumes from the stream checkpoint, so a crash costs at most
/// the events that were read but not committed, and those are delivered
/// again. A failed window waits `error_backoff_secs` on a durable timer. After
/// `windows_per_run` windows the run continues as new, which bounds its
/// history. The next run gets the settings of the current config, read
/// through a recorded activity. The run ends only when the stream is no
/// longer configured.
#[workflow]
pub(crate) async fn force_sync_cdc_stream(
    ctx: &WorkflowContext,
    input: CdcStreamInput,
) -> Result<(), String> {
    for window in 0..input.windows_per_run {
        let result: HarvestResult<CdcWindowResult> = ctx
            .execute_activity(&force_sync_cdc_window_info(), &input.stream)
            .await;
        match result {
            Ok(CdcWindowResult::Window(_)) => {}
            Ok(CdcWindowResult::Unconfigured) => return Ok(()),
            Err(_) => {
                ctx.timer(&format!("cdc-backoff-{window}"), input.error_backoff_secs)
                    .await
                    .map_err(|error| error.to_string())?;
            }
        }
    }
    let current: Option<CdcStreamInput> = ctx
        .execute_activity(&force_sync_cdc_settings_info(), &input.stream)
        .await
        .map_err(|error| error.to_string())?;
    let Some(next) = current else {
        return Ok(());
    };
    let next = serde_json::to_value(&next).map_err(|error| error.to_string())?;
    ctx.continue_as_new(next)
        .await
        .map_err(|error| error.to_string())
}

/// Starts each configured CDC stream workflow that is not running.
///
/// A schedule fires it at the CDC supervise interval.
#[workflow]
pub(crate) async fn force_sync_cdc_supervisor(
    ctx: &WorkflowContext,
    input: (),
) -> Result<usize, String> {
    let () = input;
    ctx.execute_activity(&force_sync_cdc_ensure_info(), ())
        .await
        .map_err(|error| error.to_string())
}

/// Captures one CDC window, and heartbeats while it waits for events.
///
/// The heartbeat lets harvest detect a dead worker within the heartbeat
/// timeout, so another worker takes the stream over quickly.
#[activity(
    start_to_close = "600s",
    heartbeat_timeout = "30s",
    retry = RetryPolicy::exponential(3, Duration::from_secs(1)),
    queue = "force-sync"
)]
pub(crate) async fn force_sync_cdc_window(
    ctx: &ActivityContext,
    stream: String,
) -> Result<CdcWindowResult, String> {
    let state = sync_state(ctx)?;
    let Some(setup) = state.cdc.as_deref() else {
        return Ok(CdcWindowResult::Unconfigured);
    };
    let Some(stream) = setup.stream(&stream) else {
        return Ok(CdcWindowResult::Unconfigured);
    };

    let capture = state
        .ops
        .cdc_window(setup.source.as_ref(), stream, state.config.cdc_window);
    tokio::pin!(capture);
    let mut tick = tokio::time::interval(CDC_CANCEL_POLL);
    tick.tick().await;
    let mut since_heartbeat = Duration::ZERO;
    loop {
        tokio::select! {
            report = &mut capture => {
                return report
                    .map(CdcWindowResult::Window)
                    .map_err(|error| error.to_string());
            }
            _ = tick.tick() => {
                // A cancelled activity, such as one on a draining worker,
                // stops here. The events committed so far stay committed.
                if ctx.is_cancelled() {
                    return Err("CDC window cancelled".to_owned());
                }
                since_heartbeat += CDC_CANCEL_POLL;
                if since_heartbeat >= CDC_HEARTBEAT_EVERY {
                    since_heartbeat = Duration::ZERO;
                    ctx.heartbeat(stream.name())
                        .await
                        .map_err(|error| error.to_string())?;
                }
            }
        }
    }
}

/// Reads the current settings of `stream`, or `None` when it is no longer
/// configured.
#[allow(clippy::unused_async, reason = "#[activity] requires an async fn")]
#[activity(
    start_to_close = "30s",
    retry = RetryPolicy::exponential(5, Duration::from_secs(1)),
    queue = "force-sync"
)]
pub(crate) async fn force_sync_cdc_settings(
    ctx: &ActivityContext,
    stream: String,
) -> Result<Option<CdcStreamInput>, String> {
    let state = sync_state(ctx)?;
    Ok(state
        .cdc
        .as_deref()
        .and_then(|setup| setup.stream(&stream))
        .map(|stream| cdc_stream_input(&state.config, stream.name())))
}

/// Starts or attaches to the workflow of each configured CDC stream.
///
/// A running workflow is kept. A workflow that ended, for any reason,
/// is started again, so the supervisor heals a failed stream.
#[activity(
    start_to_close = "60s",
    retry = RetryPolicy::exponential(5, Duration::from_secs(1)),
    queue = "force-sync"
)]
pub(crate) async fn force_sync_cdc_ensure(
    ctx: &ActivityContext,
    input: (),
) -> Result<usize, String> {
    let () = input;
    let state = sync_state(ctx)?;
    let Some(setup) = state.cdc.as_deref() else {
        return Ok(0);
    };
    let config = &state.config;
    let mut started = 0;
    for stream in &setup.streams {
        let input = cdc_stream_input(config, stream.name());
        let workflow_id = cdc_workflow_id(stream.name());
        let (shard, exec_id) = state.harvest.place(CDC_STREAM_WORKFLOW, &workflow_id);
        let mut conn = state
            .harvest
            .pool
            .pool_for(shard)
            .get()
            .await
            .map_err(|error| format!("harvest pool for shard {shard}: {error}"))?;
        let payload = serde_json::to_value(&input).map_err(|error| error.to_string())?;
        let params = StartWorkflowParams {
            reuse_policy: WorkflowIdReusePolicy::TerminateIfRunning,
            conflict_policy: WorkflowIdConflictPolicy::UseExisting,
            ..StartWorkflowParams::new(
                CDC_STREAM_WORKFLOW,
                &workflow_id,
                exec_id,
                payload,
                FORCE_SYNC_QUEUE,
            )
        };
        let result = start_or_load_workflow_execution(&mut conn, params, None)
            .await
            .map_err(|error| format!("start {workflow_id}: {error}"))?;
        if result.created {
            started += 1;
        }
    }
    Ok(started)
}

/// Every workflow of durable sync.
pub(crate) fn all_workflows() -> Vec<WorkflowInfo> {
    workflows![
        force_sync_apply,
        force_sync_pump,
        force_sync_reconcile,
        force_sync_cdc_stream,
        force_sync_cdc_supervisor,
    ]
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
        force_sync_cdc_window,
        force_sync_cdc_settings,
        force_sync_cdc_ensure,
    ]
}
