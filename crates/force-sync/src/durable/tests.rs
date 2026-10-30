//! Unit tests for durable sync. They need no database.

use std::time::Duration;

use autumn_harvest::event::WorkflowEvent;
use autumn_harvest::policy::{RetryPolicy, Schedule};
use autumn_harvest::testing::{ReplayStatus, TestRunOutcome, WorkflowTestEnv};
use serde_json::{Value, json};

use super::workflows::{
    self, ApplyInput, ApplyReport, CdcStreamInput, CdcWindowResult, PumpReport, all_activities,
    all_workflows, apply_workflow_id, cdc_workflow_id, settle_attempt,
};
use super::{
    APPLY_WORKFLOW, CDC_STREAM_WORKFLOW, CDC_SUPERVISOR_WORKFLOW, DurableSyncConfig,
    FORCE_SYNC_QUEUE, MAX_CDC_WINDOW, PUMP_WORKFLOW, RECONCILE_WORKFLOW, concurrency_key,
};
use crate::apply::steps::{ApplyPlan, ApplyResolution, RemoteCall, RemoteOutcome};
use crate::capture::salesforce::{CdcWindow, CdcWindowEnd, CdcWindowReport};

const SF_ID: &str = "001000000000001AAA";

fn rest_call() -> RemoteCall {
    RemoteCall::RestUpsert {
        object_name: "Account".to_owned(),
        external_id_field: "ExternalId__c".to_owned(),
        external_id: "ext-1".to_owned(),
        payload: json!({"Name": "Acme"}),
    }
}

fn apply_input() -> Value {
    serde_json::to_value(ApplyInput {
        task_id: 42,
        batch_size: 3,
        lease_owner: "owner-1".to_owned(),
        remote_retry: RetryPolicy::fixed(4, Duration::from_secs(1)),
    })
    .unwrap_or_else(|error| panic!("serialize input: {error}"))
}

fn to_json<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or_else(|error| panic!("serialize: {error}"))
}

/// The inputs of every scheduled activity, in order, keyed by name.
fn scheduled(outcome: &TestRunOutcome) -> Vec<(String, Value)> {
    outcome
        .events()
        .iter()
        .filter_map(|event| match event {
            WorkflowEvent::ActivityScheduled { name, input, .. } => {
                Some((name.clone(), input.clone()))
            }
            _ => None,
        })
        .collect()
}

fn finalize_resolution(outcome: &TestRunOutcome) -> ApplyResolution {
    let (_, input) = scheduled(outcome)
        .into_iter()
        .find(|(name, _)| name == "force_sync_finalize")
        .unwrap_or_else(|| panic!("finalize was not scheduled: {:?}", scheduled(outcome)));
    assert_eq!(input["task_id"], json!(42));
    assert_eq!(input["lease_owner"], json!("owner-1"));
    serde_json::from_value(input["resolution"].clone())
        .unwrap_or_else(|error| panic!("finalize resolution: {error}"))
}

async fn run_apply(env: WorkflowTestEnv) -> TestRunOutcome {
    let handler = workflows::force_sync_apply_info().handler;
    let outcome = env.run(handler, apply_input()).await;
    let replay = outcome.replay_check(handler).await;
    assert!(
        matches!(replay.status, ReplayStatus::ReplaySucceeded),
        "apply workflow must replay deterministically: {replay}"
    );
    outcome
}

fn report(outcome: &TestRunOutcome) -> ApplyReport {
    let output = outcome
        .result
        .clone()
        .unwrap_or_else(|error| panic!("workflow failed: {error}"));
    serde_json::from_value(output).unwrap_or_else(|error| panic!("report: {error}"))
}

// -- config --------------------------------------------------------------

#[test]
fn default_config_is_valid() {
    let config = DurableSyncConfig::default();
    assert!(config.validate().is_ok());
    assert_eq!(config.lease_owner, super::DEFAULT_LEASE_OWNER);
    assert_eq!(config.reconcile_interval, Some(Duration::from_secs(300)));
}

#[test]
fn config_validation_rejects_each_invalid_field() {
    let cases: Vec<(DurableSyncConfig, &str)> = vec![
        (
            DurableSyncConfig {
                pump_interval: Duration::ZERO,
                ..DurableSyncConfig::default()
            },
            "pump_interval",
        ),
        (
            DurableSyncConfig {
                pump_interval: Duration::from_millis(1500),
                ..DurableSyncConfig::default()
            },
            "pump_interval",
        ),
        (
            DurableSyncConfig {
                reconcile_interval: Some(Duration::from_millis(10)),
                ..DurableSyncConfig::default()
            },
            "reconcile_interval",
        ),
        (
            DurableSyncConfig {
                capture_batches_per_pump: 0,
                ..DurableSyncConfig::default()
            },
            "capture_batches_per_pump",
        ),
        (
            DurableSyncConfig {
                dispatch_batch_size: 0,
                ..DurableSyncConfig::default()
            },
            "dispatch_batch_size",
        ),
        (
            DurableSyncConfig {
                dispatch_lease: Duration::ZERO,
                ..DurableSyncConfig::default()
            },
            "dispatch_lease",
        ),
        (
            DurableSyncConfig {
                remote_retry: RetryPolicy::fixed(0, Duration::from_secs(1)),
                ..DurableSyncConfig::default()
            },
            "remote_retry",
        ),
        (
            DurableSyncConfig {
                lease_owner: String::new(),
                ..DurableSyncConfig::default()
            },
            "lease_owner",
        ),
    ];

    for (config, field) in cases {
        let Err(error) = config.validate() else {
            panic!("expected {field} to be rejected");
        };
        assert!(
            error.to_string().contains(field),
            "error for {field} should name it: {error}"
        );
    }
}

#[test]
fn reconcile_may_be_disabled() {
    let config = DurableSyncConfig {
        reconcile_interval: None,
        ..DurableSyncConfig::default()
    };
    assert!(config.validate().is_ok());
}

// -- registration --------------------------------------------------------

#[test]
fn workflow_names_match_the_public_constants() {
    let names: Vec<&str> = all_workflows().iter().map(|info| info.name).collect();
    assert_eq!(
        names,
        vec![
            APPLY_WORKFLOW,
            PUMP_WORKFLOW,
            RECONCILE_WORKFLOW,
            CDC_STREAM_WORKFLOW,
            CDC_SUPERVISOR_WORKFLOW
        ]
    );
}

#[test]
fn every_activity_runs_on_the_force_sync_queue() {
    let activities = all_activities();
    assert_eq!(activities.len(), 9);
    for info in activities {
        assert_eq!(
            info.default_queue,
            Some(FORCE_SYNC_QUEUE),
            "{} must run on {FORCE_SYNC_QUEUE}",
            info.name
        );
        assert!(
            info.default_start_to_close.is_some(),
            "{} needs a timeout so a crashed attempt is retried",
            info.name
        );
        assert!(
            info.default_retry_policy.is_some(),
            "{} needs a retry policy",
            info.name
        );
    }
}

#[test]
fn schedule_shape_follows_the_config() {
    let schedules = super::DurableSync::schedules_for(
        &DurableSyncConfig {
            pump_interval: Duration::from_secs(7),
            reconcile_interval: Some(Duration::from_secs(60)),
            ..DurableSyncConfig::default()
        },
        false,
    );
    assert_eq!(schedules.len(), 2);
    assert_eq!(schedules[0].workflow_name, PUMP_WORKFLOW);
    assert!(matches!(
        schedules[0].schedule,
        Schedule::Interval(interval) if interval == Duration::from_secs(7)
    ));
    assert_eq!(schedules[1].workflow_name, RECONCILE_WORKFLOW);
    assert!(matches!(
        schedules[1].schedule,
        Schedule::Interval(interval) if interval == Duration::from_secs(60)
    ));
    for schedule in &schedules {
        assert_eq!(schedule.queue_name, FORCE_SYNC_QUEUE);
        assert_eq!(schedule.max_active_runs, 1, "runs must not overlap");
    }

    let pump_only = super::DurableSync::schedules_for(
        &DurableSyncConfig {
            reconcile_interval: None,
            ..DurableSyncConfig::default()
        },
        false,
    );
    assert_eq!(pump_only.len(), 1);
    assert_eq!(pump_only[0].workflow_name, PUMP_WORKFLOW);
}

#[test]
fn apply_workflow_id_is_stable_per_task() {
    assert_eq!(apply_workflow_id(7), "force-sync-apply-7");
    assert_eq!(apply_workflow_id(7), apply_workflow_id(7));
    assert_ne!(apply_workflow_id(7), apply_workflow_id(8));
}

#[test]
fn concurrency_key_is_unambiguous_across_separators() {
    let left = concurrency_key(&("a/b".to_owned(), "c".to_owned(), "d".to_owned()));
    let right = concurrency_key(&("a".to_owned(), "b/c".to_owned(), "d".to_owned()));
    assert_ne!(left, right);
    assert_eq!(left, r#"["a/b","c","d"]"#);
}

// -- execute attempt settlement -----------------------------------------

#[test]
fn transient_failure_retries_until_the_last_attempt() {
    let retry = RemoteOutcome::Retry {
        message: "503".to_owned(),
    };
    assert_eq!(settle_attempt(retry.clone(), false), Err("503".to_owned()));
    assert_eq!(
        settle_attempt(retry, true),
        Ok(RemoteOutcome::Rejected {
            message: "Salesforce call retries exhausted: 503".to_owned()
        })
    );
}

#[test]
fn applied_and_rejected_outcomes_pass_through_on_any_attempt() {
    let applied = RemoteOutcome::Applied {
        salesforce_id: Some(SF_ID.to_owned()),
    };
    let rejected = RemoteOutcome::Rejected {
        message: "400".to_owned(),
    };
    for last in [false, true] {
        assert_eq!(settle_attempt(applied.clone(), last), Ok(applied.clone()));
        assert_eq!(settle_attempt(rejected.clone(), last), Ok(rejected.clone()));
    }
}

// -- apply workflow ------------------------------------------------------

#[tokio::test]
async fn remote_plan_runs_prepare_execute_finalize_in_order() {
    let outcome = run_apply(
        WorkflowTestEnv::new()
            .mock_activity("force_sync_prepare", |_| {
                Ok(to_json(&ApplyPlan::Remote(rest_call())))
            })
            .mock_activity("force_sync_execute", |_| {
                Ok(to_json(&RemoteOutcome::Applied {
                    salesforce_id: Some(SF_ID.to_owned()),
                }))
            })
            .mock_activity("force_sync_finalize", |_| Ok(json!(true))),
    )
    .await;

    let names: Vec<String> = scheduled(&outcome).into_iter().map(|(n, _)| n).collect();
    assert_eq!(
        names,
        vec![
            "force_sync_prepare",
            "force_sync_execute",
            "force_sync_finalize"
        ]
    );
    let prepare_input = &scheduled(&outcome)[0].1;
    assert_eq!(prepare_input, &json!({"task_id": 42, "batch_size": 3}));
    assert_eq!(
        finalize_resolution(&outcome),
        ApplyResolution::Remote {
            call: rest_call(),
            outcome: RemoteOutcome::Applied {
                salesforce_id: Some(SF_ID.to_owned())
            },
        }
    );
    assert_eq!(
        report(&outcome),
        ApplyReport {
            task_id: 42,
            applied: true
        }
    );
}

#[tokio::test]
async fn apply_activities_are_scheduled_on_the_force_sync_queue() {
    let outcome = run_apply(
        WorkflowTestEnv::new()
            .mock_activity("force_sync_prepare", |_| {
                Ok(to_json(&ApplyPlan::Remote(rest_call())))
            })
            .mock_activity("force_sync_execute", |_| {
                Ok(to_json(&RemoteOutcome::Applied {
                    salesforce_id: None,
                }))
            })
            .mock_activity("force_sync_finalize", |_| Ok(json!(true))),
    )
    .await;
    let queues: Vec<&str> = outcome
        .events()
        .iter()
        .filter_map(|event| match event {
            WorkflowEvent::ActivityScheduled { queue, .. } => Some(queue.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(queues, vec![FORCE_SYNC_QUEUE; 3]);
}

#[tokio::test]
async fn local_plan_skips_the_salesforce_call() {
    for plan in [
        ApplyPlan::Noop,
        ApplyPlan::ProjectLocal,
        ApplyPlan::Conflict {
            fields: vec!["Name".to_owned()],
        },
        ApplyPlan::Fail {
            reason: "missing apply task journal row".to_owned(),
        },
    ] {
        let returned = to_json(&plan);
        let outcome = run_apply(
            WorkflowTestEnv::new()
                .mock_activity("force_sync_prepare", move |_| Ok(returned.clone()))
                .mock_activity("force_sync_finalize", |_| Ok(json!(false))),
        )
        .await;
        let names: Vec<String> = scheduled(&outcome).into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, vec!["force_sync_prepare", "force_sync_finalize"]);
        assert_eq!(
            finalize_resolution(&outcome),
            ApplyResolution::Local { plan }
        );
    }
}

#[tokio::test]
async fn exhausted_salesforce_retries_fail_the_task_with_the_reason() {
    let outcome = run_apply(
        WorkflowTestEnv::new()
            .mock_activity("force_sync_prepare", |_| {
                Ok(to_json(&ApplyPlan::Remote(rest_call())))
            })
            .mock_activity("force_sync_execute", |_| Err("HTTP 503".to_owned()))
            .mock_activity("force_sync_finalize", |_| Ok(json!(false))),
    )
    .await;
    let ApplyResolution::Remote {
        outcome: RemoteOutcome::Rejected { message },
        ..
    } = finalize_resolution(&outcome)
    else {
        panic!("exhausted retries must finalize as rejected");
    };
    assert!(message.contains("retries exhausted"), "{message}");
    assert!(message.contains("HTTP 503"), "{message}");
    assert!(!report(&outcome).applied);
}

#[tokio::test]
async fn a_leaked_retry_outcome_never_reaches_finalize() {
    let outcome = run_apply(
        WorkflowTestEnv::new()
            .mock_activity("force_sync_prepare", |_| {
                Ok(to_json(&ApplyPlan::Remote(rest_call())))
            })
            .mock_activity("force_sync_execute", |_| {
                Ok(to_json(&RemoteOutcome::Retry {
                    message: "timeout".to_owned(),
                }))
            })
            .mock_activity("force_sync_finalize", |_| Ok(json!(false))),
    )
    .await;
    assert!(matches!(
        finalize_resolution(&outcome),
        ApplyResolution::Remote {
            outcome: RemoteOutcome::Rejected { .. },
            ..
        }
    ));
}

#[tokio::test]
async fn failed_prepare_still_settles_the_task() {
    let outcome = run_apply(
        WorkflowTestEnv::new()
            .mock_activity("force_sync_prepare", |_| {
                Err("missing required configuration: object sync".to_owned())
            })
            .mock_activity("force_sync_finalize", |_| Ok(json!(false))),
    )
    .await;
    let ApplyResolution::Local {
        plan: ApplyPlan::Fail { reason },
    } = finalize_resolution(&outcome)
    else {
        panic!("a failed prepare must finalize as a failure");
    };
    assert!(reason.starts_with("prepare failed:"), "{reason}");
    assert!(reason.contains("object sync"), "{reason}");
}

#[tokio::test]
async fn failed_finalize_fails_the_workflow_for_redispatch() {
    let outcome = run_apply(
        WorkflowTestEnv::new()
            .mock_activity("force_sync_prepare", |_| Ok(to_json(&ApplyPlan::Noop)))
            .mock_activity("force_sync_finalize", |_| {
                Err("database pool error".to_owned())
            }),
    )
    .await;
    let Err(error) = &outcome.result else {
        panic!("a failed finalize must fail the run");
    };
    assert!(error.contains("database pool error"), "{error}");
}

// -- pump and reconcile workflows ---------------------------------------

#[tokio::test]
async fn pump_captures_then_dispatches() {
    let handler = workflows::force_sync_pump_info().handler;
    let outcome = WorkflowTestEnv::new()
        .mock_activity("force_sync_capture", |_| Ok(json!(12)))
        .mock_activity("force_sync_dispatch", |_| Ok(json!(9)))
        .run(handler, Value::Null)
        .await;
    let names: Vec<String> = scheduled(&outcome).into_iter().map(|(n, _)| n).collect();
    assert_eq!(names, vec!["force_sync_capture", "force_sync_dispatch"]);
    let output = outcome
        .result
        .clone()
        .unwrap_or_else(|error| panic!("pump failed: {error}"));
    assert_eq!(
        serde_json::from_value::<PumpReport>(output)
            .unwrap_or_else(|error| panic!("pump report: {error}")),
        PumpReport {
            captured: 12,
            dispatched: 9
        }
    );
    let replay = outcome.replay_check(handler).await;
    assert!(
        matches!(replay.status, ReplayStatus::ReplaySucceeded),
        "{replay}"
    );
}

#[tokio::test]
async fn pump_does_not_dispatch_when_capture_fails() {
    let outcome = WorkflowTestEnv::new()
        .mock_activity("force_sync_capture", |_| Err("outbox locked".to_owned()))
        .mock_activity("force_sync_dispatch", |_| Ok(json!(1)))
        .run(workflows::force_sync_pump_info().handler, Value::Null)
        .await;
    assert!(outcome.result.is_err());
    let names: Vec<String> = scheduled(&outcome).into_iter().map(|(n, _)| n).collect();
    assert_eq!(names, vec!["force_sync_capture"]);
}

#[tokio::test]
async fn reconcile_returns_the_repair_count() {
    let handler = workflows::force_sync_reconcile_info().handler;
    let outcome = WorkflowTestEnv::new()
        .mock_activity("force_sync_reconcile_pass", |_| Ok(json!(3)))
        .run(handler, Value::Null)
        .await;
    assert_eq!(outcome.result, Ok(json!(3)));
    let replay = outcome.replay_check(handler).await;
    assert!(
        matches!(replay.status, ReplayStatus::ReplaySucceeded),
        "{replay}"
    );
}

// -- CDC stream and supervisor workflows --------------------------------

fn cdc_input(windows_per_run: u32) -> Value {
    serde_json::to_value(CdcStreamInput {
        stream: "salesforce:Account".to_owned(),
        windows_per_run,
        error_backoff_secs: 7,
    })
    .unwrap_or_else(|error| panic!("serialize cdc input: {error}"))
}

fn window_report(captured: usize) -> Value {
    to_json(&CdcWindowResult::Window(CdcWindowReport {
        captured,
        duplicates: 0,
        quarantined: 0,
        end: CdcWindowEnd::Deadline,
    }))
}

async fn run_cdc_stream(env: WorkflowTestEnv, input: Value) -> TestRunOutcome {
    let handler = workflows::force_sync_cdc_stream_info().handler;
    let outcome = env.run(handler, input).await;
    let replay = outcome.replay_check(handler).await;
    assert!(
        matches!(replay.status, ReplayStatus::ReplaySucceeded),
        "cdc stream workflow must replay deterministically: {replay}"
    );
    outcome
}

fn continued_as_new_input(outcome: &TestRunOutcome) -> Option<Value> {
    outcome.events().iter().find_map(|event| match event {
        WorkflowEvent::WorkflowContinuedAsNew { input, .. } => Some(input.clone()),
        _ => None,
    })
}

fn timers(outcome: &TestRunOutcome) -> Vec<u64> {
    outcome
        .events()
        .iter()
        .filter_map(|event| match event {
            WorkflowEvent::TimerStarted { duration_secs, .. } => Some(*duration_secs),
            _ => None,
        })
        .collect()
}

#[test]
fn cdc_config_validation_rejects_each_invalid_field() {
    let cases: Vec<(DurableSyncConfig, &str)> = vec![
        (
            DurableSyncConfig {
                cdc_window: CdcWindow {
                    max_duration: Duration::ZERO,
                    max_events: 10,
                },
                ..DurableSyncConfig::default()
            },
            "cdc_window.max_duration",
        ),
        (
            DurableSyncConfig {
                cdc_window: CdcWindow {
                    max_duration: MAX_CDC_WINDOW + Duration::from_secs(1),
                    max_events: 10,
                },
                ..DurableSyncConfig::default()
            },
            "cdc_window.max_duration",
        ),
        (
            DurableSyncConfig {
                cdc_window: CdcWindow {
                    max_duration: Duration::from_secs(5),
                    max_events: 0,
                },
                ..DurableSyncConfig::default()
            },
            "cdc_window.max_events",
        ),
        (
            DurableSyncConfig {
                cdc_supervise_interval: Duration::from_millis(500),
                ..DurableSyncConfig::default()
            },
            "cdc_supervise_interval",
        ),
        (
            DurableSyncConfig {
                cdc_windows_per_run: 0,
                ..DurableSyncConfig::default()
            },
            "cdc_windows_per_run",
        ),
        (
            DurableSyncConfig {
                cdc_error_backoff: Duration::ZERO,
                ..DurableSyncConfig::default()
            },
            "cdc_error_backoff",
        ),
    ];
    for (config, field) in cases {
        let Err(error) = config.validate() else {
            panic!("expected {field} to be rejected");
        };
        assert!(error.to_string().contains(field), "{field}: {error}");
    }
}

#[test]
fn cdc_adds_the_supervisor_schedule() {
    let schedules = super::DurableSync::schedules_for(
        &DurableSyncConfig {
            cdc_supervise_interval: Duration::from_secs(12),
            ..DurableSyncConfig::default()
        },
        true,
    );
    let supervisor = schedules
        .iter()
        .find(|schedule| schedule.workflow_name == CDC_SUPERVISOR_WORKFLOW)
        .unwrap_or_else(|| panic!("no supervisor schedule in {schedules:?}"));
    assert!(matches!(
        supervisor.schedule,
        Schedule::Interval(interval) if interval == Duration::from_secs(12)
    ));
    assert_eq!(supervisor.queue_name, FORCE_SYNC_QUEUE);
    assert_eq!(supervisor.max_active_runs, 1);
}

#[test]
fn cdc_workflow_id_is_stable_per_stream() {
    assert_eq!(
        cdc_workflow_id("salesforce:Account"),
        "force-sync-cdc-salesforce:Account"
    );
}

#[test]
fn cdc_window_result_round_trips_through_json() {
    for result in [
        CdcWindowResult::Unconfigured,
        CdcWindowResult::Window(CdcWindowReport {
            captured: 3,
            duplicates: 1,
            quarantined: 2,
            end: CdcWindowEnd::MaxEvents,
        }),
    ] {
        let back: CdcWindowResult = serde_json::from_value(to_json(&result))
            .unwrap_or_else(|error| panic!("round trip: {error}"));
        assert_eq!(back, result);
    }
}

#[tokio::test]
async fn cdc_stream_runs_its_windows_then_continues_as_new() {
    let outcome = run_cdc_stream(
        WorkflowTestEnv::new()
            .mock_activity("force_sync_cdc_window", |input| {
                assert_eq!(input, json!("salesforce:Account"));
                Ok(window_report(2))
            })
            .mock_activity("force_sync_cdc_settings", |_| Ok(cdc_input(3))),
        cdc_input(3),
    )
    .await;

    let names: Vec<String> = scheduled(&outcome).into_iter().map(|(n, _)| n).collect();
    assert_eq!(
        names,
        vec![
            "force_sync_cdc_window",
            "force_sync_cdc_window",
            "force_sync_cdc_window",
            "force_sync_cdc_settings"
        ]
    );
    assert!(
        timers(&outcome).is_empty(),
        "no backoff after a good window"
    );
    assert_eq!(continued_as_new_input(&outcome), Some(cdc_input(3)));
}

#[tokio::test]
async fn failed_cdc_window_backs_off_then_keeps_following_the_stream() {
    let outcome = run_cdc_stream(
        WorkflowTestEnv::new()
            .mock_activity_attempt(
                "force_sync_cdc_window",
                1,
                Err("pub/sub unavailable".to_owned()),
            )
            .mock_activity("force_sync_cdc_window", |_| Ok(window_report(1)))
            .mock_activity("force_sync_cdc_settings", |_| Ok(cdc_input(2))),
        cdc_input(2),
    )
    .await;

    let names: Vec<String> = scheduled(&outcome).into_iter().map(|(n, _)| n).collect();
    assert_eq!(
        names,
        vec![
            "force_sync_cdc_window",
            "force_sync_cdc_window",
            "force_sync_cdc_settings"
        ]
    );
    assert_eq!(timers(&outcome), vec![7], "backoff uses error_backoff_secs");
    assert!(continued_as_new_input(&outcome).is_some());
}

#[tokio::test]
async fn unconfigured_cdc_stream_ends_its_workflow() {
    let outcome = run_cdc_stream(
        WorkflowTestEnv::new().mock_activity("force_sync_cdc_window", |_| {
            Ok(to_json(&CdcWindowResult::Unconfigured))
        }),
        cdc_input(5),
    )
    .await;

    assert_eq!(outcome.result, Ok(Value::Null));
    assert_eq!(scheduled(&outcome).len(), 1);
    assert_eq!(continued_as_new_input(&outcome), None);
}

#[tokio::test]
async fn cdc_supervisor_runs_the_ensure_activity() {
    let handler = workflows::force_sync_cdc_supervisor_info().handler;
    let outcome = WorkflowTestEnv::new()
        .mock_activity("force_sync_cdc_ensure", |_| Ok(json!(2)))
        .run(handler, Value::Null)
        .await;
    assert_eq!(outcome.result, Ok(json!(2)));
    let names: Vec<String> = scheduled(&outcome).into_iter().map(|(n, _)| n).collect();
    assert_eq!(names, vec!["force_sync_cdc_ensure"]);
    let replay = outcome.replay_check(handler).await;
    assert!(
        matches!(replay.status, ReplayStatus::ReplaySucceeded),
        "{replay}"
    );
}

#[tokio::test]
async fn cdc_stream_continues_with_the_current_settings() {
    let changed = serde_json::to_value(CdcStreamInput {
        stream: "salesforce:Account".to_owned(),
        windows_per_run: 9,
        error_backoff_secs: 30,
    })
    .unwrap_or_else(|error| panic!("serialize: {error}"));
    let returned = changed.clone();
    let outcome = run_cdc_stream(
        WorkflowTestEnv::new()
            .mock_activity("force_sync_cdc_window", |_| Ok(window_report(0)))
            .mock_activity("force_sync_cdc_settings", move |input| {
                assert_eq!(input, json!("salesforce:Account"));
                Ok(returned.clone())
            }),
        cdc_input(1),
    )
    .await;

    assert_eq!(
        continued_as_new_input(&outcome),
        Some(changed),
        "a config change must reach the stream at its next run"
    );
}

#[tokio::test]
async fn cdc_stream_removed_at_the_run_boundary_ends_its_workflow() {
    let outcome = run_cdc_stream(
        WorkflowTestEnv::new()
            .mock_activity("force_sync_cdc_window", |_| Ok(window_report(0)))
            .mock_activity("force_sync_cdc_settings", |_| Ok(Value::Null)),
        cdc_input(1),
    )
    .await;

    assert_eq!(outcome.result, Ok(Value::Null));
    assert_eq!(continued_as_new_input(&outcome), None);
}

// -- DurableSync::with_cdc validation -----------------------------------

mod with_cdc {
    use std::sync::Arc;

    use async_trait::async_trait;
    use force::auth::{AccessToken, Authenticator, TokenResponse};
    use force_pubsub::ReplayPreset;

    use super::super::{CDC_SUPERVISOR_WORKFLOW, DurableSync, harvest_pool};
    use crate::{
        CdcEventStream, CdcSource, CdcStream, ForceSyncError, ObjectSync, PgStore, SyncEngine,
    };

    #[derive(Debug, Clone)]
    struct StubAuth;

    #[async_trait]
    impl Authenticator for StubAuth {
        async fn authenticate(&self) -> force::error::Result<AccessToken> {
            Ok(AccessToken::from_response(TokenResponse {
                access_token: secrecy::SecretString::new("stub".to_owned().into()),
                instance_url: "https://stub.invalid".to_owned(),
                token_type: "Bearer".to_owned(),
                issued_at: "1704067200000".to_owned(),
                signature: "stub".to_owned(),
                expires_in: Some(7200),
                refresh_token: None,
            }))
        }

        async fn refresh(&self) -> force::error::Result<AccessToken> {
            self.authenticate().await
        }
    }

    /// A source that is never subscribed in these tests.
    struct NoSource;

    #[async_trait]
    impl CdcSource for NoSource {
        async fn subscribe(
            &self,
            _topic: &str,
            _replay: ReplayPreset,
        ) -> Result<CdcEventStream, ForceSyncError> {
            Err(ForceSyncError::NotImplemented)
        }
    }

    /// A durable sync over an Account-only engine. Both pools are lazy, so
    /// no database is needed.
    async fn account_sync() -> DurableSync {
        let client = force::client::builder()
            .authenticate(StubAuth)
            .build()
            .await
            .unwrap_or_else(|error| panic!("client: {error}"));
        let mut config = deadpool_postgres::Config::new();
        config.url = Some("postgresql://unused:unused@localhost:5432/unused".to_owned());
        let pool = config
            .create_pool(
                Some(deadpool_postgres::Runtime::Tokio1),
                tokio_postgres::NoTls,
            )
            .unwrap_or_else(|error| panic!("pool: {error}"));
        let engine = SyncEngine::builder(client)
            .postgres(PgStore::new(pool))
            .object(ObjectSync::new("Account").external_id("ExternalId__c"))
            .build()
            .unwrap_or_else(|error| panic!("engine: {error}"));
        let harvest = harvest_pool("postgresql://unused:unused@localhost:5432/unused", 1)
            .unwrap_or_else(|error| panic!("harvest pool: {error}"));
        DurableSync::new(engine, harvest)
    }

    fn rejection(result: Result<DurableSync, ForceSyncError>) -> ForceSyncError {
        match result {
            Ok(_) => panic!("with_cdc must reject this setup"),
            Err(error) => error,
        }
    }

    #[tokio::test]
    async fn rejects_an_empty_stream_list() {
        let error = rejection(
            account_sync()
                .await
                .with_cdc(Arc::new(NoSource), Vec::<CdcStream>::new()),
        );
        assert!(error.to_string().contains("at least one stream"), "{error}");
    }

    #[tokio::test]
    async fn rejects_two_streams_with_one_name() {
        let error = rejection(account_sync().await.with_cdc(
            Arc::new(NoSource),
            [
                CdcStream::new("tenant", "Account"),
                CdcStream::new("tenant", "Account").with_topic("/data/Other__chn"),
            ],
        ));
        assert!(error.to_string().contains("salesforce:Account"), "{error}");
    }

    #[tokio::test]
    async fn rejects_a_stream_for_an_object_the_engine_does_not_sync() {
        let error = rejection(
            account_sync()
                .await
                .with_cdc(Arc::new(NoSource), [CdcStream::new("tenant", "Contact")]),
        );
        assert!(matches!(
            error,
            ForceSyncError::MissingConfiguration {
                field: "object sync"
            }
        ));
    }

    #[tokio::test]
    async fn valid_streams_add_the_supervisor_schedule() {
        let without = account_sync().await;
        assert!(
            without
                .schedules()
                .iter()
                .all(|schedule| schedule.workflow_name != CDC_SUPERVISOR_WORKFLOW),
            "no supervisor without CDC"
        );

        let with = account_sync()
            .await
            .with_cdc(Arc::new(NoSource), [CdcStream::new("tenant", "Account")])
            .unwrap_or_else(|error| panic!("valid setup rejected: {error}"));
        assert!(
            with.schedules()
                .iter()
                .any(|schedule| schedule.workflow_name == CDC_SUPERVISOR_WORKFLOW)
        );
    }
}
